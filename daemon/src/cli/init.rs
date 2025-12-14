//! Initialize project for Squirrel (CLI-001, INIT-001).

use std::path::PathBuf;

use crate::config::{Config, ProjectConfig, ProjectsRegistry};
use crate::db::Database;
use crate::error::Error;

use super::agents::{detect_agents, AgentConfig, AgentId, DetectedAgent};
use super::gitignore;
use super::instructions;
use super::mcp_config;

/// Run project initialization.
pub async fn run(skip_history: bool) -> Result<(), Error> {
    let cwd = std::env::current_dir()?;
    println!("Initializing Squirrel in: {}", cwd.display());

    // Create .sqrl directory
    let sqrl_dir = cwd.join(".sqrl");
    std::fs::create_dir_all(&sqrl_dir)?;

    // Initialize database
    let db_path = sqrl_dir.join("squirrel.db");
    let _db = Database::open(&db_path)?;
    println!("Created database: {}", db_path.display());

    // Generate project ID from path
    let project_id = generate_project_id(&cwd);

    // Register project in global registry
    let mut registry = ProjectsRegistry::load()?;
    registry.register(ProjectConfig {
        project_id: project_id.clone(),
        root_path: cwd.clone(),
        initialized_at: chrono::Utc::now().to_rfc3339(),
    });
    registry.save()?;
    println!("Registered project: {}", project_id);

    // Load global config for enabled agents
    let config = Config::load()?;
    let enabled_agents = get_enabled_agents(&config);

    // Detect agents in project
    let detected = detect_agents(&cwd);
    if detected.is_empty() {
        println!("No AI agents detected in project.");
    } else {
        println!("Detected agents:");
        for agent in &detected {
            let enabled = enabled_agents.contains(&agent.id);
            let status = if enabled { "enabled" } else { "disabled" };
            println!("  - {} ({})", agent.id.name(), status);
        }
    }

    // Configure enabled agents
    let mut configured_count = 0;
    for agent in &detected {
        if enabled_agents.contains(&agent.id) {
            if let Err(e) = configure_agent(&cwd, agent) {
                eprintln!("Warning: Failed to configure {}: {}", agent.id.name(), e);
            } else {
                configured_count += 1;
            }
        }
    }

    // Also configure agents that are enabled but not detected (create files)
    for &agent_id in &enabled_agents {
        let already_configured = detected.iter().any(|d| d.id == agent_id);
        if !already_configured {
            let config = AgentConfig::for_agent(agent_id);
            let agent = DetectedAgent {
                id: agent_id,
                config,
                detected_signals: vec![],
            };
            if let Err(e) = configure_agent(&cwd, &agent) {
                eprintln!("Warning: Failed to configure {}: {}", agent_id.name(), e);
            } else {
                println!("Created config for: {}", agent_id.name());
                configured_count += 1;
            }
        }
    }

    if configured_count > 0 {
        println!("Configured {} agent(s)", configured_count);
    }

    // Update .gitignore
    let gitignore_path = cwd.join(".gitignore");
    gitignore::update_gitignore(&gitignore_path)?;
    println!("Updated .gitignore");

    // Process historical logs
    if !skip_history {
        println!("Scanning for historical logs...");
        // TODO: Implement historical log ingestion (ADR-011)
        println!("Historical log ingestion not yet implemented");
    }

    println!("Squirrel initialized successfully!");
    Ok(())
}

/// Configure an agent for Squirrel.
fn configure_agent(project_root: &PathBuf, agent: &DetectedAgent) -> Result<(), Error> {
    // Configure MCP
    if let Some(ref mcp_path) = agent.config.mcp_config_path {
        let full_path = project_root.join(mcp_path);
        let backup_path = project_root.join(format!("{}.sqrl.bak", mcp_path.display()));

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Use appropriate merge function based on file extension
        let ext = mcp_path.extension().and_then(|e| e.to_str());
        match ext {
            Some("json") => mcp_config::merge_json_mcp_config(&full_path, &backup_path)?,
            Some("toml") => mcp_config::merge_toml_mcp_config(&full_path, &backup_path)?,
            _ => {
                return Err(Error::other(format!(
                    "Unsupported MCP config format: {:?}",
                    mcp_path
                )));
            }
        }
    }

    // Configure instructions files
    for rules_file in &agent.config.rules_files {
        let full_path = project_root.join(rules_file);
        let backup_path = project_root.join(format!("{}.sqrl.bak", rules_file.display()));
        instructions::update_instructions_file(&full_path, &backup_path)?;
    }

    Ok(())
}

/// Get enabled agents from config.
fn get_enabled_agents(config: &Config) -> Vec<AgentId> {
    let mut enabled = Vec::new();

    if config.agents.claude {
        enabled.push(AgentId::Claude);
    }
    if config.agents.cursor {
        enabled.push(AgentId::Cursor);
    }
    if config.agents.codex_cli {
        enabled.push(AgentId::CodexCli);
    }
    if config.agents.gemini {
        enabled.push(AgentId::Gemini);
    }

    enabled
}

fn generate_project_id(path: &PathBuf) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_project_id() {
        let path = PathBuf::from("/home/user/projects/my-project");
        assert_eq!(generate_project_id(&path), "my-project");
    }

    #[test]
    fn test_get_enabled_agents() {
        let config = Config::default();
        let enabled = get_enabled_agents(&config);
        // Default config has claude and cursor enabled
        assert!(enabled.contains(&AgentId::Claude));
        assert!(enabled.contains(&AgentId::Cursor));
    }
}
