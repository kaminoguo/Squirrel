//! SQLite storage for reading memories.
//!
//! SCHEMA-001: user_styles in ~/.sqrl/user_style.db
//! SCHEMA-002: project_memories in <repo>/.sqrl/memory.db

use std::path::{Path, PathBuf};

use rusqlite::{Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};

use crate::error::Error;

/// User style preference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStyle {
    pub id: String,
    pub text: String,
    pub use_count: i64,
}

/// Project-specific memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMemory {
    pub id: String,
    pub category: String,
    pub subcategory: String,
    pub text: String,
    pub use_count: i64,
}

/// Get the user styles database path.
fn user_styles_db_path() -> Result<PathBuf, Error> {
    let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
    Ok(home.join(".sqrl").join("user_style.db"))
}

/// Get the project memories database path.
fn project_memories_db_path(project_root: &Path) -> PathBuf {
    project_root.join(".sqrl").join("memory.db")
}

/// Read all user styles, ordered by use_count DESC.
pub fn get_user_styles() -> Result<Vec<UserStyle>, Error> {
    let db_path = user_styles_db_path()?;

    if !db_path.exists() {
        return Ok(vec![]);
    }

    let conn = Connection::open(&db_path)?;

    let mut stmt = conn.prepare(
        "SELECT id, text, use_count FROM user_styles ORDER BY use_count DESC",
    )?;

    let styles = stmt
        .query_map([], |row| {
            Ok(UserStyle {
                id: row.get(0)?,
                text: row.get(1)?,
                use_count: row.get(2)?,
            })
        })?
        .collect::<SqliteResult<Vec<_>>>()?;

    Ok(styles)
}

/// Read all project memories, ordered by use_count DESC.
pub fn get_project_memories(project_root: &Path) -> Result<Vec<ProjectMemory>, Error> {
    let db_path = project_memories_db_path(project_root);

    if !db_path.exists() {
        return Ok(vec![]);
    }

    let conn = Connection::open(&db_path)?;

    let mut stmt = conn.prepare(
        "SELECT id, category, subcategory, text, use_count
         FROM project_memories
         ORDER BY use_count DESC",
    )?;

    let memories = stmt
        .query_map([], |row| {
            Ok(ProjectMemory {
                id: row.get(0)?,
                category: row.get(1)?,
                subcategory: row.get(2)?,
                text: row.get(3)?,
                use_count: row.get(4)?,
            })
        })?
        .collect::<SqliteResult<Vec<_>>>()?;

    Ok(memories)
}

/// Read project memories grouped by category.
pub fn get_project_memories_grouped(
    project_root: &Path,
) -> Result<std::collections::HashMap<String, Vec<ProjectMemory>>, Error> {
    let memories = get_project_memories(project_root)?;

    let mut grouped: std::collections::HashMap<String, Vec<ProjectMemory>> =
        std::collections::HashMap::new();

    for memory in memories {
        grouped
            .entry(memory.category.clone())
            .or_default()
            .push(memory);
    }

    Ok(grouped)
}

/// Format project memories as markdown (for MCP response).
pub fn format_memories_as_markdown(project_root: &Path) -> Result<String, Error> {
    let grouped = get_project_memories_grouped(project_root)?;

    if grouped.is_empty() {
        return Ok("No project memories found.".to_string());
    }

    let mut output = String::new();

    // Sort categories for consistent output
    let mut categories: Vec<_> = grouped.keys().collect();
    categories.sort();

    for category in categories {
        if let Some(memories) = grouped.get(category) {
            output.push_str(&format!("## {}\n", category));
            for memory in memories {
                output.push_str(&format!("- {}\n", memory.text));
            }
            output.push('\n');
        }
    }

    Ok(output.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_empty_project_memories() {
        let dir = tempdir().unwrap();
        let result = get_project_memories(dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_format_memories_empty() {
        let dir = tempdir().unwrap();
        let result = format_memories_as_markdown(dir.path());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "No project memories found.");
    }
}
