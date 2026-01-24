//! Remove Squirrel from a project.

use std::fs;
use std::io::{self, Write};

use tracing::warn;

use crate::cli::service;
use crate::error::Error;

/// Run the goaway command.
pub async fn run(force: bool) -> Result<(), Error> {
    let project_root = std::env::current_dir()?;
    let sqrl_dir = project_root.join(".sqrl");

    if !sqrl_dir.exists() {
        println!("No .sqrl/ directory found in this project.");
        return Ok(());
    }

    // Show what will be removed
    println!("This will remove:");
    println!("  .sqrl/ ({})", sqrl_dir.display());
    print_dir_contents(&sqrl_dir, 4)?;

    // Confirm unless --force
    if !force {
        print!("\nAre you sure? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Stop and uninstall the service
    if service::is_installed().unwrap_or(false) {
        println!("Stopping and uninstalling background service...");
        if let Err(e) = service::uninstall() {
            warn!(error = %e, "Failed to uninstall service");
            println!("Warning: Could not uninstall background service: {}", e);
        }
    }

    // Remove the directory
    fs::remove_dir_all(&sqrl_dir)?;
    println!("Removed .sqrl/");
    println!("Squirrel has left the building.");

    Ok(())
}

fn print_dir_contents(path: &std::path::Path, indent: usize) -> Result<(), Error> {
    let indent_str = " ".repeat(indent);
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let metadata = entry.metadata()?;
            if metadata.is_dir() {
                println!("{}{}/", indent_str, name.to_string_lossy());
            } else {
                let size = metadata.len();
                println!(
                    "{}{} ({})",
                    indent_str,
                    name.to_string_lossy(),
                    format_size(size)
                );
            }
        }
    }
    Ok(())
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
