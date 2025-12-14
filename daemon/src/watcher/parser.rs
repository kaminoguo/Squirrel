//! Claude Code JSONL log parser.
//!
//! Parses session logs from ~/.claude/projects/<project-hash>/*.jsonl
//! to extract events for IPC-001: ingest_chunk.

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::LazyLock;

use crate::error::Error;

/// Event role (IPC-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    Tool,
    System,
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::User => write!(f, "user"),
            Role::Assistant => write!(f, "assistant"),
            Role::Tool => write!(f, "tool"),
            Role::System => write!(f, "system"),
        }
    }
}

/// Event kind (IPC-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Message,
    ToolCall,
    ToolResult,
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventKind::Message => write!(f, "message"),
            EventKind::ToolCall => write!(f, "tool_call"),
            EventKind::ToolResult => write!(f, "tool_result"),
        }
    }
}

/// User frustration level (for stats).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frustration {
    None,
    Mild,
    Moderate,
    Severe,
}

impl Default for Frustration {
    fn default() -> Self {
        Frustration::None
    }
}

/// Parsed event (IPC-001 format).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// ISO 8601 timestamp.
    pub ts: String,
    /// Event role.
    pub role: Role,
    /// Event kind.
    pub kind: EventKind,
    /// Brief summary of the event content.
    pub summary: String,
    /// Tool name if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    /// File path if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Whether this event indicates an error.
    #[serde(default)]
    pub is_error: bool,
}

/// Parsed session with events and metadata.
#[derive(Debug, Clone)]
pub struct ParsedSession {
    /// Session ID (filename stem).
    pub session_id: String,
    /// Project ID (parent directory name).
    pub project_id: String,
    /// Parsed events.
    pub events: Vec<Event>,
    /// Maximum frustration detected.
    pub max_frustration: Frustration,
    /// Total error count.
    pub error_count: usize,
}

// Maximum length for summaries
const MAX_SUMMARY_LENGTH: usize = 200;

// Frustration detection patterns (case-insensitive)
static SEVERE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(fuck|shit|damn|wtf|ffs)\b").unwrap(),
        Regex::new(r"!!{2,}").unwrap(), // Multiple exclamation marks
    ]
});

static MODERATE_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(finally|ugh|argh|sigh)\b").unwrap(),
        Regex::new(r"(?i)\b(why (won't|doesn't|isn't|can't))").unwrap(),
        Regex::new(r"(?i)\b(still (not|doesn't|won't))").unwrap(),
    ]
});

static MILD_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\b(hmm|hm+)\b").unwrap(),
        Regex::new(r"\?{2,}").unwrap(), // Multiple question marks
    ]
});

// Error detection patterns in tool results
static ERROR_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)error:").unwrap(),
        Regex::new(r"(?i)exception:").unwrap(),
        Regex::new(r"(?i)traceback").unwrap(),
        Regex::new(r"(?i)failed").unwrap(),
        Regex::new(r"(?i)errno").unwrap(),
        Regex::new(r"(?i)permission denied").unwrap(),
        Regex::new(r"(?i)not found").unwrap(),
        Regex::new(r"(?i)syntax error").unwrap(),
    ]
});

/// Detect frustration level from user message text.
fn detect_frustration(text: &str) -> Frustration {
    for pattern in SEVERE_PATTERNS.iter() {
        if pattern.is_match(text) {
            return Frustration::Severe;
        }
    }
    for pattern in MODERATE_PATTERNS.iter() {
        if pattern.is_match(text) {
            return Frustration::Moderate;
        }
    }
    for pattern in MILD_PATTERNS.iter() {
        if pattern.is_match(text) {
            return Frustration::Mild;
        }
    }
    Frustration::None
}

/// Check if tool result indicates an error.
fn is_error_result(text: &str) -> bool {
    ERROR_PATTERNS.iter().any(|p| p.is_match(text))
}

/// Truncate text with ellipsis if too long.
fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len.saturating_sub(3)])
    }
}

/// Extract summary, tool_name, and file from message content blocks.
fn summarize_content(content: &[Value]) -> (String, Option<String>, Option<String>) {
    let mut summary_parts = Vec::new();
    let mut tool_name = None;
    let mut file_path = None;

    for block in content {
        let block_type = block.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match block_type {
            "text" => {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    summary_parts.push(truncate(text, 100));
                }
            }
            "tool_use" => {
                if let Some(name) = block.get("name").and_then(|v| v.as_str()) {
                    tool_name = Some(name.to_string());
                    summary_parts.push(format!("[{}]", name));
                }
                // Extract file path from tool input
                if let Some(input) = block.get("input").and_then(|v| v.as_object()) {
                    if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                        file_path = Some(path.to_string());
                    } else if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                        file_path = Some(path.to_string());
                    }
                }
            }
            "tool_result" => {
                if let Some(result_content) = block.get("content") {
                    let text = match result_content {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    summary_parts.push(truncate(&text, 50));
                }
            }
            _ => {}
        }
    }

    let summary = if summary_parts.is_empty() {
        "(empty)".to_string()
    } else {
        summary_parts.join(" ")
    };

    (summary, tool_name, file_path)
}

/// Parse Role from string.
fn parse_role(role_str: &str) -> Role {
    match role_str {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        "system" => Role::System,
        _ => Role::Assistant,
    }
}

/// Parse a Claude Code JSONL session file.
pub fn parse_session(path: &Path) -> Result<ParsedSession, Error> {
    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let project_id = path
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut events = Vec::new();
    let mut max_frustration = Frustration::None;
    let mut error_count = 0;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let entry: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Check entry type
        let entry_type = entry.get("type").and_then(|v| v.as_str());
        if !matches!(entry_type, Some("user" | "assistant" | "system")) {
            continue;
        }

        // Parse timestamp
        let ts_str = match entry.get("timestamp").and_then(|v| v.as_str()) {
            Some(ts) => ts,
            None => continue,
        };

        // Validate timestamp format
        let _ts: DateTime<Utc> = match ts_str.parse() {
            Ok(ts) => ts,
            Err(_) => {
                // Try alternate format with Z
                match ts_str.replace("Z", "+00:00").parse() {
                    Ok(ts) => ts,
                    Err(_) => continue,
                }
            }
        };

        // Get message content
        let message = match entry.get("message") {
            Some(m) => m,
            None => continue,
        };

        let content = match message.get("content") {
            Some(Value::Array(arr)) => arr.clone(),
            Some(Value::String(s)) => {
                vec![serde_json::json!({"type": "text", "text": s})]
            }
            _ => continue,
        };

        if content.is_empty() {
            continue;
        }

        // Determine role
        let role_str = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or(entry_type.unwrap_or("assistant"));
        let role = parse_role(role_str);

        // Determine event kind
        let has_tool_use = content
            .iter()
            .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_use"));
        let has_tool_result = content
            .iter()
            .any(|b| b.get("type").and_then(|v| v.as_str()) == Some("tool_result"));

        let kind = if has_tool_use {
            EventKind::ToolCall
        } else if has_tool_result {
            EventKind::ToolResult
        } else {
            EventKind::Message
        };

        // Extract summary
        let (summary, tool_name, file_path) = summarize_content(&content);

        // Check for errors in tool results
        let mut is_error = false;
        if kind == EventKind::ToolResult {
            for block in &content {
                if block.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                    if let Some(result_content) = block.get("content") {
                        let text = match result_content {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        if is_error_result(&text) {
                            is_error = true;
                            error_count += 1;
                            break;
                        }
                    }
                }
            }
        }

        // Detect frustration in user messages
        if role == Role::User && kind == EventKind::Message {
            for block in &content {
                if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        let frustration = detect_frustration(text);
                        if frustration > max_frustration {
                            max_frustration = frustration;
                        }
                    }
                }
            }
        }

        // Create event
        let event = Event {
            ts: ts_str.to_string(),
            role,
            kind,
            summary: truncate(&summary, MAX_SUMMARY_LENGTH),
            tool_name,
            file: file_path,
            is_error,
        };
        events.push(event);
    }

    Ok(ParsedSession {
        session_id,
        project_id,
        events,
        max_frustration,
        error_count,
    })
}

/// Find all Claude Code session files for a project.
#[allow(dead_code)] // For historical processing (ADR-011)
pub fn find_sessions(claude_dir: &Path, project_path: Option<&Path>) -> Vec<std::path::PathBuf> {
    let projects_dir = claude_dir.join("projects");
    if !projects_dir.exists() {
        return Vec::new();
    }

    let dirs: Vec<_> = if let Some(project) = project_path {
        // Convert project path to Claude Code's hash format
        let project_hash = format!(
            "-{}",
            project
                .to_string_lossy()
                .replace("/", "-")
                .trim_start_matches('-')
        );
        let project_dir = projects_dir.join(&project_hash);
        if project_dir.exists() {
            vec![project_dir]
        } else {
            Vec::new()
        }
    } else {
        // All projects
        projects_dir
            .read_dir()
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect()
    };

    let mut sessions = Vec::new();
    for dir in dirs {
        if let Ok(entries) = dir.read_dir() {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                    // Exclude agent-* files (sub-conversations)
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if !name.starts_with("agent-") {
                            sessions.push(path);
                        }
                    }
                }
            }
        }
    }

    // Sort by modification time
    sessions.sort_by(|a, b| {
        let a_time = a.metadata().and_then(|m| m.modified()).ok();
        let b_time = b.metadata().and_then(|m| m.modified()).ok();
        a_time.cmp(&b_time)
    });

    sessions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_frustration() {
        assert_eq!(detect_frustration("what the fuck"), Frustration::Severe);
        assert_eq!(
            detect_frustration("finally working!!!"),
            Frustration::Severe
        ); // Multiple ! is severe
        assert_eq!(detect_frustration("finally working"), Frustration::Moderate);
        assert_eq!(detect_frustration("hmm let me think"), Frustration::Mild);
        assert_eq!(detect_frustration("please help"), Frustration::None);
    }

    #[test]
    fn test_is_error_result() {
        assert!(is_error_result("Error: file not found"));
        assert!(is_error_result("Traceback (most recent call last):"));
        assert!(is_error_result("Command failed with exit code 1"));
        assert!(!is_error_result("Success"));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a very long string", 10), "this is...");
    }

    #[test]
    fn test_parse_role() {
        assert_eq!(parse_role("user"), Role::User);
        assert_eq!(parse_role("assistant"), Role::Assistant);
        assert_eq!(parse_role("unknown"), Role::Assistant);
    }
}
