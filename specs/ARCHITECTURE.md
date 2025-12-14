# Squirrel Architecture

High-level system boundaries and data flow.

## Technology Stack

| Category | Technology | Notes |
|----------|------------|-------|
| **Rust Daemon** | | |
| Storage | SQLite + sqlite-vec | Local-first, vector search |
| IPC Protocol | JSON-RPC 2.0 | Over Unix socket |
| MCP SDK | rmcp | Official Rust SDK (modelcontextprotocol/rust-sdk) |
| CLI Framework | clap | Rust CLI parsing |
| Async Runtime | tokio | Async I/O |
| File Watching | notify | Cross-platform fs events |
| Logging | tracing | Structured logging |
| **Python Memory Service** | | |
| Agent Framework | PydanticAI | Structured LLM outputs |
| LLM Client | LiteLLM | Multi-provider support |
| Embeddings | OpenAI text-embedding-3-small | 1536-dim, API-based |
| Logging | structlog | Structured logging |
| **Build & Release** | | |
| Rust Build | cargo-dist | Generates Homebrew, MSI, installers |
| Auto-update | axoupdater | dist's official updater |
| Python Packaging | PyInstaller | Bundled, zero user deps |

## System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        User's Machine                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │ Claude Code  │    │    Cursor    │    │   Windsurf   │       │
│  │    (CLI)     │    │    (IDE)     │    │    (IDE)     │       │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘       │
│         │                   │                   │                │
│         └─────────┬─────────┴─────────┬─────────┘                │
│                   │                   │                          │
│                   ▼                   ▼                          │
│         ┌─────────────────┐  ┌─────────────────┐                 │
│         │   Log Files     │  │   MCP Client    │                 │
│         │ (watched)       │  │   (requests)    │                 │
│         └────────┬────────┘  └────────┬────────┘                 │
│                  │                    │                          │
│                  ▼                    ▼                          │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    RUST DAEMON                             │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │  │
│  │  │ Log Watcher │  │ MCP Server  │  │ CLI Handler │        │  │
│  │  │   (notify)  │  │   (rmcp)    │  │  (clap)     │        │  │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘        │  │
│  │         │                │                │               │  │
│  │         └────────────────┼────────────────┘               │  │
│  │                          ▼                                │  │
│  │              ┌─────────────┐  ┌─────────────┐             │  │
│  │              │   Commit    │  │  Retrieval  │             │  │
│  │              │    Layer    │  │   Engine    │             │  │
│  │              └─────────────┘  └─────────────┘             │  │
│  │         │                │                │               │  │
│  │         └────────────────┼────────────────┘               │  │
│  │                          ▼                                │  │
│  │                 ┌─────────────────┐                       │  │
│  │                 │     SQLite      │                       │  │
│  │                 │  + sqlite-vec   │                       │  │
│  │                 └─────────────────┘                       │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│                              │ Unix Socket (JSON-RPC 2.0)        │
│                              ▼                                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │               PYTHON MEMORY SERVICE                        │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                  Memory Writer                       │  │  │
│  │  │  - ingest_episode (strong model, 1 call/episode)    │  │  │
│  │  │  - Returns ops[] + episode_evidence                  │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                  Retrieval Module                    │  │  │
│  │  │  - embed_text (embedding generation)                │  │  │
│  │  │  - compose_context (optional, v1 uses templates)    │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                  CR-Memory Module                    │  │  │
│  │  │  - Background job (periodic/after N episodes)       │  │  │
│  │  │  - Reads memory_metrics + policy                    │  │  │
│  │  │  - Promotes / deprecates / adjusts TTL              │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │                          │                                │  │
│  │                          ▼                                │  │
│  │                 ┌─────────────────┐                       │  │
│  │                 │   LLM Router    │                       │  │
│  │                 │ (strong/fast)   │                       │  │
│  │                 └────────┬────────┘                       │  │
│  └───────────────────────────┼───────────────────────────────┘  │
│                              │                                   │
└──────────────────────────────┼───────────────────────────────────┘
                               │ HTTPS
                               ▼
                    ┌─────────────────────┐
                    │    LLM Providers    │
                    │ (Anthropic/OpenAI)  │
                    └─────────────────────┘
```

## Component Boundaries

### ARCH-001: Rust Daemon

**Responsibility:** All local I/O, storage, vector search. **100% offline - no HTTP/network calls.**

| Module | Crate | Purpose |
|--------|-------|---------|
| Log Watcher | notify | File system events for CLI logs |
| MCP Server | rmcp | Model Context Protocol server |
| CLI Handler | clap | User-facing commands |
| Commit Layer | - | Execute memory ops (ADD/UPDATE/DEPRECATE) |
| Retrieval Engine | rusqlite + sqlite-vec | Key lookup + vector search + ranking |
| SQLite Storage | rusqlite + sqlite-vec | All tables: memories, evidence, memory_metrics, episodes |

**Owns:**
- All SQLite read/write operations
- All sqlite-vec vector similarity queries
- Memory metrics updates (use_count, opportunities)
- Receiving embeddings from Memory Service as float32 arrays

**Never Contains:**
- LLM API calls
- Embedding generation (done by Memory Service)
- Any HTTP/network client code
- Tool interception (we are passive, we watch logs)

---

### ARCH-002: Python Memory Service

**Responsibility:** All LLM and embedding operations. **All HTTP/network calls live here.**

| Module | Purpose | Model Tier |
|--------|---------|------------|
| Memory Writer | Episode → ops[] extraction | strong_model |
| Retrieval | embed_text, compose_context (optional) | fast_model |
| CR-Memory | Background promotion/deprecation | rules only (v1) |

**Owns:**
- All LLM API calls (Memory Writer)
- All embedding generation (returns vectors to daemon via IPC)
- CR-Memory evaluation logic

**Never Contains:**
- File watching
- Direct database read/write (all storage via daemon IPC)
- sqlite-vec queries (daemon handles vector search)
- MCP protocol handling

---

### ARCH-003: Storage Layer

**Responsibility:** Persistence and vector search

| Database | Location | Contains |
|----------|----------|----------|
| Global DB | `~/.sqrl/squirrel.db` | memories (scope=global), memory_metrics, evidence |
| Project DB | `<repo>/.sqrl/squirrel.db` | memories (scope=project/repo_path), episodes, evidence, memory_metrics |

**Tables:** See SCHEMAS.md for full definitions.

| Table | Purpose |
|-------|---------|
| memories | Core memory storage with embeddings |
| evidence | Links memories to source episodes |
| memory_metrics | Tracks use_count, opportunities, regret for CR-Memory |
| episodes | Raw episode timeline (events_json) |

---

## Data Flow

### FLOW-001: Passive Ingestion

```
1. User codes with AI CLI
2. CLI writes to log file
3. Daemon detects file change (notify)
4. Daemon parses new log entries, buffers events
5. Episode boundary triggered:
   - Task boundary detected (new user query starts different task)
   - High-impact event resolved (severe frustration + success)
   - User explicit flush (sqrl flush)
6. Daemon inserts episodes row with events_json
7. Daemon calls Memory Service: ingest_episode(
     episode_id, project_id, scope, owner, episode_summary,
     recent_memories, policy_hints
   )
8. Memory Writer runs single strong-model prompt
9. Memory Writer returns JSON:
   - ops[] (ADD / UPDATE / DEPRECATE / IGNORE)
   - episode_evidence
10. Daemon commit layer for each op:
    - ADD: Insert memories row (status='provisional'), call embed_text,
           store embedding, insert evidence row, init memory_metrics
    - UPDATE: Mark old memory deprecated, insert new memory
    - DEPRECATE: Mark target memory status='deprecated'
11. New memories immediately available for next task
```

**Key Points:**
- Memory Writer does NOT return embeddings
- Embedding generated in commit phase (daemon calls embed_text)
- Commit is synchronous - next get_task_context sees new memories immediately

**Latency Target:** <100ms for steps 3-6, async for 7-11

---

### FLOW-002: Context Retrieval

```
1. AI CLI starts new task
2. CLI calls MCP tool (squirrel_get_task_context)
3. Daemon receives request: get_task_context(
     project_root, task, owner_type, owner_id, token_budget
   )
4. Daemon calls Memory Service: embed_text(task) → task_vector
5. Daemon queries SQLite:
   a. Filter: project_id, scope, owner_type/owner_id, status IN ('provisional','active')
   b. Key-based lookup first (project.*, user.* keys)
   c. Vector search using task_vector for semantic recall
6. Daemon ranks candidates:
   - Semantic similarity (base weight)
   - Status: active > provisional
   - Tier: emergency/long_term > short_term
   - Kind: invariant/preference > pattern > note/guard
   - Boost: log(1 + use_count)
7. Daemon optionally calls Memory Service: compose_context(
     task, memories, token_budget
   ) OR uses templates (v1)
8. Daemon returns to MCP/CLI:
   - context_prompt
   - memory_ids (used memories)
9. Daemon updates memory_metrics for each used memory:
   - use_count++
   - opportunities++
10. CLI injects context into prompt
```

**Latency Target:** <500ms total, <20ms for trivial tasks (fast path at step 5b)

---

### FLOW-003: Guard Context Injection

Guards are memories with `kind='guard'` and typically `tier='emergency'`. They represent "don't do X" knowledge.

**Important:** Squirrel is passive (log watching). We do NOT intercept tool execution. Guards are enforced through context injection - we inform the AI, and the AI decides whether to obey.

```
Write-time (during FLOW-001):
1. Memory Writer emits ADD op with kind='guard', polarity=-1
2. Guard stored as regular memory with high priority

Retrieval-time (during FLOW-002):
1. get_task_context retrieves relevant memories
2. Guards (tier='emergency') get highest priority in ranking
3. Guard text injected prominently in context
4. AI reads warning and (hopefully) obeys

Example injected context:
  "WARNING: Do not use DROP TABLE without explicit user confirmation.
   This caused data loss on 2025-11-15."
```

**No interception:** We cannot block tool calls. We inform, AI decides.

---

### FLOW-004: CR-Memory Evaluation

```
Trigger:
- Periodic cron (e.g., daily)
- After N episodes processed

For each memory m (status='provisional' or 'active'):
1. Read memory_metrics(m):
   - opportunities
   - use_count
   - suspected_regret_hits
   - estimated_regret_saved
   - last_used_at
2. Load policy thresholds from memory_policy.toml
3. Evaluate:
   a. If not enough opportunities → skip (not enough evidence)
   b. Calculate use_ratio = use_count / opportunities
   c. If use_ratio >= min_use_ratio AND regret_hits >= min_regret_hits:
      → Promote: status='active', maybe tier='long_term', extend expires_at
   d. If use_ratio <= max_use_ratio AND opportunities >= deprecation threshold:
      → Deprecate: status='deprecated'
   e. Check time-based decay (days since last_used_at)
   f. Otherwise: keep current status
4. Write updated status/tier/expires_at to memories table
```

**Note:** CR-Memory is internal background job, no external IPC. Runs inside Python Memory Service or triggered by daemon.

---

### FLOW-005: Memory Search

```
1. User runs `sqrl search "query"`
2. CLI sends request to daemon
3. Daemon calls Memory Service: embed_text(query) → query_vector
4. Daemon queries sqlite-vec for similar memories
5. Daemon returns ranked results to CLI
6. CLI displays to user
```

**Latency Target:** <200ms

---

### FLOW-006: sqrl init Historical Processing (ADR-011)

```
1. User runs `sqrl init`
2. Daemon scans CLI log folders for sessions mentioning this project
3. Sort all sessions by timestamp (earliest first)
4. Set timeline_origin = timestamp of earliest session
5. For each session chronologically:
   a. Parse session into episodes
   b. For each episode:
      - Call Memory Writer (FLOW-001 steps 7-10)
      - Memories created with timestamps relative to episode time
   c. After episode batch: run CR-Memory evaluation (FLOW-004)
      - Memories from earlier episodes have opportunities from later episodes
      - Promotion/deprecation based on actual historical usage
6. Final state: memories have earned tiers based on full history
7. Configure MCP, inject instructions (see INTERFACES.md INIT-001)
```

**Key Difference from Live Processing:**
- Live: CR-Memory runs periodically, memories start cold
- Init: CR-Memory simulated after each batch, memories earn tiers immediately

**Result:** User gets long_term memories from first query - no waiting period.

---

## Platform Considerations

| Platform | Log Locations | Socket Path | Notes |
|----------|---------------|-------------|-------|
| macOS | `~/Library/Application Support/Claude/` | `/tmp/sqrl_agent.sock` | Standard |
| Linux | `~/.config/Claude/` | `/tmp/sqrl_agent.sock` | XDG compliant |
| Windows | `%APPDATA%\Claude\` | `\\.\pipe\sqrl_agent` | Named pipe |

## Security Boundaries

| Boundary | Enforcement |
|----------|-------------|
| No network for daemon | Rust compile-time (no reqwest) |
| LLM keys in Memory Service only | Environment variables, not config |
| Project isolation | Separate SQLite per project |
| No cloud sync | Feature flag, default off |
| No secrets in memories | Memory Writer constraint |

## Python Package Structure

```
src/sqrl/
├── __init__.py          # Package exports
├── chunking.py          # Event chunking for Memory Writer (PROMPT-001)
├── cr_memory.py         # CR-Memory evaluation logic (FLOW-004)
├── embeddings.py        # Embedding generation (IPC-002)
├── ingest.py            # Ingest pipeline orchestration
├── retrieval.py         # Memory retrieval and ranking
├── db/
│   ├── __init__.py
│   ├── repository.py    # Database repository pattern
│   └── schema.py        # SQLite schema (SCHEMA-001 to 004)
├── ipc/
│   ├── __init__.py
│   ├── handlers.py      # IPC method handlers (IPC-001 to IPC-005)
│   └── server.py        # JSON-RPC 2.0 server over Unix socket
├── memory_writer/
│   ├── __init__.py
│   ├── models.py        # Memory operation data models
│   ├── prompts.py       # LLM prompts for Memory Writer (PROMPT-001)
│   └── writer.py        # Memory Writer implementation
└── parsers/
    ├── __init__.py
    ├── base.py          # BaseParser interface, Event/Episode dataclasses
    └── claude_code.py   # Claude Code log parser
```

| Module | Purpose | Spec Reference |
|--------|---------|----------------|
| sqrl.chunking | Split events into chunks for Memory Writer | PROMPT-001 |
| sqrl.cr_memory | CR-Memory promotion/deprecation logic | FLOW-004 |
| sqrl.embeddings | Embedding generation via OpenAI | IPC-002 |
| sqrl.ingest | Ingest pipeline orchestration | FLOW-001 |
| sqrl.retrieval | Memory retrieval and ranking | FLOW-002 |
| sqrl.db.repository | Database repository pattern | - |
| sqrl.db.schema | SQLite schema, init, connections | SCHEMA-001 to 004 |
| sqrl.ipc.handlers | IPC method handlers | IPC-001 to IPC-005 |
| sqrl.ipc.server | JSON-RPC 2.0 server | - |
| sqrl.memory_writer | Episode → ops extraction | PROMPT-001 |
| sqrl.parsers.base | Parser interface, data models | - |
| sqrl.parsers.claude_code | Parse Claude Code JSONL logs | - |

---

## Rust Daemon Structure

```
daemon/
├── Cargo.toml           # Dependencies and build config
└── src/
    ├── main.rs          # Entry point, CLI dispatch
    ├── config.rs        # Configuration structs
    ├── error.rs         # Error types (thiserror)
    ├── cli/
    │   ├── mod.rs       # CLI command definitions (clap)
    │   ├── init.rs      # sqrl init (CLI-002)
    │   ├── search.rs    # sqrl search (CLI-003)
    │   ├── forget.rs    # sqrl forget (CLI-004)
    │   ├── export.rs    # sqrl export (CLI-005)
    │   ├── import.rs    # sqrl import (CLI-006)
    │   ├── status.rs    # sqrl status (CLI-007)
    │   ├── config.rs    # sqrl config (CLI-008)
    │   ├── sync.rs      # sqrl sync (CLI-009)
    │   ├── flush.rs     # sqrl flush (CLI-010)
    │   ├── policy.rs    # sqrl policy (CLI-011)
    │   ├── mcp.rs       # sqrl mcp (CLI-012)
    │   └── daemon.rs    # sqrl daemon (CLI-013)
    ├── db/
    │   ├── mod.rs       # Database operations wrapper
    │   └── schema.rs    # Schema initialization (SCHEMA-001 to 004)
    ├── ipc/
    │   └── mod.rs       # JSON-RPC 2.0 client/server
    └── watcher/
        └── mod.rs       # Log file watcher (notify)
```

| Module | Purpose | Spec Reference |
|--------|---------|----------------|
| daemon::cli | CLI command definitions and dispatch | CLI-001 to CLI-013 |
| daemon::db | SQLite operations, schema init | SCHEMA-001 to 004 |
| daemon::ipc | IPC client/server over Unix socket | IPC-001 to IPC-005 |
| daemon::watcher | File system event watching | FLOW-001 |
| daemon::config | Configuration loading and structs | - |
| daemon::error | Error types with thiserror | - |

---

## Extension Points

| Point | Mechanism | Example |
|-------|-----------|---------|
| New CLI support | Log parser plugins | Add Aider support |
| New LLM provider | Memory Service config | Switch to local LLM |
| New embedding model | Memory Service config | Use Cohere embeddings |
| Team sync | Future daemon module | Cloud storage backend (v2) |
| Multi-IDE | Episodes format | Each IDE translates logs to episodes |
