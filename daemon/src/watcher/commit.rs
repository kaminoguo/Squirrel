//! Memory operation commit handler.
//!
//! Processes memory operations from IPC-001 ingest_chunk responses
//! and commits them to the SQLite database.

use chrono::Utc;
use uuid::Uuid;

use crate::db::{Database, Memory};
use crate::error::Error;
use crate::watcher::buffer::{MemoryOp, MemoryOpKind};

/// Calculate expiry timestamp from TTL days.
fn calculate_expiry(ttl_days: Option<u32>) -> Option<String> {
    ttl_days.map(|days| {
        let expiry = Utc::now() + chrono::Duration::days(days as i64);
        expiry.to_rfc3339()
    })
}

/// Create a new Memory from a MemoryOp.
fn memory_from_op(op: &MemoryOp, project_id: Option<&str>) -> Memory {
    let now = Utc::now().to_rfc3339();
    Memory {
        id: Uuid::new_v4().to_string(),
        project_id: project_id.map(|s| s.to_string()),
        scope: op.scope.clone(),
        owner_type: op.owner_type.clone(),
        owner_id: op.owner_id.clone(),
        kind: op.kind.clone(),
        tier: op.tier.clone(),
        polarity: op.polarity,
        key: op.key.clone(),
        text: op.text.clone(),
        status: "provisional".to_string(),
        confidence: Some(op.confidence),
        expires_at: calculate_expiry(op.ttl_days),
        embedding: None, // Embeddings generated later by Python service
        created_at: now.clone(),
        updated_at: now,
    }
}

/// Commit result with statistics.
#[derive(Debug, Default)]
pub struct CommitResult {
    pub added: usize,
    pub updated: usize,
    pub deprecated: usize,
    pub errors: Vec<String>,
}

/// Process and commit memory operations to the database.
///
/// # Arguments
/// * `db` - Database connection
/// * `ops` - Memory operations from IPC-001 response
/// * `project_id` - Optional project ID for scoping (extracted from session)
///
/// # Returns
/// CommitResult with counts and any errors encountered.
pub fn commit_ops(db: &Database, ops: &[MemoryOp], project_id: Option<&str>) -> CommitResult {
    let mut result = CommitResult::default();

    for op in ops {
        let commit_result = match op.op {
            MemoryOpKind::Add => commit_add(db, op, project_id),
            MemoryOpKind::Update => commit_update(db, op, project_id),
            MemoryOpKind::Deprecate => commit_deprecate(db, op),
        };

        match commit_result {
            Ok(kind) => match kind {
                MemoryOpKind::Add => result.added += 1,
                MemoryOpKind::Update => result.updated += 1,
                MemoryOpKind::Deprecate => result.deprecated += 1,
            },
            Err(e) => {
                result.errors.push(format!("{:?} failed: {}", op.op, e));
            }
        }
    }

    result
}

/// Commit an ADD operation.
fn commit_add(
    db: &Database,
    op: &MemoryOp,
    project_id: Option<&str>,
) -> Result<MemoryOpKind, Error> {
    let memory = memory_from_op(op, project_id);

    tracing::debug!(
        "ADD memory: {} (kind={}, tier={}, key={:?})",
        memory.id,
        memory.kind,
        memory.tier,
        memory.key
    );

    db.insert_memory(&memory)?;
    db.init_metrics(&memory.id)?;

    Ok(MemoryOpKind::Add)
}

/// Commit an UPDATE operation.
///
/// Deprecates the target memory and adds a new one.
fn commit_update(
    db: &Database,
    op: &MemoryOp,
    project_id: Option<&str>,
) -> Result<MemoryOpKind, Error> {
    // Deprecate the old memory if target specified
    if let Some(ref target_id) = op.target_memory_id {
        tracing::debug!("Deprecating old memory: {}", target_id);
        db.deprecate_memory(target_id)?;
    }

    // Add the new memory
    let memory = memory_from_op(op, project_id);

    tracing::debug!(
        "UPDATE -> ADD memory: {} (replaces {:?})",
        memory.id,
        op.target_memory_id
    );

    db.insert_memory(&memory)?;
    db.init_metrics(&memory.id)?;

    Ok(MemoryOpKind::Update)
}

/// Commit a DEPRECATE operation.
fn commit_deprecate(db: &Database, op: &MemoryOp) -> Result<MemoryOpKind, Error> {
    let target_id = op
        .target_memory_id
        .as_ref()
        .ok_or_else(|| Error::validation("DEPRECATE operation requires target_memory_id"))?;

    tracing::debug!("DEPRECATE memory: {}", target_id);
    db.deprecate_memory(target_id)?;

    Ok(MemoryOpKind::Deprecate)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::watcher::buffer::Evidence;

    fn make_add_op() -> MemoryOp {
        MemoryOp {
            op: MemoryOpKind::Add,
            target_memory_id: None,
            episode_idx: 0,
            scope: "project".to_string(),
            owner_type: "user".to_string(),
            owner_id: "alice".to_string(),
            kind: "pattern".to_string(),
            tier: "short_term".to_string(),
            polarity: -1,
            key: Some("http.client".to_string()),
            text: "Use httpx instead of requests for SSL compatibility".to_string(),
            ttl_days: Some(30),
            confidence: 0.85,
            evidence: Evidence {
                source: "failure_then_success".to_string(),
                frustration: Some("mild".to_string()),
            },
        }
    }

    #[test]
    fn test_memory_from_op() {
        let op = make_add_op();
        let memory = memory_from_op(&op, Some("test-project"));

        assert_eq!(memory.scope, "project");
        assert_eq!(memory.kind, "pattern");
        assert_eq!(memory.polarity, -1);
        assert!(memory.expires_at.is_some());
        assert_eq!(memory.status, "provisional");
    }

    #[test]
    fn test_calculate_expiry() {
        assert!(calculate_expiry(None).is_none());
        assert!(calculate_expiry(Some(30)).is_some());
    }

    #[test]
    fn test_commit_ops_add() {
        let db = Database::open_memory().unwrap();
        let ops = vec![make_add_op()];

        let result = commit_ops(&db, &ops, Some("test-project"));

        assert_eq!(result.added, 1);
        assert_eq!(result.updated, 0);
        assert_eq!(result.deprecated, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_commit_ops_update() {
        let db = Database::open_memory().unwrap();

        // First add a memory
        let add_op = make_add_op();
        let result = commit_ops(&db, &[add_op], Some("test-project"));
        assert_eq!(result.added, 1);

        // Get the memory ID
        let memories = db.get_active_memories("test-project", 10).unwrap();
        assert_eq!(memories.len(), 1);
        let old_id = memories[0].id.clone();

        // Now update it
        let mut update_op = make_add_op();
        update_op.op = MemoryOpKind::Update;
        update_op.target_memory_id = Some(old_id.clone());
        update_op.text = "Updated: Use httpx instead of requests".to_string();

        let result = commit_ops(&db, &[update_op], Some("test-project"));
        assert_eq!(result.updated, 1);
        assert!(result.errors.is_empty());

        // Check old memory is deprecated
        let old_memory = db.get_memory(&old_id).unwrap().unwrap();
        assert_eq!(old_memory.status, "deprecated");

        // Check new memory exists
        let memories = db.get_active_memories("test-project", 10).unwrap();
        assert_eq!(memories.len(), 1);
        assert_ne!(memories[0].id, old_id);
    }

    #[test]
    fn test_commit_ops_deprecate() {
        let db = Database::open_memory().unwrap();

        // First add a memory
        let add_op = make_add_op();
        commit_ops(&db, &[add_op], Some("test-project"));

        let memories = db.get_active_memories("test-project", 10).unwrap();
        let memory_id = memories[0].id.clone();

        // Deprecate it
        let deprecate_op = MemoryOp {
            op: MemoryOpKind::Deprecate,
            target_memory_id: Some(memory_id.clone()),
            episode_idx: 0,
            scope: "project".to_string(),
            owner_type: "user".to_string(),
            owner_id: "alice".to_string(),
            kind: "pattern".to_string(),
            tier: "short_term".to_string(),
            polarity: 0,
            key: None,
            text: String::new(),
            ttl_days: None,
            confidence: 0.0,
            evidence: Evidence {
                source: "manual".to_string(),
                frustration: None,
            },
        };

        let result = commit_ops(&db, &[deprecate_op], Some("test-project"));
        assert_eq!(result.deprecated, 1);

        // Check memory is deprecated
        let memory = db.get_memory(&memory_id).unwrap().unwrap();
        assert_eq!(memory.status, "deprecated");

        // Should no longer appear in active memories
        let memories = db.get_active_memories("test-project", 10).unwrap();
        assert!(memories.is_empty());
    }

    #[test]
    fn test_commit_ops_deprecate_missing_target() {
        let db = Database::open_memory().unwrap();

        let deprecate_op = MemoryOp {
            op: MemoryOpKind::Deprecate,
            target_memory_id: None, // Missing!
            episode_idx: 0,
            scope: "project".to_string(),
            owner_type: "user".to_string(),
            owner_id: "alice".to_string(),
            kind: "pattern".to_string(),
            tier: "short_term".to_string(),
            polarity: 0,
            key: None,
            text: String::new(),
            ttl_days: None,
            confidence: 0.0,
            evidence: Evidence {
                source: "manual".to_string(),
                frustration: None,
            },
        };

        let result = commit_ops(&db, &[deprecate_op], None);
        assert_eq!(result.deprecated, 0);
        assert_eq!(result.errors.len(), 1);
    }
}
