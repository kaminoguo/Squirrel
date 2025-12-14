//! .gitignore management (INIT-001-D).
//!
//! Manages Squirrel entries in .gitignore with managed blocks.

use std::path::Path;

use crate::error::Error;

/// Managed block markers.
const BLOCK_START: &str = "# START Squirrel Generated Files";
const BLOCK_END: &str = "# END Squirrel Generated Files";

/// Squirrel gitignore entries.
const SQUIRREL_ENTRIES: &str = "/.sqrl/";

/// Generate full managed block content.
fn managed_block() -> String {
    format!("{}\n{}\n{}", BLOCK_START, SQUIRREL_ENTRIES, BLOCK_END)
}

/// Update or append Squirrel entries to .gitignore.
///
/// Behavior:
/// - If file doesn't exist: create with managed block
/// - If file exists with block: replace content between markers
/// - If file exists without block: append block at end
pub fn update_gitignore(path: &Path) -> Result<(), Error> {
    let content = if path.exists() {
        std::fs::read_to_string(path)?
    } else {
        String::new()
    };

    let new_content = if content.is_empty() {
        managed_block()
    } else if content.contains(BLOCK_START) && content.contains(BLOCK_END) {
        replace_managed_block(&content)?
    } else {
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

/// Remove Squirrel entries from .gitignore.
pub fn remove_from_gitignore(path: &Path) -> Result<bool, Error> {
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
        std::fs::remove_file(path)?;
    } else {
        std::fs::write(path, format!("{}\n", new_content))?;
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_new_gitignore() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(".gitignore");

        update_gitignore(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains(BLOCK_START));
        assert!(content.contains(BLOCK_END));
        assert!(content.contains("/.sqrl/"));
    }

    #[test]
    fn test_append_to_existing() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(".gitignore");

        std::fs::write(&path, "node_modules/\n.env").unwrap();

        update_gitignore(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains(".env"));
        assert!(content.contains(BLOCK_START));
        assert!(content.contains("/.sqrl/"));
    }

    #[test]
    fn test_replace_existing_block() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(".gitignore");

        let existing = format!(
            "node_modules/\n\n{}\nold-entries\n{}\n\n.env",
            BLOCK_START, BLOCK_END
        );
        std::fs::write(&path, &existing).unwrap();

        update_gitignore(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains(".env"));
        assert!(!content.contains("old-entries"));
        assert!(content.contains("/.sqrl/"));
    }

    #[test]
    fn test_remove_from_gitignore() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(".gitignore");

        let content = format!(
            "node_modules/\n\n{}\n/.sqrl/\n{}\n\n.env",
            BLOCK_START, BLOCK_END
        );
        std::fs::write(&path, &content).unwrap();

        let removed = remove_from_gitignore(&path).unwrap();
        assert!(removed);

        let new_content = std::fs::read_to_string(&path).unwrap();
        assert!(!new_content.contains(BLOCK_START));
        assert!(new_content.contains("node_modules/"));
        assert!(new_content.contains(".env"));
    }
}
