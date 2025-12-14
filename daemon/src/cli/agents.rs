//! Agent detection and configuration (INIT-001-A).
//!
//! Detects AI coding tools and manages their configuration files.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Known AI agent definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentId {
    Claude,
    Cursor,
    CodexCli,
    Gemini,
    Copilot,
    Windsurf,
    Aider,
    Cline,
    Zed,
}

impl AgentId {
    /// Get all agent IDs.
    pub fn all() -> &'static [AgentId] {
        &[
            AgentId::Claude,
            AgentId::Cursor,
            AgentId::CodexCli,
            AgentId::Gemini,
            AgentId::Copilot,
            AgentId::Windsurf,
            AgentId::Aider,
            AgentId::Cline,
            AgentId::Zed,
        ]
    }

    /// Get v1 supported agents.
    pub fn v1_supported() -> &'static [AgentId] {
        &[
            AgentId::Claude,
            AgentId::Cursor,
            AgentId::CodexCli,
            AgentId::Gemini,
        ]
    }

    /// Get display name.
    pub fn name(&self) -> &'static str {
        match self {
            AgentId::Claude => "Claude Code",
            AgentId::Cursor => "Cursor",
            AgentId::CodexCli => "OpenAI Codex CLI",
            AgentId::Gemini => "Gemini CLI",
            AgentId::Copilot => "GitHub Copilot",
            AgentId::Windsurf => "Windsurf",
            AgentId::Aider => "Aider",
            AgentId::Cline => "Cline",
            AgentId::Zed => "Zed",
        }
    }

    /// Get string ID for config.
    #[allow(dead_code)]
    pub fn id(&self) -> &'static str {
        match self {
            AgentId::Claude => "claude",
            AgentId::Cursor => "cursor",
            AgentId::CodexCli => "codex_cli",
            AgentId::Gemini => "gemini",
            AgentId::Copilot => "copilot",
            AgentId::Windsurf => "windsurf",
            AgentId::Aider => "aider",
            AgentId::Cline => "cline",
            AgentId::Zed => "zed",
        }
    }
}

/// Agent configuration paths and detection signals.
#[derive(Debug)]
pub struct AgentConfig {
    #[allow(dead_code)] // For API completeness
    pub id: AgentId,
    pub detection_signals: Vec<PathBuf>,
    pub rules_files: Vec<PathBuf>,
    pub mcp_config_path: Option<PathBuf>,
}

impl AgentConfig {
    /// Get configuration for an agent.
    pub fn for_agent(id: AgentId) -> Self {
        match id {
            AgentId::Claude => AgentConfig {
                id,
                detection_signals: vec![PathBuf::from("CLAUDE.md"), PathBuf::from(".mcp.json")],
                rules_files: vec![PathBuf::from("CLAUDE.md")],
                mcp_config_path: Some(PathBuf::from(".mcp.json")),
            },
            AgentId::Cursor => AgentConfig {
                id,
                detection_signals: vec![
                    PathBuf::from(".cursor"),
                    PathBuf::from(".cursor/mcp.json"),
                ],
                rules_files: vec![PathBuf::from("AGENTS.md")],
                mcp_config_path: Some(PathBuf::from(".cursor/mcp.json")),
            },
            AgentId::CodexCli => AgentConfig {
                id,
                detection_signals: vec![PathBuf::from(".codex/config.toml")],
                rules_files: vec![PathBuf::from("AGENTS.md")],
                mcp_config_path: Some(PathBuf::from(".codex/config.toml")),
            },
            AgentId::Gemini => AgentConfig {
                id,
                detection_signals: vec![PathBuf::from(".gemini/settings.json")],
                rules_files: vec![PathBuf::from("AGENTS.md")],
                mcp_config_path: Some(PathBuf::from(".gemini/settings.json")),
            },
            AgentId::Copilot => AgentConfig {
                id,
                detection_signals: vec![PathBuf::from(".vscode"), PathBuf::from("AGENTS.md")],
                rules_files: vec![PathBuf::from("AGENTS.md")],
                mcp_config_path: Some(PathBuf::from(".vscode/mcp.json")),
            },
            AgentId::Windsurf => AgentConfig {
                id,
                detection_signals: vec![PathBuf::from(".windsurf")],
                rules_files: vec![PathBuf::from("AGENTS.md")],
                mcp_config_path: Some(PathBuf::from(".windsurf/mcp_config.json")),
            },
            AgentId::Aider => AgentConfig {
                id,
                detection_signals: vec![PathBuf::from(".aider.conf.yml")],
                rules_files: vec![PathBuf::from("AGENTS.md")],
                mcp_config_path: Some(PathBuf::from(".mcp.json")),
            },
            AgentId::Cline => AgentConfig {
                id,
                detection_signals: vec![PathBuf::from(".clinerules")],
                rules_files: vec![PathBuf::from(".clinerules")],
                mcp_config_path: None,
            },
            AgentId::Zed => AgentConfig {
                id,
                detection_signals: vec![PathBuf::from(".zed/settings.json")],
                rules_files: vec![PathBuf::from("AGENTS.md")],
                mcp_config_path: Some(PathBuf::from(".zed/settings.json")),
            },
        }
    }
}

/// Detected agent in a project.
#[derive(Debug)]
pub struct DetectedAgent {
    pub id: AgentId,
    pub config: AgentConfig,
    #[allow(dead_code)] // For debugging and future status display
    pub detected_signals: Vec<PathBuf>,
}

/// Detect agents in a project directory.
pub fn detect_agents(project_root: &Path) -> Vec<DetectedAgent> {
    let mut detected = Vec::new();

    for &id in AgentId::all() {
        let config = AgentConfig::for_agent(id);
        let mut signals_found = Vec::new();

        for signal in &config.detection_signals {
            let full_path = project_root.join(signal);
            if full_path.exists() {
                signals_found.push(signal.clone());
            }
        }

        if !signals_found.is_empty() {
            detected.push(DetectedAgent {
                id,
                config,
                detected_signals: signals_found,
            });
        }
    }

    detected
}

/// Filter agents by enabled list from config.
pub fn filter_enabled(detected: Vec<DetectedAgent>, enabled: &[AgentId]) -> Vec<DetectedAgent> {
    detected
        .into_iter()
        .filter(|d| enabled.contains(&d.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_agent_id_all() {
        assert_eq!(AgentId::all().len(), 9);
    }

    #[test]
    fn test_agent_id_v1_supported() {
        assert_eq!(AgentId::v1_supported().len(), 4);
    }

    #[test]
    fn test_detect_claude() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("CLAUDE.md"), "# Claude").unwrap();

        let detected = detect_agents(temp.path());
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].id, AgentId::Claude);
    }

    #[test]
    fn test_detect_cursor() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir(temp.path().join(".cursor")).unwrap();

        let detected = detect_agents(temp.path());
        assert_eq!(detected.len(), 1);
        assert_eq!(detected[0].id, AgentId::Cursor);
    }

    #[test]
    fn test_detect_multiple() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("CLAUDE.md"), "# Claude").unwrap();
        std::fs::create_dir(temp.path().join(".cursor")).unwrap();

        let detected = detect_agents(temp.path());
        assert_eq!(detected.len(), 2);
    }

    #[test]
    fn test_filter_enabled() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("CLAUDE.md"), "# Claude").unwrap();
        std::fs::create_dir(temp.path().join(".cursor")).unwrap();

        let detected = detect_agents(temp.path());
        let filtered = filter_enabled(detected, &[AgentId::Claude]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, AgentId::Claude);
    }
}
