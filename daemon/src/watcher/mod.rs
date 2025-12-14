//! Log file watcher for AI agent interactions.
//!
//! Watches for changes to Claude, Cursor, and other AI tool log files.

pub mod buffer;
pub mod commit;
pub mod parser;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, Mutex};

use crate::config::Config;
use crate::error::Error;

pub use buffer::{EventBuffer, IngestChunkRequest, IngestChunkResponse, MemoryOp, MemoryOpKind};
pub use parser::{Event as ParsedEvent, EventKind, Frustration, ParsedSession, Role};

/// Log watcher for AI agent files.
pub struct LogWatcher {
    config: Config,
    watcher: RecommendedWatcher,
    rx: mpsc::Receiver<Result<Event, notify::Error>>,
    watched_paths: HashMap<PathBuf, String>, // path -> project_id
    /// Event buffer for accumulating parsed events.
    buffer: Arc<Mutex<EventBuffer>>,
}

impl LogWatcher {
    /// Create a new log watcher.
    ///
    /// # Arguments
    /// * `config` - Daemon configuration
    /// * `ops_tx` - Channel to send memory operations for commit
    /// * `owner_type` - Default owner type (e.g., "user")
    /// * `owner_id` - Default owner ID (e.g., username)
    pub fn new(
        config: &Config,
        ops_tx: mpsc::Sender<Vec<MemoryOp>>,
        owner_type: String,
        owner_id: String,
    ) -> Result<Self, Error> {
        let (tx, rx) = mpsc::channel(100);

        let watcher = notify::recommended_watcher(move |res| {
            let _ = tx.blocking_send(res);
        })
        .map_err(|e| Error::Watcher(e.to_string()))?;

        let buffer = EventBuffer::new(ops_tx, owner_type, owner_id);

        Ok(Self {
            config: config.clone(),
            watcher,
            rx,
            watched_paths: HashMap::new(),
            buffer: Arc::new(Mutex::new(buffer)),
        })
    }

    /// Get a handle to the event buffer.
    pub fn buffer(&self) -> Arc<Mutex<EventBuffer>> {
        self.buffer.clone()
    }

    /// Add a project to watch.
    pub fn watch_project(&mut self, project_path: &Path) -> Result<(), Error> {
        let project_id = project_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Watch .claude directory if Claude is enabled
        if self.config.agents.claude {
            let claude_dir = project_path.join(".claude");
            if claude_dir.exists() {
                self.watcher
                    .watch(&claude_dir, RecursiveMode::Recursive)
                    .map_err(|e| Error::Watcher(e.to_string()))?;
                self.watched_paths.insert(claude_dir, project_id.clone());
                tracing::debug!("Watching .claude in {}", project_path.display());
            }
        }

        // Watch .cursor directory if Cursor is enabled
        if self.config.agents.cursor {
            let cursor_dir = project_path.join(".cursor");
            if cursor_dir.exists() {
                self.watcher
                    .watch(&cursor_dir, RecursiveMode::Recursive)
                    .map_err(|e| Error::Watcher(e.to_string()))?;
                self.watched_paths.insert(cursor_dir, project_id.clone());
                tracing::debug!("Watching .cursor in {}", project_path.display());
            }
        }

        // Watch home directory logs for global agent files
        if let Some(home) = dirs::home_dir() {
            // Claude Code projects directory
            if self.config.agents.claude {
                let claude_projects = home.join(".claude").join("projects");
                if claude_projects.exists() {
                    self.watcher
                        .watch(&claude_projects, RecursiveMode::Recursive)
                        .map_err(|e| Error::Watcher(e.to_string()))?;
                    tracing::info!(
                        "Watching Claude Code projects at {}",
                        claude_projects.display()
                    );
                }
            }
        }

        Ok(())
    }

    /// Run the watcher loop.
    pub async fn run(&mut self) {
        while let Some(result) = self.rx.recv().await {
            match result {
                Ok(event) => {
                    self.handle_event(event).await;
                }
                Err(e) => {
                    tracing::error!("Watch error: {}", e);
                }
            }
        }
    }

    async fn handle_event(&self, event: Event) {
        use notify::EventKind;

        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                for path in &event.paths {
                    self.process_file(path).await;
                }
            }
            _ => {}
        }
    }

    async fn process_file(&self, path: &Path) {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return;
        };

        // Only process JSONL files (Claude Code session logs)
        if ext != "jsonl" {
            return;
        }

        // Skip agent-* files (sub-conversations)
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("agent-") {
                return;
            }
        }

        tracing::debug!("File changed: {}", path.display());
        self.parse_session(path).await;
    }

    async fn parse_session(&self, path: &Path) {
        match parser::parse_session(path) {
            Ok(session) => {
                if session.events.is_empty() {
                    tracing::debug!("Session {} has no events, skipping", session.session_id);
                    return;
                }

                tracing::info!(
                    "Parsed session {} ({} events, {} errors, frustration: {:?})",
                    session.session_id,
                    session.events.len(),
                    session.error_count,
                    session.max_frustration
                );

                // Add to buffer
                let mut buffer = self.buffer.lock().await;
                buffer.add_session(
                    session.session_id.clone(),
                    session.project_id.clone(),
                    session.events,
                    session.max_frustration,
                    session.error_count,
                );

                tracing::debug!("Buffer now has {} total events", buffer.total_events());
            }
            Err(e) => {
                tracing::error!("Failed to parse session {}: {}", path.display(), e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_log_watcher_creation() {
        let (ops_tx, _ops_rx) = mpsc::channel(10);
        let config = Config::default();
        let watcher = LogWatcher::new(&config, ops_tx, "user".to_string(), "test".to_string());
        assert!(watcher.is_ok());
    }

    #[tokio::test]
    async fn test_buffer_integration() {
        let (ops_tx, _ops_rx) = mpsc::channel(10);
        let config = Config::default();
        let watcher =
            LogWatcher::new(&config, ops_tx, "user".to_string(), "test".to_string()).unwrap();

        let buffer = watcher.buffer();
        let buffer = buffer.lock().await;
        assert_eq!(buffer.total_events(), 0);
    }
}
