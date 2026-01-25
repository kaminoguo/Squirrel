//! Open configuration UI.

use std::process::Command;

use crate::cli::service;
use crate::dashboard::DEFAULT_PORT;
use crate::error::Error;

/// Open the config UI in browser.
pub fn open() -> Result<(), Error> {
    let url = format!("http://localhost:{}", DEFAULT_PORT);

    // Check if daemon is running (dashboard is part of daemon)
    if !service::is_running().unwrap_or(false) {
        println!("Daemon is not running.");
        println!("Run 'sqrl on' to start the daemon first.");
        return Ok(());
    }

    println!("Opening {}", url);
    open_browser(&url)?;

    Ok(())
}

/// Open URL in default browser.
fn open_browser(url: &str) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| Error::Ipc(format!("Failed to open browser: {}", e)))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try xdg-open first, then fallback to common browsers
        if Command::new("xdg-open").arg(url).spawn().is_err() {
            if Command::new("firefox").arg(url).spawn().is_err() {
                if Command::new("chromium").arg(url).spawn().is_err() {
                    Command::new("google-chrome")
                        .arg(url)
                        .spawn()
                        .map_err(|e| Error::Ipc(format!("Failed to open browser: {}", e)))?;
                }
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/c", "start", url])
            .spawn()
            .map_err(|e| Error::Ipc(format!("Failed to open browser: {}", e)))?;
    }

    Ok(())
}
