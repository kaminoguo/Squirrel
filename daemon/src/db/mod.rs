//! SQLite database operations for Squirrel.
//!
//! Implements SCHEMA-001 to SCHEMA-004 from specs/SCHEMAS.md.

mod schema;

pub use schema::{init_db, load_sqlite_vec, Episode, Evidence, Memory, MemoryMetrics};

use rusqlite::Connection;
use std::path::Path;

use crate::error::Error;

/// Database connection wrapper.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open or create database at path.
    pub fn open(path: &Path) -> Result<Self, Error> {
        // Ensure sqlite-vec extension is loaded before opening any connection
        load_sqlite_vec();
        let conn = Connection::open(path)?;
        init_db(&conn)?;
        Ok(Self { conn })
    }

    /// Open in-memory database for testing.
    #[allow(dead_code)]
    pub fn open_memory() -> Result<Self, Error> {
        // Ensure sqlite-vec extension is loaded before opening any connection
        load_sqlite_vec();
        let conn = Connection::open_in_memory()?;
        init_db(&conn)?;
        Ok(Self { conn })
    }

    /// Get connection reference.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    // ========== Memories ==========

    /// Insert a new memory.
    pub fn insert_memory(&self, memory: &Memory) -> Result<(), Error> {
        self.conn.execute(
            r#"
            INSERT INTO memories (
                id, project_id, scope, owner_type, owner_id,
                kind, tier, polarity, key, text,
                status, confidence, expires_at, embedding,
                created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            "#,
            rusqlite::params![
                memory.id,
                memory.project_id,
                memory.scope,
                memory.owner_type,
                memory.owner_id,
                memory.kind,
                memory.tier,
                memory.polarity,
                memory.key,
                memory.text,
                memory.status,
                memory.confidence,
                memory.expires_at,
                memory.embedding,
                memory.created_at,
                memory.updated_at,
            ],
        )?;
        Ok(())
    }

    /// Get memory by ID.
    pub fn get_memory(&self, id: &str) -> Result<Option<Memory>, Error> {
        let mut stmt = self.conn.prepare("SELECT * FROM memories WHERE id = ?1")?;
        let mut rows = stmt.query([id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(Memory::from_row(row)?))
        } else {
            Ok(None)
        }
    }

    /// Get active memories for a project.
    pub fn get_active_memories(
        &self,
        project_id: &str,
        limit: usize,
    ) -> Result<Vec<Memory>, Error> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT * FROM memories
            WHERE project_id = ?1 AND status IN ('provisional', 'active')
            ORDER BY updated_at DESC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map([project_id, &limit.to_string()], Memory::from_row)?;

        let mut memories = Vec::new();
        for row in rows {
            memories.push(row?);
        }
        Ok(memories)
    }

    /// Deprecate a memory.
    pub fn deprecate_memory(&self, id: &str) -> Result<(), Error> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE memories SET status = 'deprecated', updated_at = ?1 WHERE id = ?2",
            [&now, id],
        )?;
        Ok(())
    }

    /// Update memory status/tier.
    #[allow(dead_code)] // For CR-Memory evaluation (FLOW-004)
    pub fn update_memory_status(
        &self,
        id: &str,
        status: &str,
        tier: Option<&str>,
        expires_at: Option<&str>,
    ) -> Result<(), Error> {
        let now = chrono::Utc::now().to_rfc3339();

        if let Some(t) = tier {
            self.conn.execute(
                r#"
                UPDATE memories
                SET status = ?1, tier = ?2, expires_at = ?3, updated_at = ?4
                WHERE id = ?5
                "#,
                rusqlite::params![status, t, expires_at, now, id],
            )?;
        } else {
            self.conn.execute(
                r#"
                UPDATE memories
                SET status = ?1, expires_at = ?2, updated_at = ?3
                WHERE id = ?4
                "#,
                rusqlite::params![status, expires_at, now, id],
            )?;
        }
        Ok(())
    }

    // ========== Memory Metrics ==========

    /// Initialize metrics for a memory.
    pub fn init_metrics(&self, memory_id: &str) -> Result<(), Error> {
        self.conn.execute(
            "INSERT OR IGNORE INTO memory_metrics (memory_id) VALUES (?1)",
            [memory_id],
        )?;
        Ok(())
    }

    /// Increment use count.
    pub fn increment_use_count(&self, memory_id: &str) -> Result<(), Error> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"
            UPDATE memory_metrics
            SET use_count = use_count + 1, last_used_at = ?1
            WHERE memory_id = ?2
            "#,
            [&now, memory_id],
        )?;
        Ok(())
    }

    /// Increment opportunities.
    pub fn increment_opportunities(&self, memory_id: &str) -> Result<(), Error> {
        self.conn.execute(
            "UPDATE memory_metrics SET opportunities = opportunities + 1 WHERE memory_id = ?1",
            [memory_id],
        )?;
        Ok(())
    }

    /// Get metrics for a memory.
    #[allow(dead_code)] // For CR-Memory evaluation (FLOW-004)
    pub fn get_metrics(&self, memory_id: &str) -> Result<Option<MemoryMetrics>, Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM memory_metrics WHERE memory_id = ?1")?;
        let mut rows = stmt.query([memory_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(MemoryMetrics::from_row(row)?))
        } else {
            Ok(None)
        }
    }

    // ========== Episodes ==========

    /// Insert an episode.
    #[allow(dead_code)] // For historical processing (ADR-011)
    pub fn insert_episode(&self, episode: &Episode) -> Result<(), Error> {
        self.conn.execute(
            r#"
            INSERT INTO episodes (id, project_id, start_ts, end_ts, events_json, processed, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            rusqlite::params![
                episode.id,
                episode.project_id,
                episode.start_ts,
                episode.end_ts,
                episode.events_json,
                episode.processed,
                episode.created_at,
            ],
        )?;
        Ok(())
    }

    /// Get unprocessed episodes.
    #[allow(dead_code)] // For historical processing (ADR-011)
    pub fn get_unprocessed_episodes(&self, project_id: &str) -> Result<Vec<Episode>, Error> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT * FROM episodes
            WHERE project_id = ?1 AND processed = 0
            ORDER BY start_ts ASC
            "#,
        )?;
        let rows = stmt.query_map([project_id], Episode::from_row)?;

        let mut episodes = Vec::new();
        for row in rows {
            episodes.push(row?);
        }
        Ok(episodes)
    }

    /// Mark episode as processed.
    #[allow(dead_code)] // For historical processing (ADR-011)
    pub fn mark_episode_processed(&self, id: &str) -> Result<(), Error> {
        self.conn
            .execute("UPDATE episodes SET processed = 1 WHERE id = ?1", [id])?;
        Ok(())
    }

    // ========== Evidence ==========

    /// Insert evidence linking memory to episode.
    #[allow(dead_code)] // For memory commit (FLOW-001)
    pub fn insert_evidence(&self, evidence: &Evidence) -> Result<(), Error> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            r#"
            INSERT INTO evidence (id, memory_id, episode_id, source, frustration, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            rusqlite::params![
                evidence.id,
                evidence.memory_id,
                evidence.episode_id,
                evidence.source,
                evidence.frustration,
                now,
            ],
        )?;
        Ok(())
    }

    // ========== Vector Search ==========

    /// Insert or update embedding for a memory.
    #[allow(dead_code)] // Will be used when Memory Service sends embeddings (IPC-002)
    pub fn upsert_memory_embedding(&self, memory_id: &str, embedding: &[f32]) -> Result<(), Error> {
        // Convert f32 slice to bytes (little-endian)
        let embedding_bytes: Vec<u8> = embedding.iter().flat_map(|f| f.to_le_bytes()).collect();

        self.conn.execute(
            r#"
            INSERT INTO vec_memories (memory_id, embedding)
            VALUES (?1, ?2)
            ON CONFLICT(memory_id) DO UPDATE SET embedding = excluded.embedding
            "#,
            rusqlite::params![memory_id, embedding_bytes],
        )?;
        Ok(())
    }

    /// Search memories by vector similarity.
    ///
    /// Returns memory IDs ordered by similarity (most similar first).
    pub fn search_memories_by_vector(
        &self,
        query_embedding: &[f32],
        project_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Memory>, Error> {
        // Convert f32 slice to bytes (little-endian)
        let query_bytes: Vec<u8> = query_embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        // Use vec0 KNN search with optional project filter
        let sql = if project_id.is_some() {
            r#"
            SELECT m.*
            FROM vec_memories v
            JOIN memories m ON m.id = v.memory_id
            WHERE v.embedding MATCH ?1
              AND k = ?2
              AND m.project_id = ?3
              AND m.status IN ('provisional', 'active')
            ORDER BY distance
            "#
        } else {
            r#"
            SELECT m.*
            FROM vec_memories v
            JOIN memories m ON m.id = v.memory_id
            WHERE v.embedding MATCH ?1
              AND k = ?2
              AND m.status IN ('provisional', 'active')
            ORDER BY distance
            "#
        };

        let mut stmt = self.conn.prepare(sql)?;

        let rows = if let Some(pid) = project_id {
            stmt.query_map(
                rusqlite::params![query_bytes, limit as i64, pid],
                Memory::from_row,
            )?
        } else {
            stmt.query_map(
                rusqlite::params![query_bytes, limit as i64],
                Memory::from_row,
            )?
        };

        let mut memories = Vec::new();
        for row in rows {
            memories.push(row?);
        }
        Ok(memories)
    }
}
