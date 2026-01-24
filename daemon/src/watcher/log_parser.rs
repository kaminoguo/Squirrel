//! JSONL log parser for Claude Code logs.

use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::Error;

/// Content block in assistant messages.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[allow(dead_code)]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Thinking {
        thinking: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
}

/// User message content - can be string or array of content blocks.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
#[allow(dead_code)]
pub enum UserContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

/// Message structure within a log entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub role: String,
    pub content: serde_json::Value,
    #[serde(default)]
    pub model: Option<String>,
}

/// Parsed log entry from Claude Code JSONL.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LogEntry {
    User {
        uuid: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        timestamp: String,
        cwd: String,
        message: Message,
        #[serde(rename = "gitBranch")]
        git_branch: Option<String>,
    },
    Assistant {
        uuid: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        timestamp: String,
        cwd: String,
        message: Message,
        #[serde(rename = "parentUuid")]
        parent_uuid: Option<String>,
    },
    System {
        uuid: String,
        #[serde(rename = "sessionId")]
        session_id: String,
        timestamp: String,
        message: Message,
    },
    Progress {
        uuid: String,
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    Summary {
        uuid: String,
        #[serde(rename = "sessionId")]
        session_id: String,
    },
    FileHistorySnapshot {
        #[serde(rename = "messageId")]
        message_id: String,
    },
    QueueOperation {},
    #[serde(other)]
    Unknown,
}

#[allow(dead_code)]
impl LogEntry {
    /// Get the session ID from the entry, if available.
    pub fn session_id(&self) -> Option<&str> {
        match self {
            LogEntry::User { session_id, .. } => Some(session_id),
            LogEntry::Assistant { session_id, .. } => Some(session_id),
            LogEntry::System { session_id, .. } => Some(session_id),
            LogEntry::Progress { session_id, .. } => Some(session_id),
            LogEntry::Summary { session_id, .. } => Some(session_id),
            _ => None,
        }
    }

    /// Get the timestamp from the entry, if available.
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        let ts_str = match self {
            LogEntry::User { timestamp, .. } => Some(timestamp),
            LogEntry::Assistant { timestamp, .. } => Some(timestamp),
            LogEntry::System { timestamp, .. } => Some(timestamp),
            _ => None,
        }?;

        DateTime::parse_from_rfc3339(ts_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }

    /// Get the working directory from the entry, if available.
    pub fn cwd(&self) -> Option<&str> {
        match self {
            LogEntry::User { cwd, .. } => Some(cwd),
            LogEntry::Assistant { cwd, .. } => Some(cwd),
            _ => None,
        }
    }

    /// Check if this is a meaningful entry for episode processing.
    pub fn is_meaningful(&self) -> bool {
        matches!(self, LogEntry::User { .. } | LogEntry::Assistant { .. })
    }
}

/// JSONL log file parser.
pub struct LogParser {
    max_content_length: usize,
}

impl Default for LogParser {
    fn default() -> Self {
        Self::new()
    }
}

impl LogParser {
    /// Create a new log parser.
    pub fn new() -> Self {
        Self {
            max_content_length: 200,
        }
    }

    /// Parse a single line of JSONL.
    pub fn parse_line(&self, line: &str) -> Result<LogEntry, Error> {
        serde_json::from_str(line).map_err(|e| Error::InvalidLogEntry(e.to_string()))
    }

    /// Read and parse all entries from a file starting at a position.
    pub fn parse_from_position(
        &self,
        path: &Path,
        start_pos: u64,
    ) -> Result<(Vec<LogEntry>, u64), Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        if start_pos > 0 {
            reader.seek(SeekFrom::Start(start_pos))?;
        }

        let mut entries = Vec::new();
        let mut current_pos = start_pos;

        for line in reader.lines() {
            let line = line?;
            current_pos += line.len() as u64 + 1; // +1 for newline

            if line.trim().is_empty() {
                continue;
            }

            match self.parse_line(&line) {
                Ok(entry) => {
                    if entry.is_meaningful() {
                        entries.push(entry);
                    }
                }
                Err(e) => {
                    debug!(error = %e, "Failed to parse log line, skipping");
                }
            }
        }

        Ok((entries, current_pos))
    }

    /// Extract a content summary from a message.
    pub fn summarize_content(&self, message: &Message) -> String {
        let content = &message.content;

        if let Some(text) = content.as_str() {
            return self.truncate(text);
        }

        if let Some(arr) = content.as_array() {
            let mut parts = Vec::new();

            for block in arr {
                if let Some(block_type) = block.get("type").and_then(|t| t.as_str()) {
                    match block_type {
                        "text" => {
                            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                parts.push(self.truncate(text));
                            }
                        }
                        "tool_use" => {
                            if let Some(name) = block.get("name").and_then(|n| n.as_str()) {
                                parts.push(format!("[tool: {}]", name));
                            }
                        }
                        "tool_result" => {
                            let is_error = block
                                .get("is_error")
                                .and_then(|e| e.as_bool())
                                .unwrap_or(false);
                            if is_error {
                                parts.push("[tool_error]".to_string());
                            } else {
                                parts.push("[tool_result]".to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }

            return parts.join(" ");
        }

        "[unknown content]".to_string()
    }

    fn truncate(&self, text: &str) -> String {
        let text = text.trim();
        if text.len() <= self.max_content_length {
            text.to_string()
        } else {
            format!("{}...", &text[..self.max_content_length])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_user_entry() {
        let line = r#"{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/home/test","sessionId":"test-session","version":"2.0.71","gitBranch":"main","type":"user","message":{"role":"user","content":"hello world"},"uuid":"test-uuid","timestamp":"2026-01-06T17:07:10.675Z"}"#;

        let parser = LogParser::new();
        let entry = parser.parse_line(line).unwrap();

        match entry {
            LogEntry::User {
                session_id,
                message,
                ..
            } => {
                assert_eq!(session_id, "test-session");
                assert_eq!(message.role, "user");
            }
            _ => panic!("Expected User entry"),
        }
    }

    #[test]
    fn test_summarize_text_content() {
        let parser = LogParser::new();
        let message = Message {
            role: "user".to_string(),
            content: serde_json::json!("test message"),
            model: None,
        };

        assert_eq!(parser.summarize_content(&message), "test message");
    }
}
