//! Instructions file management (INIT-001-C).
//!
//! Manages agent instruction files (CLAUDE.md, AGENTS.md) with managed blocks.

use std::path::Path;

use crate::error::Error;

/// Managed block markers.
const BLOCK_START: &str = "<!-- START Squirrel Instructions -->";
const BLOCK_END: &str = "<!-- END Squirrel Instructions -->";

/// Squirrel instructions content.
const SQUIRREL_INSTRUCTIONS: &str = r#"## Squirrel Memory System

This project uses Squirrel for persistent memory across sessions.

**MCP Tools Available:**
- `squirrel_get_task_context` - Get relevant memories before coding tasks
- `squirrel_search_memory` - Search memories by query

**When to Use:**
- Before fixing bugs: call `squirrel_get_task_context` to recall past issues
- Before refactoring: search for architectural decisions
- After completing tasks: memories are extracted automatically from logs"#;

/// Generate full managed block content.
fn managed_block() -> String {
    format!("{}\n{}\n{}", BLOCK_START, SQUIRREL_INSTRUCTIONS, BLOCK_END)
}

/// Update or append Squirrel instructions to a file.
///
/// Behavior:
/// - If file doesn't exist: create with managed block
/// - If file exists with block: replace content between markers
/// - If file exists without block: append block at end
pub fn update_instructions_file(path: &Path, backup_path: &Path) -> Result<(), Error> {
    let content = if path.exists() {
        std::fs::copy(path, backup_path)?;
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    let new_content = if content.is_empty() {
        // Create new file with just the managed block
        managed_block()
    } else if content.contains(BLOCK_START) && content.contains(BLOCK_END) {
        // Replace existing block
        replace_managed_block(&content)?
    } else {
        // Append block at end
        format!("{}\n\n{}", content.trim_end(), managed_block())
    };

    std::fs::write(path, new_content)?;
    Ok(())
}

/// Replace content between managed block markers.
fn replace_managed_block(content: &str) -> Result<String, Error> {
    let start_idx = content
        .find(BLOCK_START)
        .ok_or_else(|| Error::other("Block start marker not found"))?;
    let end_idx = content
        .find(BLOCK_END)
        .ok_or_else(|| Error::other("Block end marker not found"))?;

    if end_idx < start_idx {
        return Err(Error::other("Block markers are in wrong order"));
    }

    let before = &content[..start_idx];
    let after = &content[end_idx + BLOCK_END.len()..];

    Ok(format!("{}{}{}", before, managed_block(), after))
}

/// Remove Squirrel instructions from a file.
pub fn remove_instructions(path: &Path) -> Result<bool, Error> {
    if !path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(path)?;
    if !content.contains(BLOCK_START) || !content.contains(BLOCK_END) {
        return Ok(false);
    }

    let start_idx = content.find(BLOCK_START).unwrap();
    let end_idx = content.find(BLOCK_END).unwrap() + BLOCK_END.len();

    let before = &content[..start_idx];
    let after = &content[end_idx..];

    let new_content = format!("{}{}", before.trim_end(), after.trim_start());
    let new_content = new_content.trim();

    if new_content.is_empty() {
        // File would be empty, delete it
        std::fs::remove_file(path)?;
    } else {
        std::fs::write(path, new_content)?;
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_new_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("CLAUDE.md");
        let backup = temp.path().join("CLAUDE.md.sqrl.bak");

        update_instructions_file(&path, &backup).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains(BLOCK_START));
        assert!(content.contains(BLOCK_END));
        assert!(content.contains("squirrel_get_task_context"));
    }

    #[test]
    fn test_append_to_existing() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("CLAUDE.md");
        let backup = temp.path().join("CLAUDE.md.sqrl.bak");

        // Create existing file
        std::fs::write(&path, "# My Project\n\nExisting content.").unwrap();

        update_instructions_file(&path, &backup).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# My Project"));
        assert!(content.contains("Existing content."));
        assert!(content.contains(BLOCK_START));
        assert!(content.contains(BLOCK_END));

        // Backup should exist
        assert!(backup.exists());
    }

    #[test]
    fn test_replace_existing_block() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("CLAUDE.md");
        let backup = temp.path().join("CLAUDE.md.sqrl.bak");

        // Create file with existing block
        let existing = format!(
            "# Header\n\n{}\nOld content\n{}\n\n# Footer",
            BLOCK_START, BLOCK_END
        );
        std::fs::write(&path, &existing).unwrap();

        update_instructions_file(&path, &backup).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Header"));
        assert!(content.contains("# Footer"));
        assert!(!content.contains("Old content"));
        assert!(content.contains("squirrel_get_task_context"));
    }

    #[test]
    fn test_remove_instructions() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("CLAUDE.md");

        // Create file with block
        let content = format!(
            "# Header\n\n{}\nSquirrel stuff\n{}\n\n# Footer",
            BLOCK_START, BLOCK_END
        );
        std::fs::write(&path, &content).unwrap();

        let removed = remove_instructions(&path).unwrap();
        assert!(removed);

        let new_content = std::fs::read_to_string(&path).unwrap();
        assert!(!new_content.contains(BLOCK_START));
        assert!(new_content.contains("# Header"));
        assert!(new_content.contains("# Footer"));
    }

    #[test]
    fn test_remove_from_empty_result() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("CLAUDE.md");

        // Create file with only the block
        let content = format!("{}\nSquirrel stuff\n{}", BLOCK_START, BLOCK_END);
        std::fs::write(&path, &content).unwrap();

        let removed = remove_instructions(&path).unwrap();
        assert!(removed);

        // File should be deleted
        assert!(!path.exists());
    }
}
