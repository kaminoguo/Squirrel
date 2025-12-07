# Squirrel Development Plan (v1)

Modular development plan with Rust daemon + Python Agent communicating via Unix socket IPC.

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

## Core Insight: Success Detection

Passive learning from logs requires knowing WHAT succeeded and WHAT failed. Unlike explicit APIs where users call `memory.add()`, we infer success from conversation patterns.

**Success signals (implicit):**
- AI says "done" / "complete" + User moves to next task → SUCCESS
- Tests pass, build succeeds → SUCCESS
- User says "thanks", "perfect", "works" → SUCCESS

**Failure signals:**
- Same error reappears after attempted fix → FAILURE
- User says "still broken", "that didn't work" → FAILURE
- User abandons task mid-conversation → UNCERTAIN

**The LLM-decides-everything approach:**
- One LLM call per Episode (4-hour window)
- LLM segments tasks, classifies outcomes, extracts memories
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
- `serde`, `serde_json` (serialization)
- `notify` (file watching)
- `clap` (CLI)
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
-- memories table (individual DB: squirrel.db)
CREATE TABLE memories (
  id TEXT PRIMARY KEY,
  content_hash TEXT NOT NULL UNIQUE,
  content TEXT NOT NULL,
  memory_type TEXT NOT NULL,        -- user_style | user_profile | process | pitfall | recipe | project_fact
  repo TEXT NOT NULL,               -- repo path OR 'global'
  embedding BLOB,                   -- 1536-dim float32 (text-embedding-3-small)
  confidence REAL NOT NULL,
  importance TEXT NOT NULL DEFAULT 'medium',  -- critical | high | medium | low
  state TEXT NOT NULL DEFAULT 'active',       -- active | deleted
  user_id TEXT NOT NULL DEFAULT 'local',
  assistant_id TEXT NOT NULL DEFAULT 'squirrel',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
);

-- team memories table (group DB: group.db) - paid tier
CREATE TABLE team_memories (
  id TEXT PRIMARY KEY,
  content_hash TEXT NOT NULL UNIQUE,
  content TEXT NOT NULL,
  memory_type TEXT NOT NULL,        -- team_style | team_profile | team_process | shared_pitfall | shared_recipe | shared_fact
  repo TEXT NOT NULL,               -- repo path OR 'global'
  embedding BLOB,                   -- 1536-dim float32
  confidence REAL NOT NULL,
  importance TEXT NOT NULL DEFAULT 'medium',
  state TEXT NOT NULL DEFAULT 'active',
  team_id TEXT NOT NULL,            -- team identifier
  contributed_by TEXT NOT NULL,     -- user who shared this
  source_memory_id TEXT,            -- original individual memory ID (if promoted)
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  deleted_at TEXT
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
- `squirrel.db` - individual memories (free)
- `group.db` - team memories (paid, synced)
- `config.toml` - settings

Config fields:
- llm.provider, llm.api_key, llm.base_url
- llm.strong_model, llm.fast_model (2-tier design)
- embedding.provider, embedding.model (default: openai/text-embedding-3-small)
- daemon.idle_timeout_hours (default: 2)
- daemon.socket_path
- team.enabled, team.team_id (paid tier)
- team.sync_mode (cloud | self-hosted | local)
- team.sync_url (for self-hosted)

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

Unix socket client with JSON-RPC style protocol:
```json
{"method": "agent_execute", "params": {...}, "id": 123}
{"result": {...}, "id": 123}
```

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

LLM analyzes entire Episode in ONE call:
1. Segment into Tasks (user goals)
2. Classify each: SUCCESS | FAILURE | UNCERTAIN
3. Extract memories based on outcome:
   - SUCCESS → recipe or project_fact
   - FAILURE → pitfall
   - UNCERTAIN → skip

Near-duplicate check before ADD (0.9 similarity threshold).

UUID→integer mapping when showing existing memories to LLM (prevents hallucination).

### C4. Schemas (`schemas/`)

Memory schema (individual):
- id, content_hash, content, memory_type, repo, embedding
- confidence, importance, state, user_id, assistant_id
- created_at, updated_at, deleted_at
- memory_type: user_style | user_profile | process | pitfall | recipe | project_fact

TeamMemory schema (group, paid):
- id, content_hash, content, memory_type, repo, embedding
- confidence, importance, state, team_id, contributed_by, source_memory_id
- created_at, updated_at, deleted_at
- memory_type: team_style | team_profile | team_process | shared_pitfall | shared_recipe | shared_fact

UserProfile schema:
- key, value, source (explicit|inferred), confidence, updated_at

---

## Track D – Python: Tools Implementation

### D1. Memory Tools (`tools/memory.py`)

**ingest_episode(events):** LLM analysis, task segmentation, outcome classification, memory extraction

**search_memories(query, filters):** Embed query, sqlite-vec search, return ranked results (searches both individual and team DBs)

**get_task_context(task, budget):**
1. Vector search retrieves top 20 candidates (from both individual and team DBs)
2. LLM (fast_model) reranks + composes context prompt:
   - Selects relevant memories
   - Resolves conflicts (team memories may override individual)
   - Merges related memories
   - Generates structured prompt with memory IDs
3. Returns ready-to-inject context prompt within token budget

**forget_memory(id):** Soft-delete (set state='deleted')

**share_memory(id, target_type):** Promote individual memory to team DB (manual, opt-in)
- Copy memory from squirrel.db to group.db
- Set contributed_by, source_memory_id
- Optional type conversion (e.g., pitfall → shared_pitfall)

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
3. find_cli_configs → offer to set_mcp_config

**get_mcp_config(cli), set_mcp_config(cli, server, config):** Read/write MCP config files

**get_user_profile(), set_user_profile(key, value):** Manage user_profile table

### D3.1 Team Tools (`tools/team.py`) - Paid Tier

**team_join(team_id):** Join a team, enable sync

**team_create(name):** Create new team, get team_id

**team_sync():** Force sync with cloud/self-hosted server

**team_leave():** Leave team, keep local copies of team memories

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
- `sqrl update` → self-update binary

Team commands (paid tier):
- `sqrl share <id>` → promote individual memory to team
- `sqrl export <type>` → export memories as JSON
- `sqrl import <file>` → import memories
- `sqrl team join <team-id>` → join team
- `sqrl team create <name>` → create team
- `sqrl team sync` → force sync

---

## Phase X – Hardening

### Logging & Observability
- Structured logging (Rust: `tracing`, Python: `structlog`)
- Metrics: events/episodes/memories processed, latency

### Testing
- Unit tests: storage, events, agent tools
- Integration tests: full flow from log to memory to retrieval

### Cross-Platform
- Mac: brew + install script
- Linux: install script + AUR + nixpkg
- Windows: install script + winget + scoop

### Update Mechanism
- `sqrl update`: download latest binary, replace self
- Auto-update in v1.1

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

**Individual Features (Free):**
- Passive log watching (4 CLIs)
- Success detection (SUCCESS/FAILURE/UNCERTAIN)
- Unified Python agent with tools
- Natural language CLI
- MCP integration (2 tools)
- Lazy daemon (start on demand, stop after 2hr idle)
- Retroactive log ingestion on init (token-limited)
- 6 memory types (user_style, user_profile, process, pitfall, recipe, project_fact)
- Near-duplicate deduplication (0.9 threshold)
- Cross-platform (Mac, Linux, Windows)
- Export/import memories (JSON)
- Auto-update (`sqrl update`)
- Memory consolidation
- Retrieval debugging tools

**Team Features (Paid):**
- Cloud sync for group.db
- Team memory types (team_style, team_profile, shared_*, team_process)
- `sqrl share` command (promote individual to team)
- Team management (create, join, sync)
- Self-hosted option for enterprise

## v2 Scope (Future)

- Hooks output for Claude Code / Gemini CLI
- File injection for AGENTS.md / GEMINI.md
- Team analytics dashboard
- Memory marketplace (export/sell recipe packs)
- Web dashboard

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
