//! Database schema definitions for Squirrel.
//!
//! Implements SCHEMA-001 to SCHEMA-004 from specs/SCHEMAS.md.

use rusqlite::{Connection, Result, Row};

use crate::error::Error;

/// Initialize database with all tables.
pub fn init_db(conn: &Connection) -> Result<(), Error> {
    // SCHEMA-001: memories
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS memories (
            id          TEXT PRIMARY KEY,
            project_id  TEXT,
            scope       TEXT NOT NULL,
            owner_type  TEXT NOT NULL,
            owner_id    TEXT NOT NULL,
            kind        TEXT NOT NULL,
            tier        TEXT NOT NULL,
            polarity    INTEGER DEFAULT 1,
            key         TEXT,
            text        TEXT NOT NULL,
            status      TEXT NOT NULL DEFAULT 'provisional',
            confidence  REAL,
            expires_at  TEXT,
            embedding   BLOB,
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_memories_project ON memories(project_id);
        CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope);
        CREATE INDEX IF NOT EXISTS idx_memories_owner ON memories(owner_type, owner_id);
        CREATE INDEX IF NOT EXISTS idx_memories_kind ON memories(kind);
        CREATE INDEX IF NOT EXISTS idx_memories_tier ON memories(tier);
        CREATE INDEX IF NOT EXISTS idx_memories_key ON memories(key);
        CREATE INDEX IF NOT EXISTS idx_memories_status ON memories(status);
        "#,
    )?;

    // SCHEMA-002: evidence
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS evidence (
            id           TEXT PRIMARY KEY,
            memory_id    TEXT NOT NULL,
            episode_id   TEXT NOT NULL,
            source       TEXT,
            frustration  TEXT,
            created_at   TEXT NOT NULL,
            FOREIGN KEY (memory_id) REFERENCES memories(id),
            FOREIGN KEY (episode_id) REFERENCES episodes(id)
        );

        CREATE INDEX IF NOT EXISTS idx_evidence_memory ON evidence(memory_id);
        CREATE INDEX IF NOT EXISTS idx_evidence_episode ON evidence(episode_id);
        "#,
    )?;

    // SCHEMA-003: memory_metrics
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS memory_metrics (
            memory_id              TEXT PRIMARY KEY,
            use_count              INTEGER DEFAULT 0,
            opportunities          INTEGER DEFAULT 0,
            suspected_regret_hits  INTEGER DEFAULT 0,
            estimated_regret_saved REAL DEFAULT 0.0,
            last_used_at           TEXT,
            last_evaluated_at      TEXT,
            FOREIGN KEY (memory_id) REFERENCES memories(id)
        );
        "#,
    )?;

    // SCHEMA-004: episodes
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS episodes (
            id          TEXT PRIMARY KEY,
            project_id  TEXT NOT NULL,
            start_ts    TEXT NOT NULL,
            end_ts      TEXT NOT NULL,
            events_json TEXT NOT NULL,
            processed   INTEGER DEFAULT 0,
            created_at  TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_episodes_project ON episodes(project_id);
        CREATE INDEX IF NOT EXISTS idx_episodes_processed ON episodes(processed);
        CREATE INDEX IF NOT EXISTS idx_episodes_start ON episodes(start_ts);
        "#,
    )?;

    Ok(())
}

/// Memory record (SCHEMA-001).
#[derive(Debug, Clone)]
pub struct Memory {
    pub id: String,
    pub project_id: Option<String>,
    pub scope: String,
    pub owner_type: String,
    pub owner_id: String,
    pub kind: String,
    pub tier: String,
    pub polarity: i32,
    pub key: Option<String>,
    pub text: String,
    pub status: String,
    pub confidence: Option<f64>,
    pub expires_at: Option<String>,
    pub embedding: Option<Vec<u8>>,
    pub created_at: String,
    pub updated_at: String,
}

impl Memory {
    pub fn from_row(row: &Row<'_>) -> Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            scope: row.get("scope")?,
            owner_type: row.get("owner_type")?,
            owner_id: row.get("owner_id")?,
            kind: row.get("kind")?,
            tier: row.get("tier")?,
            polarity: row.get("polarity")?,
            key: row.get("key")?,
            text: row.get("text")?,
            status: row.get("status")?,
            confidence: row.get("confidence")?,
            expires_at: row.get("expires_at")?,
            embedding: row.get("embedding")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// Memory metrics record (SCHEMA-003).
#[derive(Debug, Clone)]
#[allow(dead_code)] // For CR-Memory evaluation (FLOW-004)
pub struct MemoryMetrics {
    pub memory_id: String,
    pub use_count: i32,
    pub opportunities: i32,
    pub suspected_regret_hits: i32,
    pub estimated_regret_saved: f64,
    pub last_used_at: Option<String>,
    pub last_evaluated_at: Option<String>,
}

impl MemoryMetrics {
    #[allow(dead_code)]
    pub fn from_row(row: &Row<'_>) -> Result<Self> {
        Ok(Self {
            memory_id: row.get("memory_id")?,
            use_count: row.get("use_count")?,
            opportunities: row.get("opportunities")?,
            suspected_regret_hits: row.get("suspected_regret_hits")?,
            estimated_regret_saved: row.get("estimated_regret_saved")?,
            last_used_at: row.get("last_used_at")?,
            last_evaluated_at: row.get("last_evaluated_at")?,
        })
    }
}

/// Episode record (SCHEMA-004).
#[derive(Debug, Clone)]
#[allow(dead_code)] // For historical processing (ADR-011)
pub struct Episode {
    pub id: String,
    pub project_id: String,
    pub start_ts: String,
    pub end_ts: String,
    pub events_json: String,
    pub processed: bool,
    pub created_at: String,
}

impl Episode {
    #[allow(dead_code)]
    pub fn from_row(row: &Row<'_>) -> Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            project_id: row.get("project_id")?,
            start_ts: row.get("start_ts")?,
            end_ts: row.get("end_ts")?,
            events_json: row.get("events_json")?,
            processed: row.get::<_, i32>("processed")? != 0,
            created_at: row.get("created_at")?,
        })
    }
}

/// Evidence record (SCHEMA-002).
#[derive(Debug, Clone)]
#[allow(dead_code)] // For memory commit (FLOW-001)
pub struct Evidence {
    pub id: String,
    pub memory_id: String,
    pub episode_id: String,
    pub source: Option<String>,
    pub frustration: Option<String>,
    pub created_at: String,
}

impl Evidence {
    #[allow(dead_code)]
    pub fn from_row(row: &Row<'_>) -> Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            memory_id: row.get("memory_id")?,
            episode_id: row.get("episode_id")?,
            source: row.get("source")?,
            frustration: row.get("frustration")?,
            created_at: row.get("created_at")?,
        })
    }
}
