//! Project configuration (CONFIG-001).
//!
//! Handles loading and saving `.sqrl/config.yaml`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// Project configuration stored in `.sqrl/config.yaml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// AI tools enabled for this project.
    #[serde(default)]
    pub tools: ToolsConfig,

    /// Documentation indexing settings.
    #[serde(default)]
    pub docs: DocsConfig,

    /// Doc debt detection rules.
    #[serde(default)]
    pub doc_rules: DocRulesConfig,

    /// Git hooks behavior.
    #[serde(default)]
    pub hooks: HooksConfig,

    /// Internal state (not user-editable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub internal: Option<InternalConfig>,
}

/// AI tools configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default = "default_true")]
    pub claude_code: bool,
    #[serde(default)]
    pub cursor: bool,
    #[serde(default)]
    pub codex: bool,
}

/// Documentation indexing settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocsConfig {
    /// File extensions to index.
    #[serde(default = "default_extensions")]
    pub extensions: Vec<String>,

    /// Paths to include (relative to project root).
    #[serde(default = "default_include_paths")]
    pub include_paths: Vec<String>,

    /// Paths to exclude.
    #[serde(default = "default_exclude_paths")]
    pub exclude_paths: Vec<String>,
}

/// Doc debt detection rules.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocRulesConfig {
    /// Custom mappings: code pattern -> doc file.
    #[serde(default)]
    pub mappings: Vec<CodeDocMapping>,

    /// Reference patterns to detect.
    #[serde(default)]
    pub reference_patterns: Vec<ReferencePattern>,
}

/// Code to doc mapping rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeDocMapping {
    pub code: String,
    pub doc: String,
}

/// Reference pattern rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencePattern {
    pub pattern: String,
    pub doc: String,
}

/// Git hooks behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    /// Auto-install hooks when git detected.
    #[serde(default = "default_true")]
    pub auto_install: bool,

    /// Block push if doc debt exists (strict mode).
    #[serde(default)]
    pub pre_push_block: bool,
}

/// Internal state (managed by daemon, not user).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalConfig {
    pub watcher_enabled: bool,
    pub initialized_at: String,
}

// Default value functions
fn default_true() -> bool {
    true
}

fn default_extensions() -> Vec<String> {
    vec![
        "md".to_string(),
        "mdc".to_string(),
        "txt".to_string(),
        "rst".to_string(),
    ]
}

fn default_include_paths() -> Vec<String> {
    vec![
        "specs/".to_string(),
        "docs/".to_string(),
        ".claude/".to_string(),
        ".cursor/".to_string(),
    ]
}

fn default_exclude_paths() -> Vec<String> {
    vec![
        "node_modules/".to_string(),
        "target/".to_string(),
        ".git/".to_string(),
        "vendor/".to_string(),
        "dist/".to_string(),
    ]
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            claude_code: true,
            cursor: false,
            codex: false,
        }
    }
}

impl Default for DocsConfig {
    fn default() -> Self {
        Self {
            extensions: default_extensions(),
            include_paths: default_include_paths(),
            exclude_paths: default_exclude_paths(),
        }
    }
}

impl Default for HooksConfig {
    fn default() -> Self {
        Self {
            auto_install: true,
            pre_push_block: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tools: ToolsConfig::default(),
            docs: DocsConfig::default(),
            doc_rules: DocRulesConfig::default(),
            hooks: HooksConfig::default(),
            internal: Some(InternalConfig {
                watcher_enabled: true,
                initialized_at: chrono::Utc::now().to_rfc3339(),
            }),
        }
    }
}

impl Config {
    /// Get the config file path for a project.
    pub fn path(project_root: &Path) -> PathBuf {
        project_root.join(".sqrl").join("config.yaml")
    }

    /// Load config from a project directory.
    pub fn load(project_root: &Path) -> Result<Self, Error> {
        let config_path = Self::path(project_root);
        if !config_path.exists() {
            return Err(Error::ConfigNotFound(config_path));
        }
        let content = fs::read_to_string(&config_path)?;
        let config: Config =
            serde_yaml::from_str(&content).map_err(|e| Error::ConfigParse(e.to_string()))?;
        Ok(config)
    }

    /// Save config to a project directory.
    pub fn save(&self, project_root: &Path) -> Result<(), Error> {
        let config_path = Self::path(project_root);
        let content = serde_yaml::to_string(self).map_err(|e| Error::ConfigParse(e.to_string()))?;

        // Add header comment
        let with_header = format!(
            "# Squirrel project configuration\n# Edit with 'sqrl config' or modify directly\n\n{}",
            content
        );

        fs::write(&config_path, with_header)?;
        Ok(())
    }

    /// Check if watcher is enabled.
    pub fn is_watcher_enabled(&self) -> bool {
        self.internal
            .as_ref()
            .map(|i| i.watcher_enabled)
            .unwrap_or(true)
    }

    /// Set watcher enabled state.
    pub fn set_watcher_enabled(&mut self, enabled: bool) {
        if let Some(ref mut internal) = self.internal {
            internal.watcher_enabled = enabled;
        } else {
            self.internal = Some(InternalConfig {
                watcher_enabled: enabled,
                initialized_at: chrono::Utc::now().to_rfc3339(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.tools.claude_code);
        assert!(!config.tools.cursor);
        assert_eq!(config.docs.extensions, vec!["md", "mdc", "txt", "rst"]);
        assert!(config.hooks.auto_install);
    }

    #[test]
    fn test_save_and_load() {
        let dir = TempDir::new().unwrap();
        let sqrl_dir = dir.path().join(".sqrl");
        fs::create_dir_all(&sqrl_dir).unwrap();

        let config = Config::default();
        config.save(dir.path()).unwrap();

        let loaded = Config::load(dir.path()).unwrap();
        assert!(loaded.tools.claude_code);
        assert!(loaded.is_watcher_enabled());
    }
}
