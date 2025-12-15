//! Sync all projects with CLI configs (CLI-003).

use std::path::PathBuf;

use crate::config::{Config, ProjectsRegistry};
use crate::error::Error;

use super::agents::{detect_agents, AgentConfig, AgentId, DetectedAgent};
use super::gitignore;
use super::instructions;
use super::mcp_config;

/// Run sync command.
pub async fn run() -> Result<(), Error> {
    println!("Syncing projects...");

    let registry = ProjectsRegistry::load()?;
    let config = Config::load()?;
    let enabled_agents = get_enabled_agents(&config);

    if enabled_agents.is_empty() {
        println!("No agents enabled in config. Enable agents with 'sqrl config'.");
        return Ok(());
    }

    let mut synced_count = 0;
    let mut skipped_count = 0;

    for project in &registry.projects {
        let sqrl_dir = project.root_path.join(".sqrl");

        if !sqrl_dir.exists() {
            println!(
                "  {} - missing .sqrl directory, skipping",
                project.project_id
            );
            skipped_count += 1;
            continue;
        }

        // Sync agent configs (MCP configs, instructions files)
        match sync_project_agents(&project.root_path, &enabled_agents) {
            Ok(count) => {
                if count > 0 {
                    println!("  {} - synced {} agent(s)", project.project_id, count);
                    synced_count += 1;
                } else {
                    println!("  {} - up to date", project.project_id);
                }
            }
            Err(e) => {
                eprintln!("  {} - error: {}", project.project_id, e);
            }
        }
    }

    println!(
        "Sync complete. {} project(s) synced, {} skipped.",
        synced_count, skipped_count
    );
    Ok(())
}

/// Sync agent configs for a project.
fn sync_project_agents(project_root: &PathBuf, enabled_agents: &[AgentId]) -> Result<usize, Error> {
    let detected = detect_agents(project_root);
    let mut configured_count = 0;

    // Configure detected agents that are enabled
    for agent in &detected {
        if enabled_agents.contains(&agent.id) {
            if let Err(e) = configure_agent(project_root, agent) {
                tracing::warn!("Failed to configure {}: {}", agent.id.name(), e);
            } else {
                configured_count += 1;
            }
        }
    }

    // Also configure agents that are enabled but not detected (create files)
    for &agent_id in enabled_agents {
        let already_configured = detected.iter().any(|d| d.id == agent_id);
        if !already_configured {
            let config = AgentConfig::for_agent(agent_id);
            let agent = DetectedAgent {
                id: agent_id,
                config,
                detected_signals: vec![],
            };
            if let Err(e) = configure_agent(project_root, &agent) {
                tracing::warn!("Failed to configure {}: {}", agent_id.name(), e);
            } else {
                configured_count += 1;
            }
        }
    }

    // Update .gitignore
    let gitignore_path = project_root.join(".gitignore");
    gitignore::update_gitignore(&gitignore_path)?;

    Ok(configured_count)
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
