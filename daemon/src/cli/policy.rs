//! Manage memory policy (CLI-011).

use crate::config::Config;
use crate::error::Error;
use crate::ipc::send_reload_policy;

/// Run policy command.
pub async fn run(action: &str) -> Result<(), Error> {
    match action {
        "show" => {
            // Check for project policy first
            let cwd = std::env::current_dir()?;
            let project_policy = cwd.join(".sqrl").join("policy.toml");
            let global_policy = dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".sqrl")
                .join("policy.toml");

            if project_policy.exists() {
                println!("Project Policy: {}", project_policy.display());
                let content = std::fs::read_to_string(&project_policy)?;
                println!("{}", content);
            } else if global_policy.exists() {
                println!("Global Policy: {}", global_policy.display());
                let content = std::fs::read_to_string(&global_policy)?;
                println!("{}", content);
            } else {
                println!("No policy file found.");
                println!();
                println!("Create one at:");
                println!("  Project: .sqrl/policy.toml");
                println!("  Global: ~/.sqrl/policy.toml");
            }
        }

        "reload" => {
            let config = Config::load()?;
            let socket_path = &config.daemon.socket_path;

            match send_reload_policy(socket_path).await {
                Ok(result) => {
                    if let Some(msg) = result.get("message") {
                        println!("Policy reload: {}", msg);
                    } else {
                        println!("Policy reload signaled.");
                    }
                }
                Err(e) => {
                    eprintln!("Failed to signal daemon: {}", e);
                    eprintln!("Is the daemon running? Start it with 'sqrl daemon'");
                }
            }
        }

        _ => {
            println!("Unknown action: {}", action);
            println!("Usage: sqrl policy [show|reload]");
        }
    }

    Ok(())
}
