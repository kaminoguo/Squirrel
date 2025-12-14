//! Event buffer for IPC-001: ingest_chunk.
//!
//! Buffers parsed events and sends them to Memory Service for processing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::error::Error;
use crate::watcher::parser::{Event, Frustration};

/// Default chunk size for event batching.
const DEFAULT_CHUNK_SIZE: usize = 50;

/// Memory operation from Memory Service (IPC-001 response).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryOp {
    pub op: MemoryOpKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_memory_id: Option<String>,
    pub episode_idx: usize,
    pub scope: String,
    pub owner_type: String,
    pub owner_id: String,
    pub kind: String,
    pub tier: String,
    pub polarity: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_days: Option<u32>,
    pub confidence: f64,
    pub evidence: Evidence,
}

/// Memory operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MemoryOpKind {
    Add,
    Update,
    Deprecate,
}

/// Evidence for memory derivation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frustration: Option<String>,
}

/// Episode boundary detected by Memory Service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // For IPC-001 response parsing
pub struct EpisodeBoundary {
    pub start_idx: usize,
    pub end_idx: usize,
    pub label: String,
}

/// Recent memory for context (sent to Memory Service).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentMemory {
    pub id: String,
    pub kind: String,
    pub tier: String,
    pub key: Option<String>,
    pub text: String,
}

/// IPC-001 ingest_chunk request parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestChunkRequest {
    pub project_id: String,
    pub owner_type: String,
    pub owner_id: String,
    pub chunk_index: usize,
    pub events: Vec<Event>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carry_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_memories: Option<Vec<RecentMemory>>,
}

/// IPC-001 ingest_chunk response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)] // For IPC-001 response parsing
pub struct IngestChunkResponse {
    pub episodes: Vec<EpisodeBoundary>,
    pub memories: Vec<MemoryOp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carry_state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discard_reason: Option<String>,
}

/// Session state tracking for buffering.
#[derive(Debug)]
struct SessionState {
    project_id: String,
    events: Vec<Event>,
    chunk_index: usize,
    carry_state: Option<String>,
    max_frustration: Frustration,
    error_count: usize,
}

/// Event buffer for accumulating and sending events to Memory Service.
pub struct EventBuffer {
    /// Per-session state.
    sessions: HashMap<String, SessionState>,
    /// Chunk size for batching.
    chunk_size: usize,
    /// Channel to send processed memory ops.
    #[allow(dead_code)] // For IPC integration
    ops_tx: mpsc::Sender<Vec<MemoryOp>>,
    /// Default owner type.
    owner_type: String,
    /// Default owner ID.
    owner_id: String,
}

impl EventBuffer {
    /// Create a new event buffer.
    pub fn new(ops_tx: mpsc::Sender<Vec<MemoryOp>>, owner_type: String, owner_id: String) -> Self {
        Self {
            sessions: HashMap::new(),
            chunk_size: DEFAULT_CHUNK_SIZE,
            ops_tx,
            owner_type,
            owner_id,
        }
    }

    /// Set chunk size for batching.
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.chunk_size = size;
        self
    }

    /// Add events from a parsed session.
    pub fn add_session(
        &mut self,
        session_id: String,
        project_id: String,
        events: Vec<Event>,
        max_frustration: Frustration,
        error_count: usize,
    ) {
        let state = self
            .sessions
            .entry(session_id)
            .or_insert_with(|| SessionState {
                project_id,
                events: Vec::new(),
                chunk_index: 0,
                carry_state: None,
                max_frustration: Frustration::None,
                error_count: 0,
            });

        state.events.extend(events);
        if max_frustration > state.max_frustration {
            state.max_frustration = max_frustration;
        }
        state.error_count += error_count;

        tracing::debug!("Buffer now has {} events for session", state.events.len());
    }

    /// Check if any session has enough events for a chunk.
    pub fn has_ready_chunk(&self) -> bool {
        self.sessions
            .values()
            .any(|s| s.events.len() >= self.chunk_size)
    }

    /// Get sessions that have pending events.
    #[allow(dead_code)] // For IPC integration
    pub fn pending_sessions(&self) -> Vec<String> {
        self.sessions
            .iter()
            .filter(|(_, s)| !s.events.is_empty())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Build an ingest_chunk request for a session.
    ///
    /// Returns None if session has no events.
    pub fn build_chunk_request(
        &mut self,
        session_id: &str,
        recent_memories: Option<Vec<RecentMemory>>,
    ) -> Option<IngestChunkRequest> {
        let state = self.sessions.get_mut(session_id)?;
        if state.events.is_empty() {
            return None;
        }

        // Take up to chunk_size events
        let chunk_events: Vec<Event> = if state.events.len() > self.chunk_size {
            state.events.drain(..self.chunk_size).collect()
        } else {
            state.events.drain(..).collect()
        };

        let request = IngestChunkRequest {
            project_id: state.project_id.clone(),
            owner_type: self.owner_type.clone(),
            owner_id: self.owner_id.clone(),
            chunk_index: state.chunk_index,
            events: chunk_events,
            carry_state: state.carry_state.take(),
            recent_memories,
        };

        state.chunk_index += 1;

        Some(request)
    }

    /// Process response from Memory Service.
    #[allow(dead_code)] // For IPC integration
    pub async fn process_response(
        &mut self,
        session_id: &str,
        response: IngestChunkResponse,
    ) -> Result<(), Error> {
        // Update carry state
        if let Some(state) = self.sessions.get_mut(session_id) {
            state.carry_state = response.carry_state;
        }

        // Send memory ops for commit
        if !response.memories.is_empty() {
            tracing::info!(
                "Received {} memory operations for session {}",
                response.memories.len(),
                session_id
            );
            self.ops_tx
                .send(response.memories)
                .await
                .map_err(|e| Error::ipc(format!("Failed to send memory ops: {}", e)))?;
        }

        // Log discard reason if present
        if let Some(reason) = response.discard_reason {
            tracing::debug!("Chunk discarded: {}", reason);
        }

        // Log episode boundaries
        for episode in &response.episodes {
            tracing::debug!(
                "Episode detected: {} (events {}-{})",
                episode.label,
                episode.start_idx,
                episode.end_idx
            );
        }

        Ok(())
    }

    /// Flush all pending events for a session (end of session).
    #[allow(dead_code)] // For IPC integration
    pub fn flush_session(&mut self, session_id: &str) -> Option<IngestChunkRequest> {
        self.build_chunk_request(session_id, None)
    }

    /// Clear a session from the buffer.
    #[allow(dead_code)] // For IPC integration
    pub fn clear_session(&mut self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    /// Get total buffered event count.
    pub fn total_events(&self) -> usize {
        self.sessions.values().map(|s| s.events.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::parser::{EventKind, Role};

    fn make_event(ts: &str, role: Role, kind: EventKind) -> Event {
        Event {
            ts: ts.to_string(),
            role,
            kind,
            summary: "test".to_string(),
            tool_name: None,
            file: None,
            is_error: false,
        }
    }

    #[test]
    fn test_add_session() {
        let (tx, _rx) = mpsc::channel(10);
        let mut buffer = EventBuffer::new(tx, "user".to_string(), "alice".to_string());

        let events = vec![
            make_event("2024-01-01T10:00:00Z", Role::User, EventKind::Message),
            make_event("2024-01-01T10:01:00Z", Role::Assistant, EventKind::ToolCall),
        ];

        buffer.add_session(
            "session-1".to_string(),
            "project-1".to_string(),
            events,
            Frustration::None,
            0,
        );

        assert_eq!(buffer.total_events(), 2);
    }

    #[test]
    fn test_build_chunk_request() {
        let (tx, _rx) = mpsc::channel(10);
        let mut buffer =
            EventBuffer::new(tx, "user".to_string(), "alice".to_string()).with_chunk_size(2);

        let events = vec![
            make_event("2024-01-01T10:00:00Z", Role::User, EventKind::Message),
            make_event("2024-01-01T10:01:00Z", Role::Assistant, EventKind::ToolCall),
            make_event("2024-01-01T10:02:00Z", Role::Tool, EventKind::ToolResult),
        ];

        buffer.add_session(
            "session-1".to_string(),
            "project-1".to_string(),
            events,
            Frustration::None,
            0,
        );

        // First chunk should have 2 events
        let request = buffer.build_chunk_request("session-1", None).unwrap();
        assert_eq!(request.events.len(), 2);
        assert_eq!(request.chunk_index, 0);

        // Second chunk should have 1 event
        let request = buffer.build_chunk_request("session-1", None).unwrap();
        assert_eq!(request.events.len(), 1);
        assert_eq!(request.chunk_index, 1);
    }

    #[test]
    fn test_has_ready_chunk() {
        let (tx, _rx) = mpsc::channel(10);
        let mut buffer =
            EventBuffer::new(tx, "user".to_string(), "alice".to_string()).with_chunk_size(3);

        let events = vec![make_event(
            "2024-01-01T10:00:00Z",
            Role::User,
            EventKind::Message,
        )];
        buffer.add_session(
            "s1".to_string(),
            "p1".to_string(),
            events,
            Frustration::None,
            0,
        );
        assert!(!buffer.has_ready_chunk());

        let events = vec![
            make_event("2024-01-01T10:01:00Z", Role::User, EventKind::Message),
            make_event("2024-01-01T10:02:00Z", Role::User, EventKind::Message),
        ];
        buffer.add_session(
            "s1".to_string(),
            "p1".to_string(),
            events,
            Frustration::None,
            0,
        );
        assert!(buffer.has_ready_chunk());
    }
}
