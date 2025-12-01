# Squirrel Development Plan (v1)

Modular development plan for v1 architecture with Rust daemon + Python Memory Service communicating via Unix socket IPC.

## v1 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        RUST DAEMON                              │
│  Log Watcher → Events → Episodes → IPC → Memory Service         │
│  SQLite/sqlite-vec storage                                      │
│  MCP Server (2 tools)                                           │
│  CLI (sqrl init/config/daemon/status/mcp)                       │
└─────────────────────────────────────────────────────────────────┘
                             ↕ Unix socket IPC
┌─────────────────────────────────────────────────────────────────┐
│                     PYTHON MEMORY SERVICE                       │
│  Router Agent (dual-mode: INGEST + ROUTE)                       │
│  ONNX Embeddings (all-MiniLM-L6-v2, 384-dim)                    │
│  Retrieval + "Why" generation                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Development Tracks

```
Phase 0: Scaffolding (1-2 days)
    |
    v
+-------+-------+-------+
|       |       |       |
v       v       v       v
Track A Track B Track C Track D
(Rust   (Rust   (Python (Python
Storage) Daemon) Router) Retrieval)
    |       |       |       |
    +---+---+       +---+---+
        |               |
        v               v
    Week 1-2        Week 2-3
        |               |
        +-------+-------+
                |
                v
            Track E (Week 3)
            MCP Layer
                |
                v
            Phase X (Week 4+)
            Hardening
```

---

## Phase 0 – Scaffolding (1-2 days)

### Rust Module (`agent/`)

- [ ] Create `Cargo.toml`:
  - `tokio` (async runtime)
  - `rusqlite` + `sqlite-vec` (storage)
  - `serde`, `serde_json` (serialization)
  - `notify` (file watching)
  - `clap` (CLI)
  - `uuid`, `chrono` (ID, timestamps)

- [ ] Directory structure:
  ```
  agent/src/
  ├── main.rs
  ├── lib.rs
  ├── daemon.rs       # Process management
  ├── watcher.rs      # Log file watching
  ├── storage.rs      # SQLite + sqlite-vec
  ├── events.rs       # Event/Episode structs
  ├── config.rs       # Config management
  ├── mcp.rs          # MCP server
  └── ipc.rs          # Unix socket client
  ```

### Python Module (`memory_service/`)

- [ ] Create `pyproject.toml`:
  - `onnxruntime` (embeddings)
  - `sentence-transformers` (model loading)
  - `anthropic` / `openai` (Router Agent)
  - `pydantic` (schemas)

- [ ] Directory structure:
  ```
  memory_service/
  ├── squirrel_memory/
  │   ├── __init__.py
  │   ├── server.py       # Unix socket server
  │   ├── router_agent.py # Dual-mode router
  │   ├── embeddings.py   # ONNX embeddings
  │   ├── retrieval.py    # Similarity search
  │   └── schemas/
  └── tests/
  ```

- [ ] CI basics (GitHub Actions: lint, test)

---

## Track A – Rust: Storage + Config + Events (Week 1)

### A1. Storage layer (`storage.rs`)

- [ ] SQLite + sqlite-vec initialization:
  ```rust
  fn init_project_db(path: &Path) -> Result<Connection>
  fn init_global_db() -> Result<Connection>  // ~/.sqrl/squirrel.db
  ```

- [ ] Create tables (see ARCHITECTURE.md for full schema):
  ```sql
  CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    content_hash TEXT NOT NULL UNIQUE,
    content TEXT NOT NULL,
    memory_type TEXT NOT NULL,
    repo TEXT NOT NULL,
    embedding BLOB,
    confidence REAL NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
  );

  CREATE TABLE events (id, cli, repo, event_type, content, file_paths, timestamp, processed);
  CREATE TABLE episodes (id, repo, cli, start_ts, end_ts, event_ids, processed);
  ```

- [ ] CRUD operations for events, episodes, memories

### A2. Event model (`events.rs`)

- [ ] Define `Event` struct:
  ```rust
  struct Event {
      id: String,
      cli: String,        // claude_code | codex | cursor | gemini
      repo: String,
      event_type: String, // user_message | assistant_response | tool_use | tool_result
      content: String,
      file_paths: Vec<String>,
      timestamp: DateTime<Utc>,
      processed: bool,
  }
  ```

- [ ] Define `Episode` struct:
  ```rust
  struct Episode {
      id: String,
      repo: String,
      cli: String,
      start_ts: DateTime<Utc>,
      end_ts: DateTime<Utc>,
      event_ids: Vec<String>,
      processed: bool,
  }
  ```

- [ ] Dedup hash computation
- [ ] Episode grouping (same repo + CLI + time gap < 20min)

### A3. Config (`config.rs`)

- [ ] Path helpers:
  ```rust
  fn get_sqrl_dir() -> PathBuf          // ~/.sqrl/
  fn get_project_sqrl_dir(repo: &Path) -> PathBuf  // <repo>/.sqrl/
  ```

- [ ] Config management:
  ```rust
  struct Config {
      user_id: String,
      anthropic_api_key: Option<String>,
      default_model: String,
      socket_path: String,
  }
  ```

- [ ] Projects registry (track which repos are initialized)

### A4. CLI skeleton (`main.rs`)

- [ ] Command structure:
  ```
  sqrl init              # Initialize project
  sqrl config            # Set user_id, API keys
  sqrl daemon start      # Start daemon
  sqrl daemon stop       # Stop daemon
  sqrl status            # Show memory stats
  sqrl mcp               # Run MCP server
  ```

- [ ] Implement `sqrl init`:
  - Create `<repo>/.sqrl/` directory
  - Initialize `squirrel.db`
  - Register in projects registry

---

## Track B – Rust: Daemon + Watcher + IPC (Week 1-2)

### B1. Daemon (`daemon.rs`)

- [ ] Daemon structure:
  ```rust
  struct Daemon {
      watchers: Vec<LogWatcher>,
      memory_service: Option<ChildProcess>,
      ipc_client: IpcClient,
  }
  ```

- [ ] Startup sequence:
  1. Load projects registry
  2. For each project, spawn watcher
  3. Spawn Python Memory Service as child process
  4. Connect to Memory Service via Unix socket

- [ ] Shutdown: kill Python, stop watchers

### B2. Log Watchers (`watcher.rs`)

- [ ] Multi-CLI log discovery:
  ```rust
  fn find_log_paths(project: &Path) -> Vec<(CLI, PathBuf)> {
      // ~/.claude/projects/<encoded>/    → Claude Code
      // ~/.codex-cli/logs/               → Codex CLI
      // ~/.gemini/logs/                  → Gemini CLI
      // ~/.cursor-tutor/logs/            → Cursor
  }
  ```

- [ ] JSONL tailer using `notify` crate:
  ```rust
  struct LogWatcher {
      cli: CLI,
      log_dir: PathBuf,
      file_positions: HashMap<PathBuf, u64>,
  }
  ```

- [ ] Line parsers for each CLI format
- [ ] Write events to SQLite

### B3. Episode Batching

- [ ] Background task:
  ```rust
  async fn batch_events_loop(interval: Duration) {
      // Every 30 seconds or 20 events:
      // 1. Query unprocessed events
      // 2. Group into episodes (same repo + CLI + time gap < 20min)
      // 3. Send to Python via IPC
      // 4. Mark as processed
  }
  ```

### B4. IPC Client (`ipc.rs`)

- [ ] Unix socket client:
  ```rust
  struct IpcClient {
      socket_path: PathBuf,
  }

  impl IpcClient {
      async fn router_agent(&self, mode: &str, payload: Value) -> Result<Value>
      async fn fetch_memories(&self, params: FetchParams) -> Result<Vec<Memory>>
  }
  ```

- [ ] JSON-RPC style protocol:
  ```json
  {"method": "router_agent", "params": {...}, "id": 123}
  {"result": {...}, "id": 123}
  ```

---

## Track C – Python: Router Agent + Server (Week 1-2)

### C1. Unix Socket Server (`server.py`)

- [ ] Socket server listening at `/tmp/sqrl_router.sock`:
  ```python
  async def handle_connection(reader, writer):
      request = await read_json(reader)
      if request["method"] == "router_agent":
          result = await router_agent(request["params"])
      elif request["method"] == "fetch_memories":
          result = await fetch_memories(request["params"])
      await write_json(writer, {"result": result, "id": request["id"]})
  ```

### C2. Router Agent (`router_agent.py`)

- [ ] Dual-mode router:
  ```python
  async def router_agent(mode: str, payload: dict) -> dict:
      if mode == "ingest":
          return await ingest_mode(payload)
      elif mode == "route":
          return await route_mode(payload)
  ```

- [ ] INGEST mode (write-side):
  ```python
  async def ingest_mode(payload: dict) -> dict:
      """
      Input: {episode: {...}, events: [...]}
      Output: {action: ADD|UPDATE|NOOP, memory?: {...}, confidence: float}

      LLM prompt determines:
      - Is there a memorable pattern here?
      - What type? (user_style, project_fact, pitfall, recipe)
      - Does it duplicate existing memory?
      """
  ```

- [ ] ROUTE mode (read-side):
  ```python
  async def route_mode(payload: dict) -> dict:
      """
      Input: {task: str, candidates: [...]}
      Output: {selected: [...], why: {...}}

      LLM selects which candidates are relevant and generates "why"
      """
  ```

### C3. Schemas (`schemas/`)

- [ ] Memory schema:
  ```python
  class Memory(BaseModel):
      id: str
      content_hash: str
      content: str
      memory_type: Literal["user_style", "project_fact", "pitfall", "recipe"]
      repo: str
      embedding: Optional[bytes]
      confidence: float
      created_at: datetime
      updated_at: datetime
  ```

- [ ] IPC request/response schemas:
  ```python
  class RouterAgentRequest(BaseModel):
      mode: Literal["ingest", "route"]
      payload: dict

  class FetchMemoriesRequest(BaseModel):
      repo: str
      task: Optional[str]
      memory_types: Optional[list[str]]
      max_results: int = 20
  ```

---

## Track D – Python: Embeddings + Retrieval (Week 2-3)

### D1. ONNX Embeddings (`embeddings.py`)

- [ ] Load all-MiniLM-L6-v2 via ONNX:
  ```python
  class EmbeddingModel:
      def __init__(self):
          self.session = ort.InferenceSession("all-MiniLM-L6-v2.onnx")

      def embed(self, text: str) -> np.ndarray:
          # Tokenize and run inference
          # Returns 384-dim vector
  ```

- [ ] Batch embedding for memory items
- [ ] Cache model in memory

### D2. Retrieval (`retrieval.py`)

- [ ] Similarity search:
  ```python
  async def retrieve_candidates(
      repo: str,
      task: str,
      memory_types: list[str] = None,
      top_k: int = 20
  ) -> list[Memory]:
      # 1. Embed task description
      # 2. Query sqlite-vec for similar memories
      # 3. Filter by repo and types
      # 4. Return top candidates
  ```

- [ ] "Why" generation (heuristic templates):
  ```python
  def generate_why(memory: Memory, task: str) -> str:
      templates = {
          "user_style": "Relevant because {task_verb} matches your preference for {pattern}",
          "project_fact": "Relevant because {task_verb} involves {entity}",
          "pitfall": "Warning: {issue} may occur when {task_verb}",
          "recipe": "Useful pattern for {task_verb}",
      }
      # Simple keyword matching + template filling
  ```

### D3. Memory Update Logic

- [ ] Confidence threshold (0.7):
  ```python
  async def process_ingest_result(result: dict, existing: list[Memory]) -> Memory | None:
      if result["action"] == "NOOP":
          return None
      if result["confidence"] < 0.7:
          return None

      if result["action"] == "ADD":
          return create_memory(result["memory"])
      elif result["action"] == "UPDATE":
          return update_memory(result["memory"], existing)
  ```

---

## Track E – MCP Layer (Week 3)

### E1. MCP Server (`mcp.rs`)

- [ ] MCP stdio protocol handler
- [ ] 2 tool definitions:
  ```rust
  // Primary tool
  ToolDefinition {
      name: "squirrel_get_task_context",
      description: "Get task-aware memory with 'why' explanations",
      input_schema: {
          "project_root": "string (required)",
          "task": "string (required)",
          "max_tokens": "integer (default: 400)",
          "memory_types": "array of strings (optional)"
      }
  }

  // Search tool
  ToolDefinition {
      name: "squirrel_search_memory",
      description: "Semantic search across all memory",
      input_schema: {
          "project_root": "string (required)",
          "query": "string (required)",
          "top_k": "integer (default: 10)",
          "memory_types": "array of strings (optional)"
      }
  }
  ```

### E2. Tool Handlers

- [ ] `squirrel_get_task_context`:
  ```rust
  async fn handle_get_task_context(args: Value) -> Result<Value> {
      // 1. Retrieve candidates via IPC (fetch_memories)
      // 2. Call Router Agent (route mode) via IPC
      // 3. Format response with "why" explanations
      // 4. Respect max_tokens budget
  }
  ```

- [ ] `squirrel_search_memory`:
  ```rust
  async fn handle_search_memory(args: Value) -> Result<Value> {
      // 1. Retrieve via IPC (fetch_memories with query)
      // 2. Return with similarity scores
  }
  ```

### E3. `sqrl mcp` Command

- [ ] Entry point:
  ```rust
  fn cmd_mcp() {
      ensure_daemon_running()?;
      let server = McpServer::new();
      server.run_stdio();  // Blocks, handles MCP protocol
  }
  ```

### E4. Claude Code Integration

- [ ] MCP config snippet:
  ```json
  {
    "mcpServers": {
      "squirrel": {
        "command": "sqrl",
        "args": ["mcp"]
      }
    }
  }
  ```

---

## Phase X – Hardening (Week 4+)

### Logging & Observability

- [ ] Structured logging (Rust: `tracing`, Python: `structlog`)
- [ ] Metrics: events/episodes/memories processed, latency

### Testing

- [ ] Unit tests:
  - Rust: storage, events, episode grouping
  - Python: router agent, embeddings, retrieval

- [ ] Integration tests:
  - Full flow: log → event → episode → memory
  - MCP tool calls end-to-end

### CLI Polish

- [ ] `sqrl status`: daemon status, project stats, memory counts
- [ ] `sqrl config`: interactive API key setup

---

## Timeline Summary

| Week | Track A | Track B | Track C | Track D | Track E |
|------|---------|---------|---------|---------|---------|
| 0 | Scaffold | Scaffold | Scaffold | - | - |
| 1 | Storage, Events, Config, CLI | Daemon start | Socket server, Router Agent | - | - |
| 2 | - | Watchers, IPC client, Batch loop | - | Embeddings, Retrieval | - |
| 3 | - | Integration | - | Update logic | MCP layer (2 tools) |
| 4+ | - | Hardening | - | Why templates | Integration |

---

## Team Assignment (3 developers)

**Developer 1 (Rust focus):**
- Phase 0: Rust scaffold
- Track A: All
- Track B: All
- Track E: All

**Developer 2 (Python focus):**
- Phase 0: Python scaffold
- Track C: All
- Track D: All

**Developer 3 (Full-stack / Prompts):**
- Phase 0: CI, docs
- Router Agent prompts
- Phase X: Testing, documentation

---

## v2 Scope (Future)

Not in v1:
- Hooks output for Claude Code / Gemini CLI
- File injection for AGENTS.md / GEMINI.md
- Cloud sync
- Team memory sharing
- Web dashboard
