# Squirrel Schemas

All data schemas with stable IDs. AI agents must reference these IDs when implementing storage logic.

## SCHEMA-001: memories

Primary storage for all memory types.

```sql
CREATE TABLE memories (
    id TEXT PRIMARY KEY,                          -- UUID
    project_id TEXT,                              -- NULL for global scope
    memory_type TEXT NOT NULL,                    -- lesson | fact | profile

    -- Lesson fields (SCHEMA-001-L)
    outcome TEXT,                                 -- success | failure | uncertain

    -- Fact fields (SCHEMA-001-F)
    fact_type TEXT,                               -- knowledge | process (optional)
    key TEXT,                                     -- declarative key (see KEYS.md)
    value TEXT,                                   -- declarative value
    evidence_source TEXT,                         -- success | failure | neutral | manual
    support_count INTEGER DEFAULT 1,              -- episodes supporting this fact
    last_seen_at TEXT,                            -- last episode timestamp

    -- Content (SCHEMA-001-C)
    text TEXT NOT NULL,                           -- human-readable content
    embedding BLOB,                               -- 1536-dim float32 vector
    metadata TEXT,                                -- JSON: anchors, user_frustration, etc.

    -- Scoring (SCHEMA-001-S)
    confidence REAL NOT NULL,                     -- 0.0 - 1.0
    importance TEXT NOT NULL DEFAULT 'medium',   -- critical | high | medium | low

    -- Lifecycle (SCHEMA-001-LC)
    status TEXT NOT NULL DEFAULT 'active',       -- active | inactive | invalidated
    valid_from TEXT NOT NULL,                    -- ISO 8601 timestamp
    valid_to TEXT,                               -- NULL = still valid
    superseded_by TEXT,                          -- memory_id that replaced this

    -- Audit (SCHEMA-001-A)
    user_id TEXT NOT NULL DEFAULT 'local',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_memories_project ON memories(project_id);
CREATE INDEX idx_memories_type ON memories(memory_type);
CREATE INDEX idx_memories_key ON memories(key);
CREATE INDEX idx_memories_status ON memories(status);
```

### Field Constraints

| Field | Type | Nullable | Allowed Values |
|-------|------|----------|----------------|
| memory_type | TEXT | No | `lesson`, `fact`, `profile` |
| outcome | TEXT | Yes | `success`, `failure`, `uncertain` (lesson only) |
| evidence_source | TEXT | Yes | `success`, `failure`, `neutral`, `manual` (fact only) |
| importance | TEXT | No | `critical`, `high`, `medium`, `low` |
| status | TEXT | No | `active`, `inactive`, `invalidated` |

### Metadata JSON Schema (SCHEMA-001-M)

```json
{
    "anchors": {
        "files": ["string"],
        "components": ["string"],
        "endpoints": ["string"]
    },
    "user_frustration": "none | mild | moderate | severe",
    "source_segments": ["string"],
    "source_episode_id": "string"
}
```

---

## SCHEMA-002: events

Raw events from CLI logs (internal, not exposed to users).

```sql
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    repo TEXT NOT NULL,                          -- absolute path
    kind TEXT NOT NULL,                          -- user | assistant | tool | system
    content TEXT NOT NULL,
    file_paths TEXT,                             -- JSON array
    ts TEXT NOT NULL,                            -- ISO 8601
    processed INTEGER DEFAULT 0
);

CREATE INDEX idx_events_repo ON events(repo);
CREATE INDEX idx_events_processed ON events(processed);
```

### Field Constraints

| Field | Type | Nullable | Allowed Values |
|-------|------|----------|----------------|
| kind | TEXT | No | `user`, `assistant`, `tool`, `system` |
| processed | INTEGER | No | `0` (pending), `1` (processed) |

---

## SCHEMA-003: user_profile

Structured user identity (separate from memory-based profile).

```sql
CREATE TABLE user_profile (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    source TEXT NOT NULL,                        -- explicit | inferred
    confidence REAL,
    updated_at TEXT NOT NULL
);
```

### Standard Keys

| Key | Description | Example |
|-----|-------------|---------|
| `name` | User's name | "Alice" |
| `role` | Job role | "Backend Developer" |
| `experience_level` | Skill level | "Senior" |
| `company` | Organization | "Acme Inc" |
| `primary_use_case` | Main work type | "API development" |

---

## SCHEMA-004: memory_history

Audit trail for memory changes.

```sql
CREATE TABLE memory_history (
    id TEXT PRIMARY KEY,
    memory_id TEXT NOT NULL,
    old_content TEXT,                            -- NULL for ADD
    new_content TEXT NOT NULL,
    event TEXT NOT NULL,                         -- ADD | UPDATE | DELETE
    created_at TEXT NOT NULL,
    FOREIGN KEY (memory_id) REFERENCES memories(id)
);

CREATE INDEX idx_history_memory ON memory_history(memory_id);
```

---

## SCHEMA-005: memory_access_log

Debugging and analytics for retrieval.

```sql
CREATE TABLE memory_access_log (
    id TEXT PRIMARY KEY,
    memory_id TEXT NOT NULL,
    access_type TEXT NOT NULL,                   -- search | get_context | list | key_lookup
    query TEXT,
    score REAL,
    metadata TEXT,                               -- JSON
    accessed_at TEXT NOT NULL,
    FOREIGN KEY (memory_id) REFERENCES memories(id)
);

CREATE INDEX idx_access_memory ON memory_access_log(memory_id);
CREATE INDEX idx_access_type ON memory_access_log(access_type);
```

---

## SCHEMA-006: Episode (in-memory only)

Not persisted. Used for batching events before ingestion.

```python
@dataclass
class Episode:
    id: str                    # UUID
    repo: str                  # absolute path
    start_ts: str              # ISO 8601
    end_ts: str                # ISO 8601
    events: list[Event]        # SCHEMA-002 records
```

### Batching Rules

| Trigger | Condition |
|---------|-----------|
| Time window | 4 hours elapsed since first event |
| Event count | 50 events accumulated |
| Shutdown | Daemon graceful shutdown |

---

## Database Files

| Scope | Path | Contains |
|-------|------|----------|
| Global | `~/.sqrl/squirrel.db` | SCHEMA-001 (scope=global), SCHEMA-003, SCHEMA-004, SCHEMA-005 |
| Project | `<repo>/.sqrl/squirrel.db` | SCHEMA-001 (scope=project), SCHEMA-002 |
