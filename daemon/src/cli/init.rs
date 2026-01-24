//! Initialize Squirrel for a project.

use std::fs;
use std::path::PathBuf;

use tracing::{info, warn};

use crate::cli::service;
use crate::error::Error;

/// Default history to process (30 days).
const DEFAULT_HISTORY_DAYS: u32 = 30;

/// Run the init command.
pub async fn run(process_history: bool) -> Result<(), Error> {
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

    // Enable watcher by default
    let config_path = sqrl_dir.join("config.json");
    let config = serde_json::json!({
        "watcher_enabled": true,
        "initialized_at": chrono::Utc::now().to_rfc3339(),
    });
    fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

    println!("Squirrel initialized.");

    if process_history {
        println!(
            "Processing last {} days of history...",
            DEFAULT_HISTORY_DAYS
        );
        // TODO: Implement history processing
        // This would scan ~/.claude/projects/ for sessions related to this project
        // and process them chronologically
        println!("(History processing not yet implemented)");
    }

    // Install and start the system service
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
