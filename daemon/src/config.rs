//! Configuration management for Squirrel.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::Error;

/// Global Squirrel configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub agents: AgentsConfig,

    #[serde(default)]
    pub llm: LlmConfig,

    #[serde(default)]
    pub daemon: DaemonConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentsConfig {
    #[serde(default = "default_true")]
    pub claude: bool,

    #[serde(default = "default_true")]
    pub cursor: bool,

    #[serde(default)]
    pub codex_cli: bool,

    #[serde(default)]
    pub gemini: bool,

    #[serde(default)]
    pub copilot: bool,

    #[serde(default)]
    pub windsurf: bool,
}

impl Default for AgentsConfig {
    fn default() -> Self {
        Self {
            claude: true,
            cursor: true,
            codex_cli: false,
            gemini: false,
            copilot: false,
            windsurf: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    #[serde(default = "default_strong_model")]
    pub strong_model: String,

    #[serde(default = "default_fast_model")]
    pub fast_model: String,

    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,

    /// OpenRouter API key (for openrouter/* models)
    #[serde(default)]
    pub openrouter_api_key: String,

    /// OpenAI API key (for openai/* models and embeddings)
    #[serde(default)]
    pub openai_api_key: String,

    /// Anthropic API key (for anthropic/* models)
    #[serde(default)]
    pub anthropic_api_key: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            strong_model: default_strong_model(),
            fast_model: default_fast_model(),
            embedding_model: default_embedding_model(),
            openrouter_api_key: String::new(),
            openai_api_key: String::new(),
            anthropic_api_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_socket_path")]
    pub socket_path: String,

    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            log_level: default_log_level(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_strong_model() -> String {
    "openrouter/google/gemini-3-pro-preview".to_string()
}

fn default_fast_model() -> String {
    "openrouter/google/gemini-2.5-flash-preview".to_string()
}

fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}

fn default_socket_path() -> String {
    "/tmp/sqrl_agent.sock".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Config {
    /// Load config from ~/.sqrl/config.toml
    pub fn load() -> Result<Self, Error> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save config to ~/.sqrl/config.toml
    pub fn save(&self) -> Result<(), Error> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| Error::Other(e.to_string()))?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Path to global sqrl directory (~/.sqrl/)
    pub fn global_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".sqrl")
    }

    /// Path to config file
    pub fn path() -> PathBuf {
        Self::global_dir().join("config.toml")
    }

    /// Path to global database
    pub fn global_db_path() -> PathBuf {
        Self::global_dir().join("squirrel.db")
    }

    /// Path to projects registry
    pub fn projects_path() -> PathBuf {
        Self::global_dir().join("projects.json")
    }
}

/// Project-level configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_id: String,
    pub root_path: PathBuf,
    pub initialized_at: String,
}

/// Projects registry
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectsRegistry {
    pub projects: Vec<ProjectConfig>,
}

impl ProjectsRegistry {
    pub fn load() -> Result<Self, Error> {
        let path = Config::projects_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let registry: ProjectsRegistry = serde_json::from_str(&content)?;
        Ok(registry)
    }

    pub fn save(&self) -> Result<(), Error> {
        let path = Config::projects_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    pub fn register(&mut self, project: ProjectConfig) {
        // Remove existing entry for same path
        self.projects.retain(|p| p.root_path != project.root_path);
        self.projects.push(project);
    }

    #[allow(dead_code)]
    pub fn find_by_path(&self, path: &std::path::Path) -> Option<&ProjectConfig> {
        self.projects.iter().find(|p| p.root_path == path)
    }
}
