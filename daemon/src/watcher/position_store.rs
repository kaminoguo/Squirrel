//! Position store for tracking processed log file positions.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::Error;

/// Stored position data for a single file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilePosition {
    /// Byte position in the file.
    pub position: u64,
    /// File size at last read (for detecting truncation).
    pub file_size: u64,
}

/// Persistent store for tracking processed positions in log files.
#[derive(Debug)]
pub struct PositionStore {
    /// Path to the store file.
    store_path: PathBuf,
    /// In-memory positions, keyed by file path.
    positions: HashMap<PathBuf, FilePosition>,
}

impl PositionStore {
    /// Create or load a position store.
    pub fn new(store_path: PathBuf) -> Result<Self, Error> {
        let positions = if store_path.exists() {
            let data = fs::read_to_string(&store_path)?;
            serde_json::from_str(&data).unwrap_or_else(|e| {
                warn!(error = %e, "Failed to parse position store, starting fresh");
                HashMap::new()
            })
        } else {
            HashMap::new()
        };

        Ok(Self {
            store_path,
            positions,
        })
    }

    /// Get the default store path.
    pub fn default_path() -> Result<PathBuf, Error> {
        let home = dirs::home_dir().ok_or(Error::HomeDirNotFound)?;
        Ok(home.join(".sqrl").join("positions.json"))
    }

    /// Get the stored position for a file.
    #[allow(dead_code)]
    pub fn get_position(&self, path: &Path) -> Option<&FilePosition> {
        self.positions.get(path)
    }

    /// Get the byte position to start reading from.
    ///
    /// Returns 0 if the file was truncated or not seen before.
    pub fn get_start_position(&self, path: &Path) -> Result<u64, Error> {
        let current_size = fs::metadata(path)?.len();

        if let Some(stored) = self.positions.get(path) {
            if current_size < stored.file_size {
                // File was truncated, start from beginning
                info!(path = %path.display(), "File truncated, restarting from beginning");
                return Ok(0);
            }
            if current_size == stored.file_size {
                // No new content
                return Ok(stored.position);
            }
            // File grew, continue from stored position
            Ok(stored.position)
        } else {
            // New file, start from beginning
            Ok(0)
        }
    }

    /// Update the position for a file.
    pub fn set_position(&mut self, path: PathBuf, position: u64) -> Result<(), Error> {
        let file_size = fs::metadata(&path)?.len();

        self.positions.insert(
            path.clone(),
            FilePosition {
                position,
                file_size,
            },
        );

        debug!(path = %path.display(), position, "Updated position");
        Ok(())
    }

    /// Save the store to disk.
    pub fn save(&self) -> Result<(), Error> {
        // Ensure parent directory exists
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = serde_json::to_string_pretty(&self.positions)?;
        fs::write(&self.store_path, data)?;

        debug!(path = %self.store_path.display(), "Saved position store");
        Ok(())
    }

    /// Remove position data for a file.
    #[allow(dead_code)]
    pub fn remove(&mut self, path: &Path) {
        self.positions.remove(path);
    }

    /// Clear all positions.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.positions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_position_store() {
        let temp_store = NamedTempFile::new().unwrap();
        let store_path = temp_store.path().to_path_buf();

        let mut store = PositionStore::new(store_path.clone()).unwrap();

        // Create a test file
        let mut test_file = NamedTempFile::new().unwrap();
        writeln!(test_file, "line 1").unwrap();
        writeln!(test_file, "line 2").unwrap();
        test_file.flush().unwrap();

        let test_path = test_file.path().to_path_buf();

        // Initially should return 0
        let pos = store.get_start_position(&test_path).unwrap();
        assert_eq!(pos, 0);

        // Set position
        store.set_position(test_path.clone(), 10).unwrap();

        // Should return stored position
        let pos = store.get_start_position(&test_path).unwrap();
        assert_eq!(pos, 10);

        // Save and reload
        store.save().unwrap();
        let store2 = PositionStore::new(store_path).unwrap();
        let pos = store2.get_start_position(&test_path).unwrap();
        assert_eq!(pos, 10);
    }
}
