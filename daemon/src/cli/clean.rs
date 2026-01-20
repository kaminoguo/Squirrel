//! Clean/uninstall Squirrel data.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::error::Error;

/// Run clean command.
pub async fn run(project: bool, global: bool, all: bool, force: bool) -> Result<(), Error> {
    // Determine what to clean
    let clean_project = project || all || (!project && !global && !all);
    let clean_global = global || all;

    if !clean_project && !clean_global {
        println!("Nothing to clean. Use --project, --global, or --all.");
        return Ok(());
    }

    // Collect paths to remove
    let mut paths_to_remove: Vec<(&str, std::path::PathBuf)> = Vec::new();

    if clean_project {
        let project_sqrl = std::env::current_dir()?.join(".sqrl");
        if project_sqrl.exists() {
            paths_to_remove.push(("Project .sqrl/", project_sqrl));
        } else {
            println!("No .sqrl/ directory in current project.");
        }
    }

    if clean_global {
        if let Some(home) = dirs::home_dir() {
            let global_sqrl = home.join(".sqrl");
            if global_sqrl.exists() {
                paths_to_remove.push(("Global ~/.sqrl/", global_sqrl));
            } else {
                println!("No ~/.sqrl/ directory found.");
            }
        }
    }

    if paths_to_remove.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    // Show what will be removed
    println!("The following will be removed:");
    for (label, path) in &paths_to_remove {
        println!("  {} ({})", label, path.display());
        print_dir_contents(path, 2)?;
    }

    // Confirm unless --force
    if !force {
        print!("\nProceed? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Remove directories
    for (label, path) in &paths_to_remove {
        fs::remove_dir_all(path)?;
        println!("Removed {}", label);
    }

    println!("Clean complete.");
    Ok(())
}

fn print_dir_contents(path: &Path, indent: usize) -> Result<(), Error> {
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
