# Squirrel Schemas

All data schemas with stable IDs. AI agents must reference these IDs when implementing storage logic.

---

## SCHEMA-001: memories

Core memory table. Minimal schema - no outcome/importance/evidence_source fields.

```sql
CREATE TABLE memories (
  id          TEXT PRIMARY KEY,           -- UUID
  project_id  TEXT,                       -- repo/project identifier (NULL for global)
  scope       TEXT NOT NULL,              -- 'global' | 'project' | 'repo_path'

  owner_type  TEXT NOT NULL,              -- 'user' | 'team' | 'org'
  owner_id    TEXT NOT NULL,              -- user_id / team_id / org_id

  kind        TEXT NOT NULL,              -- 'preference' | 'invariant' | 'pattern' | 'guard' | 'note'
  tier        TEXT NOT NULL,              -- 'short_term' | 'long_term' | 'emergency'
  polarity    INTEGER DEFAULT 1,          -- +1 = recommend/do this, -1 = avoid/don't do this
  key         TEXT,                       -- optional declarative key (e.g., 'project.http.client')
  text        TEXT NOT NULL,              -- 1-2 sentence human-readable memory

  status      TEXT NOT NULL DEFAULT 'provisional',  -- 'provisional' | 'active' | 'deprecated'
  confidence  REAL,                       -- Memory Writer's confidence (0.0-1.0)
  expires_at  TEXT,                       -- ISO 8601 timestamp, NULL = no expiry
  embedding   BLOB,                       -- 1536-dim float32 vector (sqlite-vec)

  created_at  TEXT NOT NULL,              -- ISO 8601
  updated_at  TEXT NOT NULL               -- ISO 8601
);

CREATE INDEX idx_memories_project ON memories(project_id);
CREATE INDEX idx_memories_scope ON memories(scope);
CREATE INDEX idx_memories_owner ON memories(owner_type, owner_id);
CREATE INDEX idx_memories_kind ON memories(kind);
CREATE INDEX idx_memories_tier ON memories(tier);
CREATE INDEX idx_memories_key ON memories(key);
CREATE INDEX idx_memories_status ON memories(status);
```

### Field Constraints

| Field | Type | Nullable | Allowed Values |
|-------|------|----------|----------------|
| scope | TEXT | No | `global`, `project`, `repo_path` |
| owner_type | TEXT | No | `user`, `team`, `org` |
| kind | TEXT | No | `preference`, `invariant`, `pattern`, `guard`, `note` |
| tier | TEXT | No | `short_term`, `long_term`, `emergency` |
| polarity | INTEGER | No | `+1` (recommend), `-1` (avoid) |
| status | TEXT | No | `provisional`, `active`, `deprecated` |
| confidence | REAL | Yes | 0.0 - 1.0 |

### Polarity Semantics

| Polarity | Meaning | Example |
|----------|---------|---------|
| `+1` | Recommend this behavior/fact | "Use httpx as HTTP client" |
| `-1` | Avoid this behavior (anti-pattern) | "Don't use requests - causes SSL errors" |

Anti-patterns (`polarity=-1`) are especially valuable because "don't do X" knowledge often prevents repeated mistakes. CR-Memory can measure regret reduction more clearly for negative memories.

### Kind Semantics

| Kind | Purpose | Example |
|------|---------|---------|
| `preference` | User style/interaction preferences | "User prefers async/await over callbacks" |
| `invariant` | Stable project facts / architectural decisions | "This project uses httpx as HTTP client" |
| `pattern` | Reusable debugging or design patterns | "SSL errors with requests → switch to httpx" |
| `guard` | Risk rules ("don't do X", "ask before Y") | "Don't retry requests with SSL errors" |
| `note` | Lightweight notes / ideas / hypotheses | "Consider migrating to FastAPI v1.0" |

### Tier Semantics

| Tier | Purpose | Lifecycle |
|------|---------|-----------|
| `short_term` | Hot cache, trial memories | Short horizon, may expire or be deprecated |
| `long_term` | Promoted, validated memories | Project/user "personality", extended TTL |
| `emergency` | High-severity guards/patterns | Affects tool execution, not just context |

### Status Lifecycle

```
Memory Writer creates
         │
         ▼
   ┌─────────────┐
   │ provisional │ ← Born from episode
   └──────┬──────┘
          │
    CR-Memory evaluates
          │
    ┌─────┴─────┐
    ▼           ▼
┌────────┐  ┌────────────┐
│ active │  │ deprecated │
└────────┘  └────────────┘
```

### Deprecated Fields (Not in Schema)

The following fields from the old design are **NOT** in this schema:

| Old Field | Reason Removed |
|-----------|----------------|
| `memory_type` (lesson/fact/profile) | Replaced by `kind` |
| `outcome` (success/failure/uncertain) | Stored in `evidence` table |
| `importance` (critical/high/medium/low) | Derived from future-impact, not stored |
| `evidence_source` | Stored in `evidence` table |
| `fact_type` | No longer needed |
| `support_count` | Replaced by `memory_metrics.use_count` |
| `last_seen_at` | Replaced by `memory_metrics.last_used_at` |
| `valid_from` / `valid_to` | Replaced by `created_at` / `expires_at` |
| `superseded_by` | Just use `status='deprecated'` |
| `user_frustration` in metadata | Stored in `evidence` table |

---

## SCHEMA-002: evidence

Links memories to their source episodes. Captures episode-level signals.

```sql
CREATE TABLE evidence (
  id           TEXT PRIMARY KEY,          -- UUID
  memory_id    TEXT NOT NULL,             -- FK to memories
  episode_id   TEXT NOT NULL,             -- FK to episodes
  source       TEXT,                      -- how memory was derived
  frustration  TEXT,                      -- episode frustration level
  created_at   TEXT NOT NULL,             -- ISO 8601
  FOREIGN KEY (memory_id) REFERENCES memories(id),
  FOREIGN KEY (episode_id) REFERENCES episodes(id)
);

CREATE INDEX idx_evidence_memory ON evidence(memory_id);
CREATE INDEX idx_evidence_episode ON evidence(episode_id);
```

### Field Constraints

| Field | Type | Nullable | Allowed Values |
|-------|------|----------|----------------|
| source | TEXT | Yes | `failure_then_success`, `user_correction`, `explicit_statement`, `pattern_observed`, `guard_triggered` |
| frustration | TEXT | Yes | `none`, `mild`, `moderate`, `severe` |

---

## SCHEMA-003: memory_metrics

Tracks usage and impact for CR-Memory evaluation.

```sql
CREATE TABLE memory_metrics (
  memory_id              TEXT PRIMARY KEY,   -- FK to memories
  use_count              INTEGER DEFAULT 0,  -- times actually injected
  opportunities          INTEGER DEFAULT 0,  -- tasks that could have used this
  suspected_regret_hits  INTEGER DEFAULT 0,  -- times it prevented repeated errors
  estimated_regret_saved REAL DEFAULT 0.0,   -- continuous regret metric
  last_used_at           TEXT,               -- ISO 8601, last injection time
  last_evaluated_at      TEXT,               -- ISO 8601, last CR-Memory eval
  FOREIGN KEY (memory_id) REFERENCES memories(id)
);
```

### Metrics Semantics

| Metric | Meaning | Updated By |
|--------|---------|------------|
| `use_count` | How many times memory was chosen/injected into context | Retrieval (get_task_context) |
| `opportunities` | How many tasks could reasonably have used this memory | Retrieval + CR-Memory |
| `suspected_regret_hits` | Times memory appears to have prevented repeated errors/explanations | CR-Memory evaluation |
| `estimated_regret_saved` | Continuous estimate of value (errors/retries avoided) | CR-Memory evaluation |

---

## SCHEMA-004: episodes

Raw episode timeline. Ground truth for Memory Writer and CR-Memory.

```sql
CREATE TABLE episodes (
  id          TEXT PRIMARY KEY,           -- UUID
  project_id  TEXT NOT NULL,              -- repo/project identifier
  start_ts    TEXT NOT NULL,              -- ISO 8601
  end_ts      TEXT NOT NULL,              -- ISO 8601
  events_json TEXT NOT NULL,              -- compressed JSON blob
  processed   INTEGER DEFAULT 0,          -- 0 = pending, 1 = processed by Memory Writer
  created_at  TEXT NOT NULL               -- ISO 8601
);

CREATE INDEX idx_episodes_project ON episodes(project_id);
CREATE INDEX idx_episodes_processed ON episodes(processed);
CREATE INDEX idx_episodes_start ON episodes(start_ts);
```

### events_json Schema

```json
{
  "events": [
    {
      "ts": "2024-12-10T10:00:00Z",
      "role": "user | assistant | tool",
      "kind": "message | tool_call | tool_result",
      "tool_name": "Bash | Edit | Read | Write | Http | null",
      "file": "path/to/file.py | null",
      "summary": "brief description of event",
      "raw_snippet": "optional truncated content"
    }
  ],
  "stats": {
    "error_count": 0,
    "retry_loops": 0,
    "tests_final_status": "passed | failed | not_run | null",
    "user_frustration": "none | mild | moderate | severe"
  }
}
```

**Compression rules:**
- Keep enough signal for Memory Writer & CR-Memory
- Do NOT dump: entire code files, huge tool outputs, full raw logs
- Reference files via paths, commits via hashes

---

## Database Files

| Scope | Path | Contains |
|-------|------|----------|
| Global | `~/.sqrl/squirrel.db` | memories (scope=global), memory_metrics, evidence |
| Project | `<repo>/.sqrl/squirrel.db` | memories (scope=project/repo_path), episodes, evidence, memory_metrics |

---

## Migration from Old Schema

If upgrading from the old lesson/fact/profile schema:

| Old | New |
|-----|-----|
| `memory_type='lesson'` | `kind='pattern'` (most cases) |
| `memory_type='fact'` with key | `kind='invariant'` |
| `memory_type='fact'` without key | `kind='note'` or `kind='pattern'` |
| `memory_type='profile'` | `kind='preference'` |
| `outcome='failure'` + high importance | Consider `kind='guard'` |
| `support_count` | Initialize `memory_metrics.use_count` |
| `user_frustration` in metadata | Move to `evidence.frustration` |

All migrated memories start as `status='provisional'`, `tier='short_term'`. CR-Memory will promote valuable ones.
