# Squirrel Development Plan (v1)

Modular development plan with Rust daemon + Python Agent communicating via Unix socket IPC.

## Technology Stack

| Category | Technology | Notes |
|----------|------------|-------|
| **Storage** | SQLite + sqlite-vec | Local-first, vector search |
| **IPC Protocol** | JSON-RPC 2.0 | MCP-compatible, over Unix socket |
| **MCP SDK** | rmcp | Official Rust SDK (modelcontextprotocol/rust-sdk) |
| **CLI Framework** | clap | Rust CLI parsing |
| **Agent Framework** | PydanticAI | Python agent with tools |
| **LLM Client** | LiteLLM | Multi-provider support |
| **Embeddings** | OpenAI text-embedding-3-small | 1536-dim, API-based |
| **Build/Release** | dist (cargo-dist) | Generates Homebrew, MSI, installers |
| **Auto-update** | axoupdater | dist's official updater |
| **Python Packaging** | PyInstaller | Bundled, zero user deps |
| **Logging** | tracing (Rust), structlog (Python) | Structured logging |

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        RUST DAEMON                              │
│  Log Watcher → Events → Episodes → IPC → Python Agent           │
│  SQLite/sqlite-vec storage                                      │
│  MCP Server (2 tools)                                           │
│  Thin CLI (passes to agent)                                     │
└─────────────────────────────────────────────────────────────────┘
                             ↕ Unix socket IPC
┌─────────────────────────────────────────────────────────────────┐
│                       PYTHON AGENT                              │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │              Squirrel Agent (single LLM brain)            │  │
│  │                                                           │  │
│  │  Tools:                                                   │  │
│  │  ├── Memory Tools                                         │  │
│  │  │   ├── ingest_episode(events) → memories                │  │
│  │  │   ├── search_memories(query) → results                 │  │
│  │  │   ├── get_task_context(task) → relevant memories       │  │
│  │  │   └── forget_memory(id)                                │  │
│  │  │                                                        │  │
│  │  ├── Filesystem Tools                                     │  │
│  │  │   ├── find_cli_configs() → [claude, codex, ...]        │  │
│  │  │   ├── read_file(path)                                  │  │
│  │  │   ├── write_file(path, content)                        │  │
│  │  │   └── scan_project_logs(project_root) → logs           │  │
│  │  │                                                        │  │
│  │  ├── Config Tools                                         │  │
│  │  │   ├── get_mcp_config(cli) → config                     │  │
│  │  │   ├── set_mcp_config(cli, server, config)              │  │
│  │  │   ├── init_project(path, options)                      │  │
│  │  │   └── get/set_user_profile()                           │  │
│  │  │                                                        │  │
│  │  └── DB Tools                                             │  │
│  │      ├── query_memories(filters)                          │  │
│  │      ├── add_memory(memory)                               │  │
│  │      ├── update_memory(id, changes)                       │  │
│  │      └── get_stats()                                      │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                 │
│  Entry points (all go through same agent):                      │
│  ├── IPC: daemon sends Episode → agent ingests                  │
│  ├── IPC: MCP tool call → agent retrieves                       │
│  └── IPC: CLI command → agent executes                          │
│                                                                 │
│  API Embeddings (text-embedding-3-small, 1536-dim)              │
│  2-tier LLM: strong (ingest) + fast (compose, CLI, dedup)       │
└─────────────────────────────────────────────────────────────────┘
```

## Core Insight: Episode Segmentation

Not all sessions are coding tasks with success/failure outcomes. Sessions include architecture discussions, research, brainstorming - these produce valuable memories but don't fit the success/failure model.

**Episode → Segments → Memories (single LLM call):**

1. **Segment the episode** by kind (not by task success/failure)
2. **Extract memories** based on segment kind
3. **Only EXECUTION_TASK segments** get SUCCESS/FAILURE classification

### Segment Kinds

| Kind | Description | Outcome Field | Memory Output |
|------|-------------|---------------|---------------|
| `EXECUTION_TASK` | Coding, fixing bugs, running commands | `outcome`: SUCCESS / FAILURE / UNCERTAIN | lesson (with outcome), fact |
| `PLANNING_DECISION` | Architecture, design, tech choices | `resolution`: DECIDED / OPEN | fact, lesson (rationale), profile |
| `RESEARCH_LEARNING` | Learning, exploring docs, asking questions | `resolution`: ANSWERED / PARTIAL | fact, lesson |
| `DISCUSSION` | Brainstorming, market research, chat | (none) | profile, lesson (insights) |

**Key rule:** SUCCESS/FAILURE only allowed on EXECUTION_TASK. Other kinds never output FAILURE.

### Success/Failure Detection (EXECUTION_TASK only)

**Success signals (require evidence):**
- AI says "done" / "complete" + User moves to next task → SUCCESS
- Tests pass, build succeeds → SUCCESS
- User says "thanks", "perfect", "works" → SUCCESS

**Failure signals (require evidence):**
- Same error reappears after attempted fix → FAILURE
- User says "still broken", "that didn't work" → FAILURE

**No evidence → UNCERTAIN** (conservative default)

### Episode Ingestion Output Schema

```json
{
  "episode_summary": "...",
  "segments": [
    {
      "id": "seg_1",
      "kind": "EXECUTION_TASK",
      "title": "Fix auth 500 on null user_id",
      "event_range": [12, 33],
      "outcome": {
        "status": "SUCCESS",
        "evidence": ["event#31 tests passed", "event#33 user confirmed"]
      }
    },
    {
      "id": "seg_2",
      "kind": "PLANNING_DECISION",
      "title": "Choose sync conflict strategy",
      "event_range": [34, 44],
      "resolution": "DECIDED",
      "decision": {
        "choice": "server-wins",
        "rationale": ["shared team DB", "simplicity"]
      }
    }
  ],
  "memories": [
    {
      "memory_type": "fact",
      "scope": "project",
      "key": "project.db.engine",
      "value": "PostgreSQL",
      "text": "Project uses PostgreSQL 15 via Prisma.",
      "evidence_source": "neutral",
      "source_segments": ["seg_1"],
      "confidence": 0.86
    },
    {
      "memory_type": "fact",
      "scope": "global",
      "key": "user.preferred_style",
      "value": "async_await",
      "text": "User prefers async/await over callbacks.",
      "evidence_source": "success",
      "source_segments": ["seg_1"],
      "confidence": 0.85
    },
    {
      "memory_type": "lesson",
      "scope": "project",
      "outcome": "failure",
      "text": "Validate user_id before DB insert to avoid 500s.",
      "source_segments": ["seg_1"],
      "confidence": 0.9
    },
    {
      "memory_type": "fact",
      "scope": "project",
      "text": "Auth module handles JWT validation in middleware.",
      "evidence_source": "neutral",
      "source_segments": ["seg_1"],
      "confidence": 0.75
    }
  ]
}
```

**The LLM-decides-everything approach:**
- One LLM call per Episode (4-hour window)
- LLM segments by kind first, then extracts memories per segment
- No rules engine, no heuristics for task detection
- 100% passive - no user prompts or confirmations

## Development Tracks

```
Phase 0: Scaffolding
    |
    v
+-------+-------+-------+-------+
|       |       |       |       |
v       v       v       v       v
Track A Track B Track C Track D Track E
(Rust   (Rust   (Python (Python (MCP +
Storage) Daemon) Agent)  Tools)  CLI)
    |       |       |       |       |
    +---+---+       +---+---+       |
        |               |           |
        v               v           v
    Week 1-2        Week 2-3    Week 3-4
        |               |           |
        +-------+-------+-----------+
                |
                v
            Phase X
            Hardening
```

---

## Phase 0 – Scaffolding

### Rust Module (`agent/`)

Dependencies:
- `tokio` (async runtime)
- `rusqlite` + `sqlite-vec` (storage)
- `serde`, `serde_json` (serialization, JSON-RPC 2.0)
- `notify` (file watching)
- `clap` (CLI framework)
- `rmcp` (official MCP SDK - modelcontextprotocol/rust-sdk)
- `tracing` (structured logging)
- `uuid`, `chrono` (ID, timestamps)

Directory structure:
```
agent/src/
├── main.rs
├── lib.rs
├── daemon.rs       # Lazy start, idle shutdown
├── watcher.rs      # Log file watching
├── storage.rs      # SQLite + sqlite-vec
├── events.rs       # Event/Episode structs
├── config.rs       # Config management
├── mcp.rs          # MCP server
└── ipc.rs          # Unix socket client
```

### Python Module (`memory_service/`)

Dependencies:
- `pydantic-ai` (agent framework)
- `litellm` (multi-provider LLM support)
- `openai` (embeddings API client)
- `pydantic` (schemas)
- `structlog` (structured logging)

Directory structure:
```
memory_service/
├── squirrel_memory/
│   ├── __init__.py
│   ├── server.py           # Unix socket server
│   ├── agent.py            # Unified agent
│   ├── tools/
│   │   ├── __init__.py
│   │   ├── memory.py       # ingest, search, get_context, forget
│   │   ├── filesystem.py   # find_cli_configs, read/write_file
│   │   ├── config.py       # init_project, mcp_config, user_profile
│   │   └── db.py           # query, add, update memories
│   ├── embeddings.py       # API embeddings (OpenAI, etc.)
│   ├── retrieval.py        # Similarity search
│   └── schemas/
└── tests/
```

---

## Track A – Rust: Storage + Config + Events

### A1. Storage layer (`storage.rs`)

SQLite + sqlite-vec initialization:
```sql
-- memories table (squirrel.db)
CREATE TABLE memories (
  id TEXT PRIMARY KEY,
  project_id TEXT,                  -- NULL for global/user-scope memories
  memory_type TEXT NOT NULL,        -- lesson | fact | profile

  -- For lessons (task-level patterns/pitfalls)
  outcome TEXT,                     -- success | failure | uncertain (lesson only)

  -- For facts
  fact_type TEXT,                   -- knowledge | process (fact only, optional)
  key TEXT,                         -- declarative key: project.db.engine, user.preferred_style
  value TEXT,                       -- declarative value: PostgreSQL, async_await
  evidence_source TEXT,             -- success | failure | neutral | manual (fact only)
  support_count INTEGER,            -- approx episodes that support this fact
  last_seen_at TEXT,                -- last episode where this was seen

  -- Content
  text TEXT NOT NULL,               -- human-readable content
  embedding BLOB,                   -- 1536-dim float32 (text-embedding-3-small)
  metadata TEXT,                    -- JSON: anchors (files, components, endpoints)

  -- Confidence
  confidence REAL NOT NULL,
  importance TEXT NOT NULL DEFAULT 'medium',  -- critical | high | medium | low

  -- Lifecycle
  status TEXT NOT NULL DEFAULT 'active',      -- active | inactive | invalidated
  valid_from TEXT NOT NULL,
  valid_to TEXT,
  superseded_by TEXT,

  -- Audit
  user_id TEXT NOT NULL DEFAULT 'local',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

-- events table
CREATE TABLE events (
  id TEXT PRIMARY KEY,
  repo TEXT NOT NULL,
  kind TEXT NOT NULL,        -- user | assistant | tool | system
  content TEXT NOT NULL,
  file_paths TEXT,           -- JSON array
  ts TEXT NOT NULL,
  processed INTEGER DEFAULT 0
);

-- user_profile table (structured, not memories)
CREATE TABLE user_profile (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  source TEXT NOT NULL,      -- explicit | inferred
  confidence REAL,
  updated_at TEXT NOT NULL
);

-- memory_history table (audit trail)
CREATE TABLE memory_history (
  id TEXT PRIMARY KEY,
  memory_id TEXT NOT NULL,
  old_content TEXT,
  new_content TEXT NOT NULL,
  event TEXT NOT NULL,       -- ADD | UPDATE | DELETE
  created_at TEXT NOT NULL,
  FOREIGN KEY (memory_id) REFERENCES memories(id)
);

-- memory_access_log table (debugging)
CREATE TABLE memory_access_log (
  id TEXT PRIMARY KEY,
  memory_id TEXT NOT NULL,
  access_type TEXT NOT NULL,  -- search | get_context | list
  query TEXT,
  score REAL,
  metadata TEXT,              -- JSON
  accessed_at TEXT NOT NULL,
  FOREIGN KEY (memory_id) REFERENCES memories(id)
);
```

### A1.1 Declarative Key Registry

Declarative keys are the "rigid backbone" for critical facts. Same key + different value triggers deterministic invalidation (no LLM needed).

**Project-scoped keys** (project_id set):
```
project.db.engine          # PostgreSQL, MySQL, SQLite
project.db.version         # 15, 8.0, 3.x
project.api.framework      # FastAPI, Express, Rails
project.ui.framework       # React, Vue, Svelte
project.language.main      # Python, TypeScript, Go
project.test.command       # pytest, npm test, go test
project.build.command      # npm run build, cargo build
project.auth.method        # JWT, session, OAuth
project.package_manager    # npm, pnpm, yarn, pip, uv
project.orm                # Prisma, SQLAlchemy, TypeORM
```

**User-scoped keys** (project_id = NULL, stored in global db):
```
user.preferred_style       # async_await, callbacks, sync
user.preferred_language    # Python, TypeScript, Go
user.strict_null_checks    # true, false
user.comment_style         # minimal, detailed, jsdoc
user.error_handling        # exceptions, result_types, errors
```

**Key behaviors:**
- Keys are optional - most facts remain free-text
- LLM extracts key during ingestion when pattern matches registry
- Same key + different value → deterministic invalidation of old fact
- Keys enable fast lookup without vector search

### A1.2 Memory Lifecycle (Forget Mechanism)

**Status values:**
- `active` - Normal, appears in retrieval
- `inactive` - Soft deleted by user (`sqrl forget`), recoverable, hidden from retrieval
- `invalidated` - Superseded by newer fact, keeps history, hidden from retrieval

**Validity fields (for facts):**
- `valid_from` - When this became true (default: created_at)
- `valid_to` - When it stopped being true (null = still valid)
- `superseded_by` - ID of memory that replaced this

**Retrieval filter:**
```sql
WHERE status = 'active'
  AND (valid_to IS NULL OR valid_to > datetime('now'))
```

**Contradiction detection (during ingestion):**

For facts with declarative `key`:
- Same key + different value → invalidate old (status='invalidated', valid_to=now, superseded_by=new_id)
- Deterministic, no LLM needed
- Example: key=project.db.engine, old value=MySQL, new value=PostgreSQL → invalidate old

For free-text facts without key:
- LLM judges semantic conflict between new fact and similar existing facts
- High confidence conflict → invalidate old
- Low confidence → keep both, let retrieval handle via recency weighting

**No cascade delete:**
- Invalidating a fact does NOT delete related lessons
- Related lessons get flagged in retrieval output: `dependency_changed: true`

**CLI behavior:**
- `sqrl forget <id>` → status='inactive' (soft delete, recoverable)
- `sqrl forget "deprecated API"` → search + confirm + soft delete matching memories

**v1 limitations (documented for users):**
- No TTL/auto-expiration (manual forget only)
- No hard delete/purge (data remains in SQLite file)
- Free-text contradiction detection depends on LLM, may have false positives

### A2. Event model (`events.rs`)

Event struct (normalized, CLI-agnostic):
- id, repo, kind (User|Assistant|Tool|System), content, file_paths, ts, processed

Episode struct (in-memory only):
- id, repo, start_ts, end_ts, events

Episode batching: 4-hour time window OR 50 events max (whichever first)

### A3. Config (`config.rs`)

Paths:
- `~/.sqrl/` (global)
- `<repo>/.sqrl/` (project)

Files:
- `squirrel.db` - memories
- `config.toml` - settings

Config fields:
- agents.claude_code, agents.codex_cli, agents.gemini_cli, agents.cursor (CLI selection)
- llm.provider, llm.api_key, llm.base_url
- llm.strong_model, llm.fast_model (2-tier design)
- embedding.provider, embedding.model (default: openai/text-embedding-3-small)
- daemon.idle_timeout_hours (default: 2)
- daemon.socket_path

Projects registry (`~/.sqrl/projects.json`):
- List of initialized project paths for `sqrl sync`

---

## Track B – Rust: Daemon + Watcher + IPC

### B1. Daemon (`daemon.rs`)

**Lazy start:** Daemon starts on first `sqrl` command, not on boot.

**Idle shutdown:** Stop after N hours of no log activity. Flush pending Episodes on shutdown.

Startup sequence:
1. Check if already running (socket exists and responds)
2. If not: spawn daemon process
3. Load projects registry
4. For each project, spawn watcher
5. Spawn Python Agent as child process
6. Connect via Unix socket

### B2. Log Watchers (`watcher.rs`)

Multi-CLI log discovery:
- `~/.claude/projects/**/*.jsonl` (Claude Code)
- `~/.codex-cli/logs/**/*.jsonl` (Codex CLI)
- `~/.gemini/logs/**/*.jsonl` (Gemini CLI)
- `~/.cursor-tutor/logs/**/*.jsonl` (Cursor)

Line parsers for each CLI format → normalized Event

### B3. Episode Batching

Flush triggers:
- 50 events reached OR
- 4 hours elapsed OR
- Daemon shutdown (graceful flush)

On flush: create Episode, send to Python via IPC, mark events processed

### B4. IPC Client (`ipc.rs`)

Unix socket client with JSON-RPC 2.0 protocol (MCP-compatible):
```json
{"jsonrpc": "2.0", "method": "ingest_episode", "params": {"episode": {...}}, "id": 1}
{"jsonrpc": "2.0", "result": {"memories_created": 3}, "id": 1}
```

Methods:
- `ingest_episode` - Process episode, extract memories
- `get_task_context` - MCP tool call
- `search_memories` - MCP tool call
- `execute_command` - CLI natural language/direct command

---

## Track C – Python: Unified Agent

### C1. Unix Socket Server (`server.py`)

Socket at `/tmp/sqrl_agent.sock`

Single entry point - all requests go to agent:
```python
async def handle_connection(reader, writer):
    request = await read_json(reader)
    result = await agent.execute(request["params"])
    await write_json(writer, {"result": result, "id": request["id"]})
```

### C2. Squirrel Agent (`agent.py`)

Single LLM-powered agent with tools using PydanticAI framework. Uses 2-tier LLM design:

| Task | Model Tier | Default |
|------|------------|---------|
| Episode Ingestion | strong_model | gemini-2.5-pro |
| Context Compose | fast_model | gemini-3-flash |
| CLI Interpretation | fast_model | gemini-3-flash |
| Near-duplicate Check | fast_model | gemini-3-flash |

```python
class SquirrelAgent:
    def __init__(self):
        self.tools = [
            # Memory tools
            ingest_episode,
            search_memories,
            get_task_context,
            forget_memory,
            # Filesystem tools
            find_cli_configs,
            read_file,
            write_file,
            scan_project_logs,
            # Config tools
            init_project,
            get_mcp_config,
            set_mcp_config,
            get_user_profile,
            set_user_profile,
            # DB tools
            query_memories,
            add_memory,
            update_memory,
            get_stats,
        ]

    async def execute(self, input: dict) -> dict:
        """
        Single entry point for all operations.
        Agent decides which tools to use based on input.
        """
        # For Episode ingestion (from daemon)
        if input.get("type") == "episode":
            return await self.ingest(input["episode"])

        # For MCP calls
        if input.get("type") == "mcp":
            return await self.handle_mcp(input["tool"], input["args"])

        # For CLI commands (natural language or direct)
        return await self.handle_command(input.get("command", ""))
```

### C3. Episode Ingestion (via ingest_episode tool)

LLM analyzes entire Episode in ONE call (segment-first approach):

1. **Segment by kind** (not by success/failure):
   - EXECUTION_TASK - coding, fixing, running commands
   - PLANNING_DECISION - architecture, design choices
   - RESEARCH_LEARNING - learning, exploring docs
   - DISCUSSION - brainstorming, chat

2. **For EXECUTION_TASK segments only**, classify outcome:
   - SUCCESS (with evidence: tests passed, user confirmed, etc.)
   - FAILURE (with evidence: error persists, user says "didn't work")
   - UNCERTAIN (no clear evidence - conservative default)

3. **Extract memories based on segment kind:**
   - EXECUTION_TASK → lesson (with outcome), fact (knowledge discovered)
   - PLANNING_DECISION → fact (decisions), lesson (rationale for rejected options), profile
   - RESEARCH_LEARNING → fact (knowledge), lesson (key learnings)
   - DISCUSSION → profile (user preferences), lesson (insights)

4. **Contradiction check for facts:**
   - Extract semantic_key if possible (db.engine, api.framework, etc.)
   - Check existing facts with same key → invalidate old if conflict
   - Free-text facts → LLM judges semantic conflict

5. **Near-duplicate check** before ADD (0.9 similarity threshold)

UUID→integer mapping when showing existing memories to LLM (prevents hallucination).

### C4. Schemas (`schemas/`)

**Memory schema:**
- id, project_id (NULL for global), memory_type (lesson | fact | profile)
- text (human-readable content), embedding (1536-dim)
- metadata (JSON: anchors - files, components, endpoints)
- confidence, importance (critical | high | medium | low)
- user_id, created_at, updated_at

**Lesson-specific fields:**
- outcome: success | failure | uncertain

**Fact-specific fields:**
- fact_type: knowledge | process (optional)
- key: declarative key (project.db.engine, user.preferred_style, etc.)
- value: declarative value (PostgreSQL, async_await, etc.)
- evidence_source: success | failure | neutral | manual
- support_count: number of episodes that support this fact
- last_seen_at: timestamp of last episode where seen

**Lifecycle fields (all types):**
- status: active | inactive | invalidated
- valid_from: timestamp (when this became true)
- valid_to: timestamp | null (when it stopped being true)
- superseded_by: memory_id | null (for invalidated facts)

**UserProfile schema (structured identity, not memories):**
- key, value, source (explicit|inferred), confidence, updated_at
- Examples: name, role, experience_level, company, primary_use_case

---

## Track D – Python: Tools Implementation

### D1. Memory Tools (`tools/memory.py`)

**ingest_episode(events):** LLM analysis, task segmentation, outcome classification, memory extraction

**search_memories(query, filters):** Embed query, sqlite-vec search, return ranked results

**get_task_context(task, budget):**
1. Vector search retrieves top 20 candidates
2. LLM (fast_model) reranks + composes context prompt:
   - Selects relevant memories
   - Resolves conflicts between memories
   - Merges related memories
   - Generates structured prompt with memory IDs
3. Returns ready-to-inject context prompt within token budget

**Context output structure:**
```json
{
  "project_facts": [
    {"key": "project.db.engine", "value": "PostgreSQL", "text": "..."},
    {"key": "project.api.framework", "value": "FastAPI", "text": "..."}
  ],
  "user_prefs": [
    {"key": "user.preferred_style", "value": "async_await", "text": "..."}
  ],
  "lessons": [
    {"outcome": "failure", "text": "Validate user_id before DB insert...", "id": "mem_123"},
    {"outcome": "success", "text": "Use repository pattern for DB access...", "id": "mem_456"}
  ],
  "process_facts": [
    {"text": "Auth module handles JWT validation in middleware.", "id": "mem_789"}
  ],
  "profile": {
    "name": "Alice",
    "role": "Backend Developer",
    "experience_level": "Senior"
  }
}
```

**forget_memory(id_or_query):**
- If ID: set status='inactive' (soft delete, recoverable)
- If natural language query: search → confirm with user → soft delete matches

**export_memories(filters, format):** Export memories as JSON for sharing/backup

**import_memories(data):** Import memories from JSON

### D2. Filesystem Tools (`tools/filesystem.py`)

**find_cli_configs():** Scan for ~/.claude, ~/.codex-cli, ~/.gemini, etc.

**scan_project_logs(project_root, token_limit):** Find logs mentioning project files, return within token limit

**read_file(path), write_file(path, content):** Basic file operations for config management

### D3. Config Tools (`tools/config.py`)

**init_project(path, skip_history):**
1. Create `<path>/.sqrl/squirrel.db`
2. If not skip_history: scan_project_logs → ingest
3. For each enabled CLI in `config.agents`:
   - Configure MCP (add Squirrel server to CLI's MCP config)
   - Inject instruction text to agent file (CLAUDE.md, AGENTS.md, GEMINI.md, .cursor/rules/)
4. Register project in `~/.sqrl/projects.json`

**sync_projects():**
1. Read enabled CLIs from `config.agents`
2. For each project in `~/.sqrl/projects.json`:
   - For each enabled CLI not yet configured in that project:
     - Configure MCP
     - Inject instruction text

**get_mcp_config(cli), set_mcp_config(cli, server, config):** Read/write MCP config files

**get_agent_instructions(cli), set_agent_instructions(cli, content):** Read/write agent instruction files

**get_user_profile(), set_user_profile(key, value):** Manage user_profile table

### D3.1 MCP Config Locations

| CLI | MCP Config Location |
|-----|---------------------|
| Claude Code | `~/.claude.json` or `<project>/.mcp.json` |
| Codex CLI | `codex mcp add-server` command |
| Gemini CLI | `<project>/.gemini/settings.json` |
| Cursor | `~/.cursor/mcp.json` or `<project>/.cursor/mcp.json` |

MCP server definition:
```json
{
  "mcpServers": {
    "squirrel": {
      "command": "sqrl-daemon",
      "args": ["--mcp"],
      "disabled": false
    }
  }
}
```

### D3.2 Agent Instruction Files

| CLI | Instruction File |
|-----|------------------|
| Claude Code | `<project>/CLAUDE.md` |
| Codex CLI | `<project>/AGENTS.md` |
| Gemini CLI | `<project>/GEMINI.md` |
| Cursor | `<project>/.cursor/rules/squirrel.mdc` |

Instruction text to inject:
```markdown
## Squirrel Memory System

This project uses Squirrel for persistent memory across sessions.

ALWAYS call `squirrel_get_task_context` BEFORE:
- Fixing bugs (to check if this bug was seen before)
- Refactoring code (to get patterns that worked/failed)
- Adding features touching existing modules
- Debugging errors that seem familiar

DO NOT call for:
- Simple typo fixes
- Adding comments
- Formatting changes
```

### D4. DB Tools (`tools/db.py`)

**query_memories(filters):** Direct DB query with filtering

**add_memory(memory):** Insert + log to history

**update_memory(id, changes):** Update + log old/new to history

**get_stats():** Memory counts, access stats, etc.

### D5. Embeddings (`embeddings.py`)

API-based embeddings via OpenAI (text-embedding-3-small, 1536-dim).

Supports multiple providers via config. Batch embedding with retry logic.

### D6. Retrieval (`retrieval.py`)

Similarity search via sqlite-vec.

Scoring: `w_sim * similarity + w_imp * importance_weight + w_rec * recency`
- importance_weight: critical=1.0, high=0.75, medium=0.5, low=0.25

Access logging to memory_access_log table.

---

## Track E – MCP + CLI

### E1. MCP Server (`mcp.rs`)

Uses `rmcp` (official MCP SDK from modelcontextprotocol/rust-sdk).

2 tools:
```
squirrel_get_task_context
  - project_root: string (required)
  - task: string (required)
  - context_budget_tokens: integer (default: 400)

squirrel_search_memory
  - project_root: string (required)
  - query: string (required)
  - top_k: integer (default: 10)
```

For trivial queries, return empty fast (<20ms).

### E2. CLI (`cli.rs`)

Thin shell that passes to Python agent:

```rust
fn main() {
    ensure_daemon_running()?;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let input = args.join(" ");

    let response = ipc_client.send("agent_execute", {
        "type": "command",
        "command": input
    });

    println!("{}", response);
}
```

Supports both natural language and direct commands:
- `sqrl "setup this project"` → agent interprets
- `sqrl init --skip-history` → agent interprets
- `sqrl config` → interactive CLI selection
- `sqrl sync` → update all projects with new CLI configs
- `sqrl update` → auto-update via axoupdater
- `sqrl export <type>` → export memories as JSON
- `sqrl import <file>` → import memories

---

## Phase X – Hardening

### Logging & Observability
- Structured logging (Rust: `tracing`, Python: `structlog`)
- Metrics: events/episodes/memories processed, latency

### Testing
- Unit tests: storage, events, agent tools
- Integration tests: full flow from log to memory to retrieval

### Build & Release (dist)

Uses `dist` (cargo-dist) as single release orchestrator:
- Builds Rust daemon for Mac/Linux/Windows
- Builds Python agent via PyInstaller (as dist workspace member)
- Generates installers: Homebrew, MSI, shell/powershell scripts

### Cross-Platform Installation

| Platform | Primary | Fallback |
|----------|---------|----------|
| Mac | `brew install sqrl` | install script |
| Linux | `brew install sqrl` | install script, AUR, nixpkg |
| Windows | MSI installer | winget, install script |

Windows note: MSI recommended over raw .exe to reduce SmartScreen/AV friction.

### Auto-Update (axoupdater)

- `sqrl update` uses axoupdater (dist's official updater)
- Updates both Rust daemon and Python agent together
- Reads dist install receipt to determine installed version/source

---

## Timeline Summary

| Week | Track A | Track B | Track C | Track D | Track E |
|------|---------|---------|---------|---------|---------|
| 0 | Scaffold | Scaffold | Scaffold | - | - |
| 1 | Storage, Events, Config | Daemon start | Agent skeleton | - | - |
| 2 | - | Watchers, IPC, Batching | - | Memory tools | - |
| 3 | - | Integration | - | Filesystem, Config tools | MCP server |
| 4 | - | - | - | DB tools, Retrieval | CLI |
| 5+ | - | Hardening | - | Hardening | Cross-platform |

---

## Team Assignment (3 developers)

**Developer 1 (Rust focus):**
- Phase 0: Rust scaffold
- Track A: All
- Track B: All
- Track E: MCP server, CLI

**Developer 2 (Python focus):**
- Phase 0: Python scaffold
- Track C: All
- Track D: All

**Developer 3 (Full-stack / Integration):**
- Phase 0: CI, docs
- Agent prompts
- Cross-platform packaging
- Phase X: Testing, documentation

---

## v1 Scope

- Passive log watching (4 CLIs)
- Episode segmentation (EXECUTION_TASK / PLANNING_DECISION / RESEARCH_LEARNING / DISCUSSION)
- Success detection for EXECUTION_TASK only (SUCCESS/FAILURE/UNCERTAIN with evidence)
- Unified Python agent with tools
- Natural language CLI
- MCP integration (2 tools)
- Lazy daemon (start on demand, stop after 2hr idle)
- Retroactive log ingestion on init (token-limited)
- 3 memory types (lesson, fact, profile) with scope flag
- Declarative keys for facts (project.* and user.*) with deterministic conflict detection
- Evidence source tracking for facts (success/failure/neutral/manual)
- Memory lifecycle: status (active/inactive/invalidated) + validity (valid_from/valid_to)
- Fact contradiction detection (declarative key match + LLM for free-text)
- Soft delete (`sqrl forget`) - no hard purge
- Near-duplicate deduplication (0.9 threshold)
- Cross-platform (Mac, Linux, Windows)
- Export/import memories (JSON)
- Auto-update (`sqrl update`)
- Memory consolidation
- Retrieval debugging tools
- CLI selection (`sqrl config`) + MCP wiring + agent instruction injection
- `sqrl sync` for updating existing projects with new CLIs

**v1 limitations:**
- No TTL/auto-expiration (manual forget only)
- No hard delete (soft delete only, data remains in SQLite)
- Free-text contradiction detection may have false positives

## v2 Scope (Future)

- Team/cloud sync (group.db, share command, team management)
- Deep CLI integrations (Claude Code hooks, Cursor extension)
- Team analytics dashboard
- Memory marketplace
- TTL / temporary memory (auto-expiration)
- Hard purge for privacy/compliance
- Memory linking + evolution (A-MEM style)
- Richer conflict detection with schema/key registry
- `get_memory_history` API for debugging invalidation chains

---

## v2 Architecture: Team/Cloud

### Overview

v2 adds team memory sharing via `group.db` - a separate database that syncs with cloud. Individual memories stay in `squirrel.db` (local-only), team memories go to `group.db` (synced).

### 3-Layer Database Architecture (v2)

| Layer | DB File | Contents | Sync |
|-------|---------|----------|------|
| **Global** | `~/.sqrl/squirrel.db` | lesson, fact, profile (scope=global) | Local only |
| **Project** | `<repo>/.sqrl/squirrel.db` | lesson, fact (scope=project) | Local only |
| **Team** | `~/.sqrl/group.db` + `<repo>/.sqrl/group.db` | Shared memories (owner=team) | Cloud |

### Memory Schema (v2)

Additional fields for team support:

```sql
CREATE TABLE memories (
  -- ... all v1 fields ...
  owner TEXT NOT NULL DEFAULT 'individual',  -- individual | team
  team_id TEXT,                              -- team identifier (for owner=team)
  contributed_by TEXT,                       -- user who shared (for owner=team)
  source_memory_id TEXT                      -- original memory ID (if promoted to team)
);
```

### Team Tools (v2)

**share_memory(memory_id):** Promote individual memory to team
1. Read from `squirrel.db`
2. Copy to `group.db` with `owner: team`
3. Set `contributed_by`, `source_memory_id`
4. Sync triggers cloud upload

**team_join(team_id), team_leave():** Team membership management

**team_export(filters):** Export team memories for offline/backup

### Sync Architecture

**Local-first with background sync:**
- `group.db` is local copy, always available
- Background process syncs with cloud
- Users never wait for network
- Conflict resolution: last-write-wins with vector clocks

**Scaling considerations (from research):**
- Individual user (6 months): ~6MB (900 memories)
- Team (10,000 users): ~6GB if full sync - NOT viable

**Hybrid approach for large teams:**
| Team Size | Strategy |
|-----------|----------|
| Small (<100) | Full sync - all team memories in local group.db |
| Medium (100-1000) | Partial sync - recent + relevant memories locally |
| Large (1000+) | Cloud-primary - query cloud, cache locally |

**Reference:** Figma, Notion, Linear all use server-first or partial sync. Nobody syncs everything locally at scale.

### Team Commands (v2)

```bash
sqrl team join <team-id>      # Join team, start syncing group.db
sqrl team leave               # Leave team, remove group.db
sqrl share <memory-id>        # Promote individual memory to team
sqrl share --all              # Share all individual memories to team
sqrl team export              # Export team memories to local
```

### Migration Paths

**Local → Cloud (user subscribes):**
```bash
sqrl share --all              # Promotes all individual memories to team
```

**Cloud → Local (team exports):**
```bash
sqrl team export --project    # Downloads team memories to local squirrel.db
```

### Config (v2)

```toml
# ~/.sqrl/config.toml
[team]
id = "abc-team-id"
sync_interval_seconds = 300   # 5 min background sync
sync_strategy = "full"        # full | partial | cloud-primary
```

### Retrieval (v2)

Context retrieval queries BOTH databases:
1. `squirrel.db` (individual memories)
2. `group.db` (team memories)
3. LLM reranks combined results

Team memories get attribution: "From team member Alice"

---

## Patterns from Competitor Analysis

| Pattern | Source | Location |
|---------|--------|----------|
| UUID→integer mapping for LLM | mem0 | Agent ingest |
| History tracking (old/new content) | mem0 | memory_history table |
| Structured exceptions | mem0 | All tools |
| Soft-delete (state column) | mem0 | memories table |
| Access logging | mem0 | memory_access_log table |
| Success detection | claude-cache | Agent ingest |
| Pitfall learning | claude-cache | Memory types |
| Unified agent with tools | letta | Agent architecture |
| Session Q&A tracking | cognee | memory_access_log |

**Key insight:** Passive learning requires success detection. We let the LLM decide task outcomes instead of building a rules engine.
