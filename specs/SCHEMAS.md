# Squirrel Schemas

Database schemas for user styles and project memories.

---

## Database Files

| Database | Location | Purpose |
|----------|----------|---------|
| User Style DB | `~/.sqrl/user_style.db` | Personal development style preferences |
| Project Memory DB | `<repo>/.sqrl/memory.db` | Project-specific knowledge |

---

## SCHEMA-001: user_styles

Personal development style preferences. Synced to agent.md files.

```sql
CREATE TABLE user_styles (
  id          TEXT PRIMARY KEY,           -- UUID
  text        TEXT NOT NULL UNIQUE,       -- Style description
  use_count   INTEGER DEFAULT 1,          -- Times reinforced/extracted
  created_at  TEXT NOT NULL,              -- ISO 8601
  updated_at  TEXT NOT NULL               -- ISO 8601
);

CREATE INDEX idx_user_styles_use_count ON user_styles(use_count DESC);
```

### Examples

| text |
|------|
| "Prefer async/await over callbacks" |
| "Never use emoji in code or comments" |
| "Commits should use minimal English" |
| "AI should maintain critical, calm attitude" |

---

## SCHEMA-002: project_memories

Project-specific knowledge, organized by category.

```sql
CREATE TABLE project_memories (
  id           TEXT PRIMARY KEY,          -- UUID
  category     TEXT NOT NULL,             -- 'frontend' | 'backend' | 'docs_test' | 'other'
  subcategory  TEXT NOT NULL DEFAULT 'main',  -- User-defined subcategory
  text         TEXT NOT NULL,             -- Memory content
  use_count    INTEGER DEFAULT 1,         -- Times reinforced/extracted
  created_at   TEXT NOT NULL,             -- ISO 8601
  updated_at   TEXT NOT NULL              -- ISO 8601
);

CREATE INDEX idx_project_memories_category ON project_memories(category);
CREATE INDEX idx_project_memories_subcategory ON project_memories(category, subcategory);
CREATE INDEX idx_project_memories_use_count ON project_memories(use_count DESC);
```

### Default Categories

| Category | Description |
|----------|-------------|
| `frontend` | Frontend architecture, components, styling |
| `backend` | Backend architecture, APIs, database |
| `docs_test` | Documentation, testing conventions |
| `other` | Everything else |

### Examples

| category | subcategory | text |
|----------|-------------|------|
| backend | main | "Use httpx as HTTP client, not requests" |
| backend | database | "PostgreSQL 16 for main database" |
| frontend | main | "Component library uses shadcn/ui" |
| docs_test | main | "Tests use pytest with fixtures in conftest.py" |

---

## SCHEMA-003: categories

Custom category/subcategory definitions.

```sql
CREATE TABLE categories (
  id           TEXT PRIMARY KEY,          -- UUID
  category     TEXT NOT NULL,             -- Category name
  subcategory  TEXT NOT NULL,             -- Subcategory name
  created_at   TEXT NOT NULL,             -- ISO 8601
  UNIQUE(category, subcategory)
);
```

### Default Data

```sql
INSERT INTO categories (id, category, subcategory, created_at) VALUES
  ('cat-1', 'frontend', 'main', datetime('now')),
  ('cat-2', 'backend', 'main', datetime('now')),
  ('cat-3', 'docs_test', 'main', datetime('now')),
  ('cat-4', 'other', 'main', datetime('now'));
```

---

## SCHEMA-004: extraction_log

Track extraction history for debugging and analytics.

```sql
CREATE TABLE extraction_log (
  id              TEXT PRIMARY KEY,       -- UUID
  episode_start   TEXT NOT NULL,          -- ISO 8601
  episode_end     TEXT NOT NULL,          -- ISO 8601
  events_count    INTEGER NOT NULL,       -- Number of events processed
  skipped         INTEGER NOT NULL,       -- 1 if skipped by cleaner
  styles_added    INTEGER DEFAULT 0,      -- User styles added
  styles_updated  INTEGER DEFAULT 0,      -- User styles updated
  memories_added  INTEGER DEFAULT 0,      -- Project memories added
  memories_updated INTEGER DEFAULT 0,     -- Project memories updated
  created_at      TEXT NOT NULL           -- ISO 8601
);

CREATE INDEX idx_extraction_log_created ON extraction_log(created_at DESC);
```

---

## SCHEMA-005: docs_index

Indexed documentation files with LLM-generated summaries.

```sql
CREATE TABLE docs_index (
  id            TEXT PRIMARY KEY,           -- UUID
  path          TEXT NOT NULL UNIQUE,       -- Relative path from project root
  summary       TEXT NOT NULL,              -- LLM-generated 1-2 sentence summary
  content_hash  TEXT NOT NULL,              -- SHA-256 of file content
  last_indexed  TEXT NOT NULL               -- ISO 8601
);

CREATE INDEX idx_docs_index_path ON docs_index(path);
```

### Examples

| path | summary |
|------|---------|
| specs/ARCHITECTURE.md | System boundaries, Rust daemon vs Python service split, data flow |
| specs/SCHEMAS.md | SQLite table definitions for memories, styles, projects |
| .claude/CLAUDE.md | Project rules for Claude Code AI assistant |

---

## SCHEMA-006: doc_debt

Tracked documentation debt per commit.

```sql
CREATE TABLE doc_debt (
  id              TEXT PRIMARY KEY,         -- UUID
  commit_sha      TEXT NOT NULL,            -- Git commit SHA
  commit_message  TEXT,                     -- First line of commit message
  code_files      TEXT NOT NULL,            -- JSON array of changed code files
  expected_docs   TEXT NOT NULL,            -- JSON array of docs that should update
  detection_rule  TEXT NOT NULL,            -- 'config' | 'reference' | 'pattern'
  resolved        INTEGER DEFAULT 0,        -- 1 if debt resolved
  resolved_at     TEXT,                     -- ISO 8601 when resolved
  created_at      TEXT NOT NULL             -- ISO 8601
);

CREATE INDEX idx_doc_debt_commit ON doc_debt(commit_sha);
CREATE INDEX idx_doc_debt_resolved ON doc_debt(resolved);
```

### Detection Rules

| Rule | Priority | Description |
|------|----------|-------------|
| config | 1 | User-defined mapping in .sqrl/config.yaml |
| reference | 2 | Code contains spec ID (e.g., SCHEMA-001) |
| pattern | 3 | File pattern match (e.g., *.rs → ARCHITECTURE.md) |

---

## Team Schema (B2B Cloud)

For team features, additional cloud-side schemas.

### team_styles

Team-level development style (managed by admin).

```sql
CREATE TABLE team_styles (
  id          TEXT PRIMARY KEY,
  team_id     TEXT NOT NULL,
  text        TEXT NOT NULL,
  promoted_by TEXT,                       -- User ID who promoted this
  promoted_at TEXT,                       -- When promoted from personal
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL,
  FOREIGN KEY (team_id) REFERENCES teams(id)
);
```

### teams

Team management.

```sql
CREATE TABLE teams (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  owner_id    TEXT NOT NULL,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
```

### team_members

Team membership.

```sql
CREATE TABLE team_members (
  id          TEXT PRIMARY KEY,
  team_id     TEXT NOT NULL,
  user_id     TEXT NOT NULL,
  role        TEXT NOT NULL,              -- 'owner' | 'admin' | 'member'
  joined_at   TEXT NOT NULL,
  FOREIGN KEY (team_id) REFERENCES teams(id),
  UNIQUE(team_id, user_id)
);
```

---

## use_count Semantics

The `use_count` field tracks how many times a memory has been extracted or reinforced.

| Event | Action |
|-------|--------|
| New memory extracted | use_count = 1 |
| Similar memory extracted again | use_count++ on existing |
| Duplicate memory extracted | use_count++ on existing |

**Ordering:** Memories with higher use_count appear first in MCP responses.

**Garbage Collection:** Memories with use_count = 0 (edge case) and age > threshold are candidates for deletion.

---

## Migration Notes

### From Old Schema

If migrating from the previous CR-Memory based schema:

| Old Field | Action |
|-----------|--------|
| `tier` | Remove (no longer used) |
| `status` | Remove (no longer used) |
| `kind` | Map to category or remove |
| `memory_metrics` | Extract use_count, discard rest |
| `evidence` | Remove (no longer tracked) |
| `episodes` | Remove (no longer stored) |

All memories start fresh with use_count = 1.
