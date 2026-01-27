//! Initialize Squirrel for a project.

use std::fs;
use std::path::PathBuf;

use tracing::{info, warn};

use crate::cli::{hooks, service};
use crate::config::Config;
use crate::error::Error;
use crate::ipc::IpcClient;
use crate::watcher;

/// Default history to process (30 days).
const DEFAULT_HISTORY_DAYS: u32 = 30;

/// Run the init command.
pub async fn run(with_history: bool) -> Result<(), Error> {
    let project_root = std::env::current_dir()?;
    let sqrl_dir = project_root.join(".sqrl");

    // Check if already initialized
    if sqrl_dir.exists() {
        println!("Squirrel already initialized in this project.");
        println!("Run 'sqrl goaway' first if you want to reinitialize.");
        return Ok(());
    }

    // Create .sqrl directory
    fs::create_dir_all(&sqrl_dir)?;
    info!(path = %sqrl_dir.display(), "Created .sqrl directory");

    // Create empty database file (placeholder)
    let db_path = sqrl_dir.join("memory.db");
    fs::write(&db_path, "")?;
    info!(path = %db_path.display(), "Created database");

    // Add .sqrl/ to .gitignore if not already there
    add_to_gitignore(&project_root)?;

    // Create config with defaults (CONFIG-001)
    let config = Config::default();
    config.save(&project_root)?;
    info!("Created config.yaml");

    // Install git hooks if git exists and auto_install enabled
    if config.hooks.auto_install && hooks::has_git(&project_root) {
        if let Err(e) = hooks::install_hooks(&project_root, config.hooks.pre_push_block) {
            warn!(error = %e, "Failed to install git hooks");
        } else {
            println!("Git hooks installed.");
        }
    }

    println!("Squirrel initialized.");

    if with_history {
        println!(
            "Processing last {} days of history...",
            DEFAULT_HISTORY_DAYS
        );

        // Connect to Python Memory Service
        let ipc_client = IpcClient::default();
        if ipc_client.is_service_running().await {
            match watcher::process_history(&project_root, DEFAULT_HISTORY_DAYS, &ipc_client).await {
                Ok(stats) => {
                    println!("{}", stats);
                }
                Err(e) => {
                    warn!(error = %e, "Failed to process history");
                    println!("Warning: Could not process history: {}", e);
                }
            }
        } else {
            println!("Warning: Memory Service not running.");
            println!("History will be processed when you run 'sqrl init' again with the service running.");
        }
    }

    // Install and start the Rust daemon service
    if !service::is_installed()? {
        println!("Installing background service...");
        if let Err(e) = service::install() {
            warn!(error = %e, "Failed to install service");
            println!("Warning: Could not install background service: {}", e);
            println!("You can manually start the watcher with 'sqrl watch-daemon'");
        } else {
            println!("Background service installed.");
        }
    } else if !service::is_running()? {
        // Service installed but not running, start it
        if let Err(e) = service::start() {
            warn!(error = %e, "Failed to start service");
            println!("Warning: Could not start background service: {}", e);
        }
    }

    // Start Python Memory Service
    if let Err(e) = service::start_python_service() {
        warn!(error = %e, "Failed to start Python Memory Service");
        // Don't fail - daemon works without Python service
    }

    println!();
    println!("Watcher is enabled. Squirrel will learn from your coding sessions.");
    println!("Run 'sqrl off' to disable, 'sqrl on' to re-enable.");

    Ok(())
}

/// Add .sqrl/ to .gitignore if not already present.
fn add_to_gitignore(project_root: &PathBuf) -> Result<(), Error> {
    let gitignore_path = project_root.join(".gitignore");

    let content = if gitignore_path.exists() {
        fs::read_to_string(&gitignore_path)?
    } else {
        String::new()
    };

    // Check if .sqrl/ is already in .gitignore
    if content
        .lines()
        .any(|line| line.trim() == ".sqrl/" || line.trim() == ".sqrl")
    {
        return Ok(());
    }

    // Append .sqrl/ to .gitignore
    let new_content = if content.is_empty() || content.ends_with('\n') {
        format!("{}.sqrl/\n", content)
    } else {
        format!("{}\n.sqrl/\n", content)
    };

    fs::write(&gitignore_path, new_content)?;
    info!("Added .sqrl/ to .gitignore");

    Ok(())
}
