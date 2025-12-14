//! MCP configuration merge (INIT-001-B).
//!
//! Merges Squirrel MCP server into agent MCP configurations.

use std::path::Path;

use serde_json::{json, Value};

use crate::error::Error;

/// Squirrel MCP server configuration.
pub fn squirrel_server_entry() -> Value {
    json!({
        "command": "sqrl",
        "args": ["mcp"]
    })
}

/// Merge Squirrel server into JSON MCP config.
///
/// Handles formats:
/// - `.mcp.json` (Claude Code): `{ "mcpServers": { "squirrel": {...} } }`
/// - `.cursor/mcp.json` (Cursor): `{ "mcpServers": { "squirrel": {...} } }`
pub fn merge_json_mcp_config(path: &Path, backup_path: &Path) -> Result<(), Error> {
    let content = if path.exists() {
        // Create backup
        std::fs::copy(path, backup_path)?;
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    let mut config: Value = if content.is_empty() {
        json!({})
    } else {
        serde_json::from_str(&content)?
    };

    // Ensure mcpServers object exists
    if !config.is_object() {
        config = json!({});
    }

    let obj = config.as_object_mut().unwrap();
    if !obj.contains_key("mcpServers") {
        obj.insert("mcpServers".to_string(), json!({}));
    }

    // Add/update squirrel entry
    let mcp_servers = obj.get_mut("mcpServers").unwrap().as_object_mut().unwrap();
    mcp_servers.insert("squirrel".to_string(), squirrel_server_entry());

    // Write back with pretty formatting
    let output = serde_json::to_string_pretty(&config)?;
    std::fs::write(path, output)?;

    Ok(())
}

/// Merge Squirrel server into TOML MCP config (Codex CLI).
///
/// Format: `[mcp_servers.squirrel]`
pub fn merge_toml_mcp_config(path: &Path, backup_path: &Path) -> Result<(), Error> {
    let content = if path.exists() {
        std::fs::copy(path, backup_path)?;
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    // Parse TOML
    let mut config: toml::Value = if content.is_empty() {
        toml::Value::Table(toml::map::Map::new())
    } else {
        content
            .parse()
            .map_err(|e: toml::de::Error| Error::other(e.to_string()))?
    };

    // Ensure mcp_servers table exists
    let table = config.as_table_mut().unwrap();
    if !table.contains_key("mcp_servers") {
        table.insert(
            "mcp_servers".to_string(),
            toml::Value::Table(toml::map::Map::new()),
        );
    }

    // Add squirrel entry
    let mcp_servers = table
        .get_mut("mcp_servers")
        .unwrap()
        .as_table_mut()
        .unwrap();

    let mut squirrel = toml::map::Map::new();
    squirrel.insert(
        "command".to_string(),
        toml::Value::String("sqrl".to_string()),
    );
    squirrel.insert(
        "args".to_string(),
        toml::Value::Array(vec![toml::Value::String("mcp".to_string())]),
    );
    mcp_servers.insert("squirrel".to_string(), toml::Value::Table(squirrel));

    // Write back
    let output = toml::to_string_pretty(&config).map_err(|e| Error::other(e.to_string()))?;
    std::fs::write(path, output)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_merge_json_new_file() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join(".mcp.json");
        let backup_path = temp.path().join(".mcp.json.sqrl.bak");

        merge_json_mcp_config(&config_path, &backup_path).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let config: Value = serde_json::from_str(&content).unwrap();

        assert!(config["mcpServers"]["squirrel"]["command"].as_str() == Some("sqrl"));
    }

    #[test]
    fn test_merge_json_existing_file() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join(".mcp.json");
        let backup_path = temp.path().join(".mcp.json.sqrl.bak");

        // Create existing config with another server
        let existing = json!({
            "mcpServers": {
                "other-server": {
                    "command": "other",
                    "args": []
                }
            }
        });
        std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        merge_json_mcp_config(&config_path, &backup_path).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        let config: Value = serde_json::from_str(&content).unwrap();

        // Should have both servers
        assert!(config["mcpServers"]["squirrel"]["command"].as_str() == Some("sqrl"));
        assert!(config["mcpServers"]["other-server"]["command"].as_str() == Some("other"));

        // Should have backup
        assert!(backup_path.exists());
    }

    #[test]
    fn test_merge_toml_new_file() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("config.toml");
        let backup_path = temp.path().join("config.toml.sqrl.bak");

        merge_toml_mcp_config(&config_path, &backup_path).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[mcp_servers.squirrel]"));
        assert!(content.contains("command = \"sqrl\""));
    }

    #[test]
    fn test_merge_toml_existing_file() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("config.toml");
        let backup_path = temp.path().join("config.toml.sqrl.bak");

        // Create existing config
        std::fs::write(&config_path, "[other_section]\nkey = \"value\"\n").unwrap();

        merge_toml_mcp_config(&config_path, &backup_path).unwrap();

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("[mcp_servers.squirrel]"));
        assert!(content.contains("[other_section]"));
        assert!(backup_path.exists());
    }
}
