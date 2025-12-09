# Squirrel Architecture

High-level system boundaries and data flow.

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
│  │                 ┌─────────────────┐                       │  │
│  │                 │   IPC Router    │                       │  │
│  │                 │  (JSON-RPC 2.0) │                       │  │
│  │                 └────────┬────────┘                       │  │
│  │                          │                                │  │
│  │         ┌────────────────┼────────────────┐               │  │
│  │         ▼                ▼                ▼               │  │
│  │  ┌───────────┐    ┌───────────┐    ┌───────────┐         │  │
│  │  │  SQLite   │    │  Events   │    │  Config   │         │  │
│  │  │  Storage  │    │  Queue    │    │  Manager  │         │  │
│  │  └───────────┘    └───────────┘    └───────────┘         │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
│                              │ Unix Socket                       │
│                              ▼                                   │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                   PYTHON AGENT                             │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │  │
│  │  │  Ingestion  │  │  Retrieval  │  │  Conflict   │        │  │
│  │  │    Agent    │  │    Agent    │  │  Resolver   │        │  │
│  │  │ (PydanticAI)│  │ (PydanticAI)│  │ (PydanticAI)│        │  │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘        │  │
│  │         │                │                │               │  │
│  │         └────────────────┼────────────────┘               │  │
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

**Responsibility:** All local I/O, no LLM logic

| Module | Crate | Purpose |
|--------|-------|---------|
| Log Watcher | notify | File system events for CLI logs |
| MCP Server | rmcp | Model Context Protocol server |
| CLI Handler | clap | User-facing commands |
| IPC Router | tokio | JSON-RPC over Unix socket |
| SQLite Storage | rusqlite + sqlite-vec | Memories, events, config |
| Events Queue | crossbeam | Buffering before agent processing |
| Config Manager | serde | User and project settings |

**Never Contains:**
- LLM API calls
- Memory extraction logic
- Semantic similarity computation (done via sqlite-vec)

---

### ARCH-002: Python Agent

**Responsibility:** All LLM operations

| Module | Library | Purpose |
|--------|---------|---------|
| Ingestion Agent | PydanticAI | Episode → memories extraction |
| Retrieval Agent | PydanticAI | Task → relevant memories |
| Conflict Resolver | PydanticAI | Semantic conflict detection |
| LLM Router | httpx | Provider switching, rate limiting |
| Embedding Client | openai | Vector generation |

**Never Contains:**
- File watching
- Direct database writes (goes through daemon IPC)
- MCP protocol handling

---

### ARCH-003: Storage Layer

**Responsibility:** Persistence and vector search

| Database | Location | Contents |
|----------|----------|----------|
| Global DB | `~/.sqrl/squirrel.db` | User profile, global memories, access logs |
| Project DB | `<repo>/.sqrl/squirrel.db` | Project memories, raw events |

**Vector Search:**
- sqlite-vec extension for cosine similarity
- 1536-dim OpenAI embeddings (default)
- Indexed for top-k queries

---

## Data Flow

### FLOW-001: Passive Ingestion

```
1. User codes with AI CLI
2. CLI writes to log file
3. Daemon detects file change (notify)
4. Daemon parses new log entries
5. Events queued in memory
6. Batch trigger (time/count/shutdown)
7. Daemon calls agent via IPC (ingest_episode)
8. Agent extracts memories (LLM)
9. Agent returns memories to daemon
10. Daemon writes to SQLite
11. Daemon updates indexes
```

**Latency Target:** <100ms for steps 3-6, async for 7-11

---

### FLOW-002: Context Retrieval

```
1. AI CLI starts new task
2. CLI calls MCP tool (squirrel_get_task_context)
3. Daemon receives MCP request
4. Daemon queries keyed facts (fast path)
5. Daemon calls agent via IPC (get_task_context)
6. Agent embeds task description
7. Agent queries sqlite-vec for similar memories
8. Agent composes context (LLM)
9. Agent returns context to daemon
10. Daemon returns MCP response
11. CLI injects context into prompt
```

**Latency Target:** <500ms total, <20ms for trivial tasks (fast path)

---

### FLOW-003: Memory Search

```
1. User runs `sqrl search "query"`
2. CLI sends IPC (search_memories)
3. Daemon embeds query
4. Daemon queries sqlite-vec
5. Daemon returns ranked results
6. CLI displays to user
```

**Latency Target:** <200ms

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
| LLM keys in agent only | Environment variables, not config |
| Project isolation | Separate SQLite per project |
| No cloud sync | Feature flag, default off |

## Extension Points

| Point | Mechanism | Example |
|-------|-----------|---------|
| New CLI support | Log parser plugins | Add Aider support |
| New LLM provider | Agent config | Switch to local LLM |
| New embedding model | Agent config | Use Cohere embeddings |
| Team sync | Future daemon module | Cloud storage backend |
