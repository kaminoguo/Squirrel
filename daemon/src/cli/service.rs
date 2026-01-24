//! System service management (systemd/launchd).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tracing::info;

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

    /// Get systemd user service directory.
    fn service_dir() -> Result<PathBuf, Error> {
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        Ok(home.join(".config/systemd/user"))
    }

    /// Get service file path.
    fn service_file() -> Result<PathBuf, Error> {
        Ok(service_dir()?.join(format!("{}.service", SERVICE_NAME)))
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

    /// Install the systemd service.
    pub fn install() -> Result<(), Error> {
        let binary_path = get_binary_path()?;
        let service_dir = service_dir()?;
        let service_file = service_file()?;

        // Create service directory if needed
        fs::create_dir_all(&service_dir)?;

        // Write service file
        let content = generate_service_file(&binary_path);
        fs::write(&service_file, content)?;
        info!(path = %service_file.display(), "Created systemd service file");

        // Reload systemd
        Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output()?;

        // Enable the service (auto-start on login)
        Command::new("systemctl")
            .args(["--user", "enable", SERVICE_NAME])
            .output()?;

        info!("Systemd service installed and enabled");
        Ok(())
    }

    /// Uninstall the systemd service.
    pub fn uninstall() -> Result<(), Error> {
        let service_file = service_file()?;

        // Stop and disable
        let _ = Command::new("systemctl")
            .args(["--user", "stop", SERVICE_NAME])
            .output();
        let _ = Command::new("systemctl")
            .args(["--user", "disable", SERVICE_NAME])
            .output();

        // Remove service file
        if service_file.exists() {
            fs::remove_file(&service_file)?;
            info!(path = %service_file.display(), "Removed systemd service file");
        }

        // Reload systemd
        Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output()?;

        info!("Systemd service uninstalled");
        Ok(())
    }

    /// Start the service.
    pub fn start() -> Result<(), Error> {
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

    /// Stop the service.
    pub fn stop() -> Result<(), Error> {
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

    /// Check if the service is running.
    pub fn is_running() -> Result<bool, Error> {
        let output = Command::new("systemctl")
            .args(["--user", "is-active", SERVICE_NAME])
            .output()?;

        Ok(output.status.success())
    }

    /// Check if the service is installed.
    pub fn is_installed() -> Result<bool, Error> {
        Ok(service_file()?.exists())
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
