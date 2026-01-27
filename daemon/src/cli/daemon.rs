//! Daemon control commands (on/off).

use tracing::{info, warn};

use crate::cli::service;
use crate::config::Config;
use crate::error::Error;

/// Enable the watcher daemon.
pub async fn enable() -> Result<(), Error> {
    let project_root = std::env::current_dir()?;
    let config_path = Config::path(&project_root);

    if !config_path.exists() {
        println!("Squirrel not initialized. Run 'sqrl init' first.");
        return Ok(());
    }

    // Load and update config
    let mut config = Config::load(&project_root)?;
    config.set_watcher_enabled(true);
    config.save(&project_root)?;
    info!("Watcher enabled in config");

    // Start the Rust daemon service
    if !service::is_installed()? {
        println!("Installing background service...");
        if let Err(e) = service::install() {
            warn!(error = %e, "Failed to install service");
            println!("Warning: Could not install background service: {}", e);
            return Ok(());
        }
    }

    if !service::is_running()? {
        if let Err(e) = service::start() {
            warn!(error = %e, "Failed to start service");
            println!("Warning: Could not start background service: {}", e);
            return Ok(());
        }
    }

    // Start the Python Memory Service
    if let Err(e) = service::start_python_service() {
        warn!(error = %e, "Failed to start Python Memory Service");
        // Don't fail - daemon works without Python service
    }

    println!("Watcher enabled.");
    println!("Squirrel will learn from your coding sessions.");

    Ok(())
}

/// Disable the watcher daemon.
pub async fn disable() -> Result<(), Error> {
    let project_root = std::env::current_dir()?;
    let config_path = Config::path(&project_root);

    if !config_path.exists() {
        println!("Squirrel not initialized. Run 'sqrl init' first.");
        return Ok(());
    }

    // Load and update config
    let mut config = Config::load(&project_root)?;
    config.set_watcher_enabled(false);
    config.save(&project_root)?;
    info!("Watcher disabled in config");

    // Stop the Rust daemon service
    if service::is_running()? {
        if let Err(e) = service::stop() {
            warn!(error = %e, "Failed to stop service");
            println!("Warning: Could not stop background service: {}", e);
        }
    }

    // Stop the Python Memory Service
    if let Err(e) = service::stop_python_service() {
        warn!(error = %e, "Failed to stop Python Memory Service");
    }

    println!("Watcher disabled.");
    println!("Run 'sqrl on' to re-enable.");

    Ok(())
}
