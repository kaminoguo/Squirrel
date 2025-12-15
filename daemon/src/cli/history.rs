//! Historical log ingestion (ADR-011).
//!
//! Processes historical CLI logs to bootstrap memories.
//! Timeline starts from earliest session log, processes chronologically.

use std::path::{Path, PathBuf};

use crate::db::Database;
use crate::error::Error;
use crate::ipc::{send_ingest_chunk, MEMORY_SERVICE_SOCKET};
use crate::watcher::buffer::{IngestChunkRequest, RecentMemory};
use crate::watcher::commit::commit_ops;
use crate::watcher::parser::{find_sessions, parse_session, ParsedSession};

/// Maximum number of recent memories to include for context.
const MAX_RECENT_MEMORIES: usize = 20;

/// Default chunk size for historical processing.
const CHUNK_SIZE: usize = 50;

/// Result of historical processing.
#[derive(Debug, Default)]
pub struct HistoryResult {
    pub sessions_processed: usize,
    pub events_processed: usize,
    pub memories_created: usize,
    pub errors: Vec<String>,
}

/// Process historical logs for a project (ADR-011).
///
/// Timeline starts from earliest session log, processes chronologically.
/// Each episode is sent to Memory Service via IPC-001, then memories
/// are committed to the database.
pub async fn process_history(
    project_root: &Path,
    project_id: &str,
    db: &Database,
    owner_type: &str,
    owner_id: &str,
) -> Result<HistoryResult, Error> {
    let mut result = HistoryResult::default();

    // Find Claude Code sessions directory
    let claude_dir = dirs::home_dir()
        .ok_or_else(|| Error::other("Cannot determine home directory"))?
        .join(".claude");

    if !claude_dir.exists() {
        tracing::info!("No Claude Code directory found, skipping history");
        return Ok(result);
    }

    // Find sessions for this project
    let sessions = find_sessions(&claude_dir, Some(project_root));
    if sessions.is_empty() {
        tracing::info!("No historical sessions found for project");
        return Ok(result);
    }

    tracing::info!("Found {} historical session files", sessions.len());

    // Parse and sort sessions by earliest timestamp
    let mut parsed_sessions: Vec<(PathBuf, ParsedSession)> = Vec::new();
    for session_path in sessions {
        match parse_session(&session_path) {
            Ok(session) => {
                if !session.events.is_empty() {
                    parsed_sessions.push((session_path, session));
                }
            }
            Err(e) => {
                result
                    .errors
                    .push(format!("Failed to parse {}: {}", session_path.display(), e));
            }
        }
    }

    // Sort by first event timestamp (oldest first)
    parsed_sessions.sort_by(|a, b| {
        let a_ts = a.1.events.first().map(|e| e.ts.as_str()).unwrap_or("");
        let b_ts = b.1.events.first().map(|e| e.ts.as_str()).unwrap_or("");
        a_ts.cmp(b_ts)
    });

    tracing::info!(
        "Processing {} sessions chronologically",
        parsed_sessions.len()
    );

    // Check if Memory Service is available
    let service_available = check_memory_service().await;
    if !service_available {
        tracing::warn!("Memory Service not available, storing events for later processing");
        // Store parsed events in database for later processing
        for (_path, session) in &parsed_sessions {
            result.sessions_processed += 1;
            result.events_processed += session.events.len();
        }
        return Ok(result);
    }

    // Process each session chronologically
    for (_path, session) in parsed_sessions {
        match process_session(&session, project_id, db, owner_type, owner_id, &mut result).await {
            Ok(_) => {
                result.sessions_processed += 1;
            }
            Err(e) => {
                result
                    .errors
                    .push(format!("Session {}: {}", session.session_id, e));
            }
        }
    }

    Ok(result)
}

/// Check if Memory Service is available.
async fn check_memory_service() -> bool {
    match tokio::net::UnixStream::connect(MEMORY_SERVICE_SOCKET).await {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Query recent memories for context.
///
/// Returns the most recently updated memories for a project, converted
/// to RecentMemory format for the Memory Writer.
fn query_recent_memories(db: &Database, project_id: &str) -> Vec<RecentMemory> {
    match db.get_active_memories(project_id, MAX_RECENT_MEMORIES) {
        Ok(memories) => memories
            .into_iter()
            .map(|m| RecentMemory {
                id: m.id,
                kind: m.kind,
                tier: m.tier,
                key: m.key,
                text: m.text,
            })
            .collect(),
        Err(e) => {
            tracing::warn!("Failed to query recent memories: {}", e);
            Vec::new()
        }
    }
}

/// Process a single session's events.
async fn process_session(
    session: &ParsedSession,
    project_id: &str,
    db: &Database,
    owner_type: &str,
    owner_id: &str,
    result: &mut HistoryResult,
) -> Result<(), Error> {
    let events = &session.events;
    let total_events = events.len();
    let mut chunk_index = 0;
    let mut carry_state: Option<String> = None;

    tracing::debug!(
        "Processing session {} with {} events",
        session.session_id,
        total_events
    );

    // Process in chunks
    for chunk_start in (0..total_events).step_by(CHUNK_SIZE) {
        let chunk_end = (chunk_start + CHUNK_SIZE).min(total_events);
        let chunk_events = events[chunk_start..chunk_end].to_vec();
        let chunk_size = chunk_events.len();

        // Query recent memories for context (only for first chunk to avoid repeated queries)
        let recent_memories = if chunk_index == 0 {
            let mems = query_recent_memories(db, project_id);
            if !mems.is_empty() {
                Some(mems)
            } else {
                None
            }
        } else {
            None
        };

        let request = IngestChunkRequest {
            project_id: project_id.to_string(),
            owner_type: owner_type.to_string(),
            owner_id: owner_id.to_string(),
            chunk_index,
            events: chunk_events,
            carry_state: carry_state.take(),
            recent_memories,
        };

        // Send to Memory Service
        match send_ingest_chunk(MEMORY_SERVICE_SOCKET, &request).await {
            Ok(response) => {
                // Update carry state for next chunk
                carry_state = response.carry_state;

                // Commit memory operations to database
                if !response.memories.is_empty() {
                    let commit_result = commit_ops(db, &response.memories, Some(project_id));
                    result.memories_created += commit_result.added + commit_result.updated;
                    tracing::debug!(
                        "Committed {} memories from chunk {} (added={}, updated={}, deprecated={})",
                        commit_result.added + commit_result.updated,
                        chunk_index,
                        commit_result.added,
                        commit_result.updated,
                        commit_result.deprecated
                    );
                }

                result.events_processed += chunk_size;
            }
            Err(e) => {
                tracing::warn!("Failed to process chunk {}: {}", chunk_index, e);
                // Continue with next chunk despite error
            }
        }

        chunk_index += 1;
    }

    Ok(())
}

/// Find project path hash used by Claude Code.
///
/// Claude Code stores sessions in ~/.claude/projects/<hash>/
/// where hash is derived from the project path.
#[allow(dead_code)] // For future project path detection
pub fn project_path_to_hash(project_path: &Path) -> String {
    // Claude Code uses a specific hash format
    format!(
        "-{}",
        project_path
            .to_string_lossy()
            .replace('/', "-")
            .trim_start_matches('-')
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_project_path_to_hash() {
        let path = PathBuf::from("/home/user/projects/my-app");
        let hash = project_path_to_hash(&path);
        assert!(hash.starts_with('-'));
        assert!(hash.contains("home"));
        assert!(hash.contains("my-app"));
    }

    #[test]
    fn test_history_result_default() {
        let result = HistoryResult::default();
        assert_eq!(result.sessions_processed, 0);
        assert_eq!(result.events_processed, 0);
        assert_eq!(result.memories_created, 0);
        assert!(result.errors.is_empty());
    }
}
