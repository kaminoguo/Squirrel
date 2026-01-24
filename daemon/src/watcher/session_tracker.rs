//! Session tracker for grouping log entries and detecting session boundaries.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

use super::log_parser::{LogEntry, LogParser};

/// Idle timeout for session boundaries.
const IDLE_TIMEOUT_MINUTES: i64 = 30;

/// Event in an episode, normalized for Python service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeEvent {
    /// ISO 8601 timestamp.
    pub ts: String,
    /// Role: user, assistant, tool_result, tool_error.
    pub role: String,
    /// Summarized content (max 200 chars).
    pub content_summary: String,
}

/// State for an active session.
#[derive(Debug, Clone)]
pub struct SessionState {
    /// Session ID from Claude Code.
    pub session_id: String,
    /// Project root directory.
    pub project_root: PathBuf,
    /// Collected events for this session.
    pub events: Vec<EpisodeEvent>,
    /// Last activity timestamp.
    pub last_activity: DateTime<Utc>,
}

impl SessionState {
    /// Create a new session state.
    pub fn new(session_id: String, project_root: PathBuf) -> Self {
        Self {
            session_id,
            project_root,
            events: Vec::new(),
            last_activity: Utc::now(),
        }
    }

    /// Add an event to the session.
    pub fn add_event(&mut self, event: EpisodeEvent) {
        if let Ok(ts) = DateTime::parse_from_rfc3339(&event.ts) {
            self.last_activity = ts.with_timezone(&Utc);
        }
        self.events.push(event);
    }

    /// Check if the session is idle (no activity for IDLE_TIMEOUT_MINUTES).
    pub fn is_idle(&self) -> bool {
        let now = Utc::now();
        let idle_threshold = Duration::minutes(IDLE_TIMEOUT_MINUTES);
        now - self.last_activity > idle_threshold
    }

    /// Check if there are meaningful events to process.
    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }
}

/// Completed session ready for processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletedSession {
    /// Session ID.
    pub session_id: String,
    /// Project root path.
    pub project_root: String,
    /// Project ID (derived from project root).
    pub project_id: String,
    /// Events in the session.
    pub events: Vec<EpisodeEvent>,
}

/// Tracks active sessions and detects boundaries.
pub struct SessionTracker {
    /// Active sessions by session ID.
    sessions: HashMap<String, SessionState>,
    /// Log parser for content summarization.
    parser: LogParser,
}

impl Default for SessionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionTracker {
    /// Create a new session tracker.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            parser: LogParser::new(),
        }
    }

    /// Process a log entry and return any completed sessions.
    pub fn process_entry(&mut self, entry: LogEntry) -> Vec<CompletedSession> {
        let mut completed = Vec::new();

        // Check for idle sessions before processing new entry
        let idle_session_ids: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.is_idle())
            .map(|(id, _)| id.clone())
            .collect();

        for session_id in idle_session_ids {
            if let Some(session) = self.sessions.remove(&session_id) {
                if session.has_events() {
                    completed.push(self.complete_session(session));
                }
            }
        }

        // Process the new entry
        if let Some((session_id, event, cwd)) = self.extract_event(&entry) {
            let session = self
                .sessions
                .entry(session_id.clone())
                .or_insert_with(|| SessionState::new(session_id, PathBuf::from(cwd)));

            session.add_event(event);
        }

        completed
    }

    /// Force flush all sessions (for explicit flush command).
    pub fn flush_all(&mut self) -> Vec<CompletedSession> {
        let sessions: Vec<_> = self.sessions.drain().collect();

        sessions
            .into_iter()
            .filter(|(_, s)| s.has_events())
            .map(|(_, s)| self.complete_session(s))
            .collect()
    }

    /// Force flush a specific session.
    #[allow(dead_code)]
    pub fn flush_session(&mut self, session_id: &str) -> Option<CompletedSession> {
        self.sessions
            .remove(session_id)
            .filter(|s| s.has_events())
            .map(|s| self.complete_session(s))
    }

    /// Check for any idle sessions and return them as completed.
    pub fn check_idle_sessions(&mut self) -> Vec<CompletedSession> {
        let idle_session_ids: Vec<String> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.is_idle())
            .map(|(id, _)| id.clone())
            .collect();

        let mut completed = Vec::new();
        for session_id in idle_session_ids {
            if let Some(session) = self.sessions.remove(&session_id) {
                if session.has_events() {
                    completed.push(self.complete_session(session));
                }
            }
        }
        completed
    }

    /// Get the number of active sessions.
    #[allow(dead_code)]
    pub fn active_session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get active session IDs.
    #[allow(dead_code)]
    pub fn active_session_ids(&self) -> Vec<&str> {
        self.sessions.keys().map(|s| s.as_str()).collect()
    }

    fn extract_event(&self, entry: &LogEntry) -> Option<(String, EpisodeEvent, String)> {
        match entry {
            LogEntry::User {
                session_id,
                timestamp,
                message,
                cwd,
                ..
            } => {
                let content_summary = self.parser.summarize_content(message);

                // Check if this is a tool result
                if let Some(arr) = message.content.as_array() {
                    for block in arr {
                        if let Some("tool_result") = block.get("type").and_then(|t| t.as_str()) {
                            let is_error = block
                                .get("is_error")
                                .and_then(|e| e.as_bool())
                                .unwrap_or(false);

                            return Some((
                                session_id.clone(),
                                EpisodeEvent {
                                    ts: timestamp.clone(),
                                    role: if is_error {
                                        "tool_error".to_string()
                                    } else {
                                        "tool_result".to_string()
                                    },
                                    content_summary,
                                },
                                cwd.clone(),
                            ));
                        }
                    }
                }

                Some((
                    session_id.clone(),
                    EpisodeEvent {
                        ts: timestamp.clone(),
                        role: "user".to_string(),
                        content_summary,
                    },
                    cwd.clone(),
                ))
            }
            LogEntry::Assistant {
                session_id,
                timestamp,
                message,
                cwd,
                ..
            } => {
                let content_summary = self.parser.summarize_content(message);

                Some((
                    session_id.clone(),
                    EpisodeEvent {
                        ts: timestamp.clone(),
                        role: "assistant".to_string(),
                        content_summary,
                    },
                    cwd.clone(),
                ))
            }
            _ => None,
        }
    }

    fn complete_session(&self, session: SessionState) -> CompletedSession {
        let project_id = derive_project_id(&session.project_root);

        info!(
            session_id = %session.session_id,
            project_id = %project_id,
            event_count = session.events.len(),
            "Session completed"
        );

        CompletedSession {
            session_id: session.session_id,
            project_root: session.project_root.to_string_lossy().to_string(),
            project_id,
            events: session.events,
        }
    }
}

/// Derive a project ID from the project root path.
fn derive_project_id(path: &PathBuf) -> String {
    // Use the directory name as project ID
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_idle_detection() {
        let mut session = SessionState::new("test".to_string(), PathBuf::from("/test"));

        // Fresh session should not be idle
        assert!(!session.is_idle());

        // Set last activity to 31 minutes ago
        session.last_activity = Utc::now() - Duration::minutes(31);
        assert!(session.is_idle());
    }

    #[test]
    fn test_project_id_derivation() {
        let path = PathBuf::from("/home/user/projects/my-project");
        assert_eq!(derive_project_id(&path), "my-project");
    }
}
