//! System service management (systemd/launchd).
//!
//! On Linux, uses systemd user sessions when available.
//! Falls back to direct process spawning for WSL and other environments
//! where systemd user sessions don't work.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tracing::{info, warn};

use crate::error::Error;

/// Service name.
const SERVICE_NAME: &str = "dev.sqrl.daemon";

/// Get the path to the sqrl binary.
fn get_binary_path() -> Result<PathBuf, Error> {
    std::env::current_exe().map_err(|e| Error::Io(e))
}

// =============================================================================
// Linux (systemd)
// =============================================================================

#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::os::unix::process::CommandExt;

    /// Get systemd user service directory.
    fn service_dir() -> Result<PathBuf, Error> {
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        Ok(home.join(".config/systemd/user"))
    }

    /// Get service file path.
    fn service_file() -> Result<PathBuf, Error> {
        Ok(service_dir()?.join(format!("{}.service", SERVICE_NAME)))
    }

    /// Get PID file path for fallback mode.
    fn pid_file() -> Result<PathBuf, Error> {
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        Ok(home.join(".sqrl/daemon.pid"))
    }

    /// Get log file path for fallback mode.
    fn log_file() -> Result<PathBuf, Error> {
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        Ok(home.join(".sqrl/daemon.log"))
    }

    /// Check if systemd user session is available.
    fn is_systemd_available() -> bool {
        // Check if we can communicate with systemd user session
        // This is the most reliable check - try to actually list units
        let output = Command::new("systemctl")
            .args(["--user", "list-units", "--no-pager", "--plain"])
            .output();

        match output {
            Ok(o) => {
                // If it succeeds (exit 0) and produces output, systemd is available
                o.status.success() && !o.stdout.is_empty()
            }
            Err(_) => false,
        }
    }

    /// Generate systemd service unit file.
    fn generate_service_file(binary_path: &PathBuf) -> String {
        format!(
            r#"[Unit]
Description=Squirrel Daemon - AI coding memory system
After=network.target

[Service]
Type=simple
ExecStart={binary} watch-daemon
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=sqrl=info

[Install]
WantedBy=default.target
"#,
            binary = binary_path.display()
        )
    }

    /// Install the systemd service (only when systemd is available).
    fn install_systemd() -> Result<(), Error> {
        let binary_path = get_binary_path()?;
        let service_dir = service_dir()?;
        let service_file = service_file()?;

        fs::create_dir_all(&service_dir)?;

        let content = generate_service_file(&binary_path);
        fs::write(&service_file, content)?;
        info!(path = %service_file.display(), "Created systemd service file");

        Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output()?;

        Command::new("systemctl")
            .args(["--user", "enable", SERVICE_NAME])
            .output()?;

        info!("Systemd service installed and enabled");
        Ok(())
    }

    /// Install for fallback mode (just ensure directories exist).
    fn install_fallback() -> Result<(), Error> {
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        fs::create_dir_all(home.join(".sqrl"))?;
        info!("Fallback mode: directories ready");
        Ok(())
    }

    /// Install the service.
    pub fn install() -> Result<(), Error> {
        if is_systemd_available() {
            install_systemd()
        } else {
            warn!("Systemd not available, using fallback process mode");
            install_fallback()
        }
    }

    /// Uninstall the systemd service.
    pub fn uninstall() -> Result<(), Error> {
        // Clean up systemd if it exists
        let service_file = service_file()?;
        if service_file.exists() {
            let _ = Command::new("systemctl")
                .args(["--user", "stop", SERVICE_NAME])
                .output();
            let _ = Command::new("systemctl")
                .args(["--user", "disable", SERVICE_NAME])
                .output();
            fs::remove_file(&service_file)?;
            info!(path = %service_file.display(), "Removed systemd service file");
            let _ = Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .output();
        }

        // Clean up fallback mode
        let _ = stop_fallback();
        let pid = pid_file()?;
        if pid.exists() {
            let _ = fs::remove_file(&pid);
        }

        info!("Service uninstalled");
        Ok(())
    }

    /// Start via systemd.
    fn start_systemd() -> Result<(), Error> {
        let output = Command::new("systemctl")
            .args(["--user", "start", SERVICE_NAME])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Ipc(format!("Failed to start service: {}", stderr)));
        }

        info!("Systemd service started");
        Ok(())
    }

    /// Start via fallback (spawn background process).
    fn start_fallback() -> Result<(), Error> {
        let binary_path = get_binary_path()?;
        let pid_path = pid_file()?;
        let log_path = log_file()?;

        // Check if already running
        if is_running_fallback()? {
            info!("Daemon already running");
            return Ok(());
        }

        // Ensure parent directory exists
        if let Some(parent) = pid_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Open log file
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)?;
        let log_err = log.try_clone()?;

        // Spawn the daemon as a background process
        // Use setsid to create a new session (detach from terminal)
        let child = unsafe {
            Command::new(&binary_path)
                .arg("watch-daemon")
                .env("RUST_LOG", "sqrl=info")
                .stdout(log)
                .stderr(log_err)
                .stdin(std::process::Stdio::null())
                .pre_exec(|| {
                    // Create new session to detach from terminal
                    libc::setsid();
                    Ok(())
                })
                .spawn()?
        };

        // Write PID file
        fs::write(&pid_path, child.id().to_string())?;
        info!(pid = child.id(), "Started daemon in fallback mode");

        Ok(())
    }

    /// Start the service.
    pub fn start() -> Result<(), Error> {
        if is_systemd_available() && service_file()?.exists() {
            start_systemd()
        } else {
            warn!("Using fallback process mode (systemd unavailable)");
            start_fallback()
        }
    }

    /// Stop via systemd.
    fn stop_systemd() -> Result<(), Error> {
        let output = Command::new("systemctl")
            .args(["--user", "stop", SERVICE_NAME])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Ipc(format!("Failed to stop service: {}", stderr)));
        }

        info!("Systemd service stopped");
        Ok(())
    }

    /// Stop via fallback (kill process).
    fn stop_fallback() -> Result<(), Error> {
        let pid_path = pid_file()?;

        if !pid_path.exists() {
            return Ok(());
        }

        let pid_str = fs::read_to_string(&pid_path)?;
        let pid: i32 = pid_str
            .trim()
            .parse()
            .map_err(|_| Error::Ipc("Invalid PID in pid file".to_string()))?;

        // Check if process is still running
        let proc_path = PathBuf::from(format!("/proc/{}", pid));
        if proc_path.exists() {
            // Send SIGTERM
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            info!(pid = pid, "Sent SIGTERM to daemon");

            // Wait briefly for graceful shutdown
            std::thread::sleep(std::time::Duration::from_millis(500));

            // Force kill if still running
            if proc_path.exists() {
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
                warn!(pid = pid, "Force killed daemon");
            }
        }

        // Remove PID file
        let _ = fs::remove_file(&pid_path);
        info!("Daemon stopped");

        Ok(())
    }

    /// Stop the service.
    pub fn stop() -> Result<(), Error> {
        // Try both methods - one will work
        if is_systemd_available() && service_file()?.exists() {
            let _ = stop_systemd();
        }
        stop_fallback()
    }

    /// Check if running via systemd.
    fn is_running_systemd() -> Result<bool, Error> {
        let output = Command::new("systemctl")
            .args(["--user", "is-active", SERVICE_NAME])
            .output()?;

        Ok(output.status.success())
    }

    /// Check if running via fallback (PID file).
    fn is_running_fallback() -> Result<bool, Error> {
        let pid_path = pid_file()?;

        if !pid_path.exists() {
            return Ok(false);
        }

        let pid_str = fs::read_to_string(&pid_path)?;
        let pid: i32 = match pid_str.trim().parse() {
            Ok(p) => p,
            Err(_) => {
                // Invalid PID file, clean up
                let _ = fs::remove_file(&pid_path);
                return Ok(false);
            }
        };

        // Check if process exists
        let proc_path = PathBuf::from(format!("/proc/{}", pid));
        if !proc_path.exists() {
            // Process died, clean up PID file
            let _ = fs::remove_file(&pid_path);
            return Ok(false);
        }

        // Verify it's actually our daemon by checking cmdline
        let cmdline_path = proc_path.join("cmdline");
        if let Ok(file) = fs::File::open(&cmdline_path) {
            let reader = BufReader::new(file);
            if let Some(Ok(line)) = reader.lines().next() {
                if line.contains("sqrl") && line.contains("watch-daemon") {
                    return Ok(true);
                }
            }
        }

        // Process exists but might not be ours - assume it's stale
        let _ = fs::remove_file(&pid_path);
        Ok(false)
    }

    /// Check if the service is running.
    pub fn is_running() -> Result<bool, Error> {
        // Check systemd first
        if is_systemd_available() {
            if is_running_systemd()? {
                return Ok(true);
            }
        }
        // Check fallback mode
        is_running_fallback()
    }

    /// Check if the service is installed.
    pub fn is_installed() -> Result<bool, Error> {
        // Either systemd service file exists, or fallback is set up
        if service_file()?.exists() {
            return Ok(true);
        }
        // In fallback mode, we're always "installed" if ~/.sqrl exists
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        Ok(home.join(".sqrl").exists())
    }
}

// =============================================================================
// macOS (launchd)
// =============================================================================

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    /// Get LaunchAgents directory.
    fn agents_dir() -> Result<PathBuf, Error> {
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        Ok(home.join("Library/LaunchAgents"))
    }

    /// Get plist file path.
    fn plist_file() -> Result<PathBuf, Error> {
        Ok(agents_dir()?.join(format!("{}.plist", SERVICE_NAME)))
    }

    /// Generate launchd plist file.
    fn generate_plist(binary_path: &PathBuf) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>watch-daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/sqrl.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/sqrl.stderr.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>sqrl=info</string>
    </dict>
</dict>
</plist>
"#,
            label = SERVICE_NAME,
            binary = binary_path.display()
        )
    }

    /// Install the launchd agent.
    pub fn install() -> Result<(), Error> {
        let binary_path = get_binary_path()?;
        let agents_dir = agents_dir()?;
        let plist_file = plist_file()?;

        // Create agents directory if needed
        fs::create_dir_all(&agents_dir)?;

        // Write plist file
        let content = generate_plist(&binary_path);
        fs::write(&plist_file, content)?;
        info!(path = %plist_file.display(), "Created launchd plist file");

        // Load the agent
        Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&plist_file)
            .output()?;

        info!("Launchd agent installed and loaded");
        Ok(())
    }

    /// Uninstall the launchd agent.
    pub fn uninstall() -> Result<(), Error> {
        let plist_file = plist_file()?;

        // Unload the agent
        if plist_file.exists() {
            let _ = Command::new("launchctl")
                .args(["unload", "-w"])
                .arg(&plist_file)
                .output();

            fs::remove_file(&plist_file)?;
            info!(path = %plist_file.display(), "Removed launchd plist file");
        }

        info!("Launchd agent uninstalled");
        Ok(())
    }

    /// Start the service.
    pub fn start() -> Result<(), Error> {
        let plist_file = plist_file()?;

        let output = Command::new("launchctl")
            .args(["load", "-w"])
            .arg(&plist_file)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Ipc(format!("Failed to start service: {}", stderr)));
        }

        info!("Launchd agent started");
        Ok(())
    }

    /// Stop the service.
    pub fn stop() -> Result<(), Error> {
        let plist_file = plist_file()?;

        let output = Command::new("launchctl")
            .args(["unload"])
            .arg(&plist_file)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Ipc(format!("Failed to stop service: {}", stderr)));
        }

        info!("Launchd agent stopped");
        Ok(())
    }

    /// Check if the service is running.
    pub fn is_running() -> Result<bool, Error> {
        let output = Command::new("launchctl")
            .args(["list", SERVICE_NAME])
            .output()?;

        Ok(output.status.success())
    }

    /// Check if the service is installed.
    pub fn is_installed() -> Result<bool, Error> {
        Ok(plist_file()?.exists())
    }
}

// =============================================================================
// Windows (Task Scheduler)
// =============================================================================

#[cfg(target_os = "windows")]
mod platform {
    use super::*;

    /// Install the Windows scheduled task.
    pub fn install() -> Result<(), Error> {
        let binary_path = get_binary_path()?;

        // Create scheduled task that runs at logon
        let output = Command::new("schtasks")
            .args([
                "/Create",
                "/TN",
                SERVICE_NAME,
                "/TR",
                &format!("\"{}\" watch-daemon", binary_path.display()),
                "/SC",
                "ONLOGON",
                "/RL",
                "LIMITED",
                "/F", // Force overwrite if exists
            ])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Ipc(format!(
                "Failed to create scheduled task: {}",
                stderr
            )));
        }

        info!("Windows scheduled task installed");

        // Start it now
        start()?;

        Ok(())
    }

    /// Uninstall the Windows scheduled task.
    pub fn uninstall() -> Result<(), Error> {
        // Stop first
        let _ = stop();

        // Delete the task
        let output = Command::new("schtasks")
            .args(["/Delete", "/TN", SERVICE_NAME, "/F"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "task does not exist" errors
            if !stderr.contains("does not exist") {
                return Err(Error::Ipc(format!(
                    "Failed to delete scheduled task: {}",
                    stderr
                )));
            }
        }

        info!("Windows scheduled task uninstalled");
        Ok(())
    }

    /// Start the service.
    pub fn start() -> Result<(), Error> {
        let output = Command::new("schtasks")
            .args(["/Run", "/TN", SERVICE_NAME])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Ipc(format!("Failed to start task: {}", stderr)));
        }

        info!("Windows scheduled task started");
        Ok(())
    }

    /// Stop the service.
    pub fn stop() -> Result<(), Error> {
        let output = Command::new("schtasks")
            .args(["/End", "/TN", SERVICE_NAME])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Ignore "task is not running" errors
            if !stderr.contains("not running") {
                return Err(Error::Ipc(format!("Failed to stop task: {}", stderr)));
            }
        }

        info!("Windows scheduled task stopped");
        Ok(())
    }

    /// Check if the service is running.
    pub fn is_running() -> Result<bool, Error> {
        let output = Command::new("schtasks")
            .args(["/Query", "/TN", SERVICE_NAME, "/FO", "CSV"])
            .output()?;

        if !output.status.success() {
            return Ok(false);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout.contains("Running"))
    }

    /// Check if the service is installed.
    pub fn is_installed() -> Result<bool, Error> {
        let output = Command::new("schtasks")
            .args(["/Query", "/TN", SERVICE_NAME])
            .output()?;

        Ok(output.status.success())
    }
}

// =============================================================================
// Public API
// =============================================================================

pub use platform::*;

// =============================================================================
// Python Memory Service Management
// =============================================================================

/// Get PID file path for Python Memory Service.
fn python_pid_file() -> Result<PathBuf, Error> {
    let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
    Ok(home.join(".sqrl/memory_service.pid"))
}

/// Get log file path for Python Memory Service.
fn python_log_file() -> Result<PathBuf, Error> {
    let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
    Ok(home.join(".sqrl/memory_service.log"))
}

/// Check if Python Memory Service is running.
pub fn is_python_service_running() -> Result<bool, Error> {
    let pid_path = python_pid_file()?;

    if !pid_path.exists() {
        return Ok(false);
    }

    let pid_str = fs::read_to_string(&pid_path)?;
    let pid: u32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            let _ = fs::remove_file(&pid_path);
            return Ok(false);
        }
    };

    // Check if process exists (platform-specific)
    #[cfg(unix)]
    {
        // Check if process exists by sending signal 0
        let result = unsafe { libc::kill(pid as i32, 0) };
        if result != 0 {
            let _ = fs::remove_file(&pid_path);
            return Ok(false);
        }
        Ok(true)
    }

    #[cfg(windows)]
    {
        // On Windows, check if process exists via tasklist
        let output = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid), "/NH"])
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains(&pid.to_string()) {
            let _ = fs::remove_file(&pid_path);
            return Ok(false);
        }
        Ok(true)
    }
}

/// Find Python executable with sqrl module.
fn find_python() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;

    // Check for devenv venv Python first (has sqrl installed via pip -e .)
    let venv_python = cwd.join(".devenv/state/venv/bin/python");
    if venv_python.exists() {
        return Some(venv_python);
    }

    // Check for devenv profile Python
    let devenv_python = cwd.join(".devenv/profile/bin/python");
    if devenv_python.exists() {
        return Some(devenv_python);
    }

    // Check home directory devenv (for global usage)
    if let Some(home) = dirs::home_dir() {
        let home_venv = home.join("projects/Squirrel/.devenv/state/venv/bin/python");
        if home_venv.exists() {
            return Some(home_venv);
        }
        let home_devenv = home.join("projects/Squirrel/.devenv/profile/bin/python");
        if home_devenv.exists() {
            return Some(home_devenv);
        }
    }

    // Fall back to system Python
    Some(PathBuf::from("python"))
}

/// Start Python Memory Service.
pub fn start_python_service() -> Result<(), Error> {
    // Check if already running
    if is_python_service_running()? {
        info!("Python Memory Service already running");
        return Ok(());
    }

    let pid_path = python_pid_file()?;
    let log_path = python_log_file()?;

    // Ensure directory exists
    if let Some(parent) = pid_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Check for required environment variable
    if std::env::var("SQRL_STRONG_MODEL").is_err() {
        warn!("SQRL_STRONG_MODEL not set - Python Memory Service will not start");
        warn!("Set SQRL_STRONG_MODEL to a LiteLLM model ID (e.g., 'openrouter/anthropic/claude-3.5-sonnet')");
        return Ok(());
    }

    // Find Python executable
    let python = match find_python() {
        Some(p) => p,
        None => {
            warn!("Python not found");
            return Ok(());
        }
    };
    info!(python = %python.display(), "Using Python");

    // Open log file
    let log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let log_err = log.try_clone()?;

    // Spawn Python service
    #[cfg(unix)]
    let child = {
        use std::os::unix::process::CommandExt;
        unsafe {
            Command::new(&python)
                .args(["-m", "sqrl", "serve"])
                .stdout(log)
                .stderr(log_err)
                .stdin(std::process::Stdio::null())
                .pre_exec(|| {
                    libc::setsid();
                    Ok(())
                })
                .spawn()
        }
    };

    #[cfg(windows)]
    let child = Command::new(&python)
        .args(["-m", "sqrl", "serve"])
        .stdout(log)
        .stderr(log_err)
        .stdin(std::process::Stdio::null())
        .spawn();

    match child {
        Ok(c) => {
            fs::write(&pid_path, c.id().to_string())?;
            info!(pid = c.id(), "Started Python Memory Service");
            Ok(())
        }
        Err(e) => {
            warn!(error = %e, "Failed to start Python Memory Service");
            warn!("Ensure Python is in PATH and sqrl package is installed");
            Ok(()) // Don't fail - daemon can work without Python service
        }
    }
}

/// Stop Python Memory Service.
pub fn stop_python_service() -> Result<(), Error> {
    let pid_path = python_pid_file()?;

    if !pid_path.exists() {
        return Ok(());
    }

    let pid_str = fs::read_to_string(&pid_path)?;
    let pid: i32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            let _ = fs::remove_file(&pid_path);
            return Ok(());
        }
    };

    #[cfg(unix)]
    {
        // Send SIGTERM
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        info!(pid = pid, "Sent SIGTERM to Python Memory Service");

        // Wait briefly
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Force kill if still running
        let proc_path = PathBuf::from(format!("/proc/{}", pid));
        if proc_path.exists() {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            warn!(pid = pid, "Force killed Python Memory Service");
        }
    }

    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();
        info!(pid = pid, "Stopped Python Memory Service");
    }

    let _ = fs::remove_file(&pid_path);
    Ok(())
}
