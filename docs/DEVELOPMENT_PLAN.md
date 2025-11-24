# Squirrel Development Plan

Modular development plan organized by tracks. Team members can work in parallel on different tracks after Phase 0.

## Overview

```
Phase 0: Scaffolding (1-2 days)
    |
    v
+-------+-------+-------+-------+
|       |       |       |       |
v       v       v       v       v
Track A Track B Track C Track D Track E
(Rust   (Rust   (Python (Python (MCP
Storage) Daemon) Schema) Logic)  Layer)
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
                |
                v
            Phase X (Week 4+)
            Hardening
```

---

## Phase 0 – Repo & Scaffolding (1-2 days)

**Owner:** 1-2 people
**Parallel:** Yes (can split Rust/Python setup)

### Tasks

- [ ] Set up mono-repo structure:
  ```
  Squirrel/
  ├── agent/              # Rust module
  ├── memory_service/     # Python module
  └── docs/
  ```

- [ ] Rust toolchain setup (`agent/`):
  - [ ] Create `Cargo.toml` with dependencies:
    - `tokio` (async runtime)
    - `sqlx` or `rusqlite` (SQLite)
    - `serde`, `serde_json` (serialization)
    - `notify` (file watching)
    - `reqwest` (HTTP client)
    - `clap` (CLI)
    - `uuid` (ID generation)
    - `chrono` (timestamps)
  - [ ] Set up `rustfmt.toml`, `clippy.toml`
  - [ ] Create directory structure:
    ```
    agent/src/
    ├── main.rs
    ├── lib.rs
    ├── daemon.rs
    ├── watcher.rs
    ├── storage.rs
    ├── events.rs
    ├── config.rs
    ├── mcp.rs
    └── http_client.rs
    ```

- [ ] Python toolchain setup (`memory_service/`):
  - [ ] Create `pyproject.toml` with dependencies:
    - `fastapi`
    - `uvicorn`
    - `pydantic`
    - `httpx`
    - `openai` / `anthropic`
    - `aiosqlite`
  - [ ] Set up `ruff`, `black`, `pytest`
  - [ ] Create directory structure:
    ```
    memory_service/
    ├── squirrel_memory/
    │   ├── __init__.py
    │   ├── server.py
    │   ├── schemas/
    │   ├── extraction/
    │   ├── update/
    │   └── views/
    └── tests/
    ```

- [ ] Add CI basics (GitHub Actions for lint/test)

---

## Track A – Rust: Storage + Config + Events (Week 1)

**Dependencies:** Phase 0
**Parallel with:** Track C

### A1. Storage layer (`storage.rs`)

- [ ] Implement `init_project_db(path: &Path) -> Result<Connection>`
- [ ] Create tables:
  ```sql
  -- events table
  -- episodes table
  -- memory_items table
  -- view_meta table
  ```
- [ ] Implement `init_user_db()` for `~/.sqrl/user.db`
- [ ] Create `user_style_items` table

### A2. Event model (`events.rs`)

- [ ] Define `Event` struct matching schema:
  ```rust
  struct Event {
      id: String,
      project_id: String,
      user_id: String,
      source_tool: SourceTool,
      session_id: Option<String>,
      timestamp: DateTime<Utc>,
      event_type: EventType,
      content_json: String,
      file_paths: Vec<String>,
      hash: String,
      processed_at: Option<DateTime<Utc>>,
  }
  ```
- [ ] Implement `compute_dedup_hash(event: &Event) -> String`
- [ ] Implement `insert_event(conn, event: &Event) -> Result<bool>` with dedup
- [ ] Implement `get_unsent_events(conn, limit: usize) -> Vec<Event>`
- [ ] Implement `mark_events_processed(conn, ids: &[String], ts: DateTime)`

### A3. Config & paths (`config.rs`, `utils.rs`)

- [ ] Implement path helpers:
  ```rust
  fn get_sqrl_dir() -> PathBuf      // ~/.sqrl/
  fn get_ctx_dir(project: &Path) -> PathBuf  // <repo>/.ctx/
  fn find_repo_root(path: &Path) -> Option<PathBuf>
  ```
- [ ] Implement config management:
  ```rust
  struct Config {
      user_id: String,
      llm: LlmConfig,
      daemon: DaemonConfig,
  }
  fn load_or_init_config() -> Result<Config>
  fn save_config(config: &Config) -> Result<()>
  ```
- [ ] Implement projects registry:
  ```rust
  fn load_projects() -> Vec<PathBuf>
  fn add_project(path: &Path) -> Result<()>
  fn remove_project(path: &Path) -> Result<()>
  ```
- [ ] Implement daemon/service metadata:
  ```rust
  fn read_daemon_info() -> Option<DaemonInfo>
  fn write_daemon_info(info: &DaemonInfo) -> Result<()>
  fn read_memory_service_info() -> Option<ServiceInfo>
  fn write_memory_service_info(info: &ServiceInfo) -> Result<()>
  ```

### A4. CLI skeleton (`main.rs`, `cli.rs`)

- [ ] Set up `clap` command structure:
  ```
  sqrl init
  sqrl config [--user-id <id>] [--api-key <key>]
  sqrl daemon start|stop|status
  sqrl status
  sqrl mcp
  ```
- [ ] Implement `sqrl init`:
  - Ensure `~/.sqrl/` exists
  - Create `.ctx/` in current repo
  - Initialize `data.db`
  - Register project in `projects.json`
- [ ] Implement `sqrl config` (basic key-value set/get)
- [ ] Stub `sqrl daemon start/stop` (placeholder)

---

## Track B – Rust: Daemon + Watcher + Python Client (Week 1-2)

**Dependencies:** Track A (storage, config)
**Parallel with:** Track C, Track D

### B1. Daemon (`daemon.rs`)

- [ ] Implement daemon process structure:
  ```rust
  struct Daemon {
      projects: Vec<ProjectWatcher>,
      python_service: Option<ChildProcess>,
      tcp_server: TcpListener,
  }
  ```
- [ ] Implement startup sequence:
  1. Load `projects.json`
  2. For each project, spawn watcher
  3. Spawn Python memory service
  4. Start TCP server on random port
  5. Write `daemon.json`
- [ ] Implement shutdown sequence:
  - Kill Python service
  - Stop watchers
  - Remove `daemon.json`
- [ ] Implement daemon auto-start check:
  ```rust
  fn ensure_daemon_running() -> Result<DaemonInfo>
  ```
- [ ] Implement local TCP API:
  - `POST /batch_events` → forward to Python
  - `GET /view/{view_name}` → forward to Python
  - `GET /search` → forward to Python
  - `GET /health`

### B2. Claude Code watcher (`watcher.rs`)

- [ ] Implement Claude log discovery:
  ```rust
  fn find_claude_log_dir(project: &Path) -> Option<PathBuf>
  // Maps project path to ~/.claude/projects/<encoded-path>/
  ```
- [ ] Implement JSONL tailer using `notify`:
  ```rust
  struct ClaudeWatcher {
      project_id: String,
      log_dir: PathBuf,
      file_positions: HashMap<PathBuf, u64>,
  }
  ```
- [ ] Implement line parser:
  ```rust
  fn parse_claude_line(line: &str) -> Result<Event>
  ```
- [ ] Wire watcher to storage:
  - On new line → parse → `insert_event()`

### B3. Python HTTP client (`http_client.rs`)

- [ ] Implement client struct:
  ```rust
  struct MemoryServiceClient {
      base_url: String,
      client: reqwest::Client,
  }
  ```
- [ ] Implement methods:
  ```rust
  async fn process_events(&self, req: ProcessEventsRequest) -> Result<ProcessEventsResponse>
  async fn get_user_style_view(&self, project_id: &str) -> Result<UserStyleView>
  async fn get_project_brief_view(&self, project_id: &str) -> Result<ProjectBriefView>
  async fn get_pitfalls_view(&self, project_id: &str, ...) -> Result<PitfallsView>
  async fn search_memory(&self, project_id: &str, query: &str, ...) -> Result<SearchResponse>
  ```
- [ ] Implement retry logic with exponential backoff
- [ ] Read connection info from `memory-service.json`

### B4. Batch loop in daemon

- [ ] Implement background task:
  ```rust
  async fn batch_events_loop(project_id: &str, interval: Duration)
  ```
- [ ] Trigger conditions:
  - Every 30 seconds
  - When unsent events >= 20
- [ ] On trigger:
  1. `get_unsent_events(limit=200)`
  2. Build `ProcessEventsRequest`
  3. Call Python service
  4. On success: `mark_events_processed()`
  5. On failure: log, retry with backoff

---

## Track C – Python: Schemas + Server Skeleton (Week 1-2)

**Dependencies:** Phase 0
**Parallel with:** Track A, Track B

### C1. Schemas (`schemas/`)

- [ ] Define Pydantic models in `schemas/events.py`:
  ```python
  class EventDTO(BaseModel):
      id: str
      project_id: str
      user_id: str
      source_tool: str
      session_id: Optional[str]
      timestamp: datetime
      event_type: str
      content_json: str
      file_paths: list[str]
      hash: str
  ```

- [ ] Define models in `schemas/memory.py`:
  ```python
  class Episode(BaseModel):
      id: str
      project_id: str
      user_id: str
      session_id: Optional[str]
      start_ts: datetime
      end_ts: datetime
      event_count: int
      summary: str
      importance: float

  class MemoryItem(BaseModel):
      id: str
      project_id: str
      user_id: Optional[str]
      scope: Literal["user", "project"]
      type: Literal["user_style", "project_fact", "pitfall", "recipe"]
      key: str
      content: str
      tags: list[str]
      importance: float
      source_episode_id: Optional[str]
      source_event_ids: list[str]
  ```

- [ ] Define models in `schemas/views.py`:
  ```python
  class UserStyleItem(BaseModel):
      key: str
      summary: str
      tags: list[str]
      importance: float
      last_updated_at: datetime
      source_memory_ids: list[str]

  class UserStyleView(BaseModel):
      type: Literal["user_style_view"]
      mode: Literal["core", "full"]
      project_id: str
      user_id: str
      generated_at: datetime
      token_estimate: int
      items: list[UserStyleItem]
      markdown: str

  class ProjectBriefView(BaseModel):
      type: Literal["project_brief_view"]
      mode: Literal["core", "full"]
      project_id: str
      generated_at: datetime
      token_estimate: int
      modules: list[ModuleInfo]
      key_facts: list[str]
      markdown: str

  class PitfallsView(BaseModel):
      type: Literal["pitfalls_view"]
      project_id: str
      generated_at: datetime
      token_estimate: int
      has_relevant_pitfalls: bool
      items: list[PitfallItem]
      markdown: str

  class SelectedMemory(BaseModel):
      memory_id: str
      type: str
      key: str
      content: str
      importance: float
      reason: str  # why this memory is relevant to the task

  class TaskContext(BaseModel):
      type: Literal["task_context"]
      project_id: str
      user_id: str
      task_description: str
      generated_at: datetime
      token_estimate: int
      has_relevant_memory: bool
      selected_memories: list[SelectedMemory]
      markdown: str

  class SearchResult(BaseModel):
      memory_id: str
      type: str
      key: str
      content: str
      tags: list[str]
      importance: float
      recency_days: int
      score: float
      reason: str  # explanation of relevance
      source: SearchSource  # {episode_ids, file_paths}
  ```

- [ ] Define request/response models in `schemas/api.py`:
  ```python
  class ProcessEventsRequest(BaseModel):
      project_id: str
      user_id: str
      events: list[EventDTO]

  class ProcessEventsResponse(BaseModel):
      episode_ids: list[str]
      memory_item_ids: list[str]
  ```

### C2. HTTP server (`server.py`)

- [ ] Set up FastAPI app:
  ```python
  app = FastAPI(title="Squirrel Memory Service")
  ```

- [ ] Implement stub endpoints:
  ```python
  @app.post("/process_events")
  async def process_events(request: ProcessEventsRequest) -> ProcessEventsResponse:
      # Stub: return empty
      return ProcessEventsResponse(episode_ids=[], memory_item_ids=[])

  @app.get("/views/user_style")
  async def get_user_style_view(
      project_id: str,
      user_id: str = None,
      mode: str = "core",
      context_budget_tokens: int = 256
  ) -> UserStyleView:
      # Stub: return dummy view
      ...

  @app.get("/views/project_brief")
  async def get_project_brief_view(
      project_id: str,
      mode: str = "core",
      context_budget_tokens: int = 256
  ) -> ProjectBriefView:
      # Stub
      ...

  @app.get("/views/pitfalls")
  async def get_pitfalls_view(
      project_id: str,
      scope_paths: list[str] = None,
      task_description: str = None,
      context_budget_tokens: int = 256
  ) -> PitfallsView:
      # Stub
      ...

  @app.post("/task_context")
  async def get_task_context(request: TaskContextRequest) -> TaskContext:
      # Stub: primary output tool
      ...

  @app.get("/search_memory")
  async def search_memory(
      project_id: str,
      query: str,
      top_k: int = 5,
      types: list[str] = None,
      scope_paths: list[str] = None
  ) -> SearchResponse:
      # Stub
      ...
  ```

- [ ] Add CLI entry point:
  ```python
  # __main__.py
  if __name__ == "__main__":
      import uvicorn
      uvicorn.run("squirrel_memory.server:app", host="127.0.0.1", port=args.port)
  ```

### C3. Storage adapter (`storage.py`)

- [ ] Implement SQLite connection helper:
  ```python
  def get_project_db(project_id: str) -> aiosqlite.Connection:
      ctx_path = Path(project_id) / ".ctx" / "data.db"
      return await aiosqlite.connect(ctx_path)
  ```

- [ ] Implement CRUD operations:
  ```python
  async def save_episodes(conn, episodes: list[Episode])
  async def save_memory_items(conn, items: list[MemoryItem])
  async def query_memory_items(conn, project_id: str, type: str = None, ...) -> list[MemoryItem]
  async def get_view_meta(conn, view_name: str) -> Optional[ViewMeta]
  async def update_view_meta(conn, view_name: str, meta: ViewMeta)
  async def count_events(conn, project_id: str) -> int
  ```

---

## Track D – Python: Extraction + Update + Views (Week 2-3)

**Dependencies:** Track C
**Parallel with:** Track B (later stages)

### D1. Episode grouping (`extraction/grouping.py`)

- [ ] Implement `group_episodes(events: list[EventDTO]) -> list[Episode]`:
  ```python
  def group_episodes(events: list[EventDTO]) -> list[Episode]:
      # 1. Group by (project_id, user_id, session_id)
      # 2. Within each group, split on time gap > 20min
      # 3. Filter: keep episodes with >= 3 events
      # 4. Return Episode objects (without summary yet)
  ```

- [ ] Implement `summarize_episode(episode: Episode, events: list[EventDTO]) -> str`:
  ```python
  async def summarize_episode(episode, events) -> tuple[str, float]:
      # Build context from events
      # Call LLM for 80-150 token summary
      # Return (summary, importance_score)
  ```

### D2. Memory extraction (`extraction/memories.py`)

- [ ] Implement extraction prompt templates in `prompts/extraction.py`:
  ```python
  EXTRACT_MEMORIES_PROMPT = """
  Analyze this coding session and extract stable patterns.

  Session transcript:
  {context}

  Extract:
  1. user_style: coding preferences, testing habits, tool preferences
  2. project_fact: framework, database, services, key endpoints

  Return JSON array: [{type, key, content, tags, importance}]
  """
  ```

- [ ] Implement `extract_memories(episode: Episode, events: list[EventDTO]) -> list[MemoryItem]`:
  ```python
  async def extract_memories(episode, events) -> list[MemoryItem]:
      context = build_context(events)
      response = await llm.call(EXTRACT_MEMORIES_PROMPT.format(context=context))
      return parse_memory_items(response, episode)
  ```

### D3. Mem0-style update (`update/logic.py`)

- [ ] Implement MVP update (key-based):
  ```python
  async def apply_updates_v0(project_id: str, new_items: list[MemoryItem]) -> list[MemoryItem]:
      existing = await storage.query_memory_items(project_id)
      final = []
      for item in new_items:
          match = find_by_key(item.key, existing)
          if not match:
              final.append(item)  # ADD
          else:
              merged = merge_items(match, item)
              final.append(merged)  # UPDATE
      return final
  ```

- [ ] (Future) Implement LLM-based update:
  ```python
  async def apply_updates_v1(project_id: str, new_items: list[MemoryItem]) -> list[MemoryItem]:
      # For each item, if existing match found:
      # Call LLM to decide: ADD_NEW | UPDATE_EXISTING | NOOP | DELETE_AND_ADD
      ...
  ```

### D4. View generators (`views/`)

- [ ] Implement `views/user_style.py`:
  ```python
  async def build_user_style_view(
      project_id: str,
      user_id: str = None,
      mode: str = "core",
      context_budget_tokens: int = 256
  ) -> UserStyleView:
      # 1. Check staleness via view_meta
      # 2. If fresh and fits budget, return cached
      # 3. If stale:
      #    - Query project memory_items (type='user_style')
      #    - Query global user_style_items
      #    - Merge (project overrides global)
      #    - Select top N based on mode and budget
      #    - Call LLM to generate view
      #    - Cache and update view_meta
      # 4. Return view with token_estimate
  ```

- [ ] Implement `views/project_brief.py`:
  ```python
  async def build_project_brief_view(
      project_id: str,
      mode: str = "core",
      context_budget_tokens: int = 256
  ) -> ProjectBriefView:
      # Similar pattern, query type='project_fact'
      # Respect context_budget_tokens
      ...
  ```

- [ ] Implement `views/pitfalls.py`:
  ```python
  async def build_pitfalls_view(
      project_id: str,
      scope_paths: list[str] = None,
      task_description: str = None,
      context_budget_tokens: int = 256
  ) -> PitfallsView:
      # Query type='pitfall', filter by scope_paths
      # Return has_relevant_pitfalls=False if nothing relevant
      ...
  ```

### D5. Task Context (Primary Output) (`views/task_context.py`)

- [ ] Implement candidate retrieval:
  ```python
  async def retrieve_candidates(
      project_id: str,
      task_description: str,
      active_file_paths: list[str],
      preferred_types: list[str],
      limit: int = 20
  ) -> list[MemoryItem]:
      # 1. Embed task_description
      # 2. Filter memory_items by project, types, file overlap
      # 3. Score: 0.6*similarity + 0.2*importance + 0.1*recency + 0.1*path_match
      # 4. Return top candidates
  ```

- [ ] Implement task-aware summarization:
  ```python
  async def build_task_context(request: TaskContextRequest) -> TaskContext:
      # 1. Retrieve candidates
      # 2. If all scores below threshold:
      #    - Return has_relevant_memory=False, empty selected_memories
      # 3. Call LLM with Chain-of-Note prompt:
      #    - Decide which memories are relevant
      #    - Generate reason for each
      #    - Produce markdown bounded by context_budget_tokens
      # 4. Return TaskContext with token_estimate
  ```

- [ ] Implement abstention handling:
  ```python
  def build_empty_context(project_id: str, task: str) -> TaskContext:
      return TaskContext(
          has_relevant_memory=False,
          selected_memories=[],
          markdown="No relevant long-term memory found for this task."
      )
  ```

### D6. Enhanced Search (`views/search.py`)

- [ ] Implement explanatory search:
  ```python
  async def search_memory(
      project_id: str,
      query: str,
      top_k: int = 5,
      types: list[str] = None,
      scope_paths: list[str] = None
  ) -> SearchResponse:
      # 1. Filter and score memory_items
      # 2. For each result, generate reason via LLM or template
      # 3. Include source (episode_ids, file_paths)
      # 4. Return with score and recency_days
  ```

- [ ] Implement view prompts in `prompts/views.py`:
  ```python
  USER_STYLE_VIEW_PROMPT = """
  Generate a concise coding style guide from these memory items:
  {items}
  Budget: {context_budget_tokens} tokens max.

  Output JSON: {items: [...], markdown: "..."}
  """

  TASK_CONTEXT_PROMPT = """
  You are building a memory pack for a coding assistant.

  Task: {task_description}
  Active files: {active_file_paths}

  Candidate memories:
  {candidates}

  Instructions:
  - Decide which memories are truly relevant for this task
  - For each selected memory, explain briefly why it matters
  - Produce a concise markdown summary (max {context_budget_tokens} tokens)
  - If nothing is relevant, return selected_memories as empty

  Output JSON: {selected_memories: [...], markdown: "..."}
  """
  ```

---

## Track E – MCP Layer (Week 3)

**Dependencies:** Track B (daemon API), Track D (views working)

### E1. MCP server (`mcp.rs`)

- [ ] Implement MCP stdio protocol handler:
  ```rust
  struct McpServer {
      daemon_client: DaemonClient,
  }
  ```

- [ ] Implement tool definitions (5 tools):
  ```rust
  fn get_tool_definitions() -> Vec<ToolDefinition> {
      vec![
          // Primary tool: task-aware context
          ToolDefinition {
              name: "get_task_context",
              description: "Get task-aware memory context with relevance explanations",
              input_schema: json!({
                  "type": "object",
                  "properties": {
                      "project_root": {"type": "string"},
                      "user_id": {"type": "string"},
                      "task_description": {"type": "string"},
                      "active_file_paths": {"type": "array", "items": {"type": "string"}},
                      "context_budget_tokens": {"type": "integer", "default": 400},
                      "preferred_memory_types": {"type": "array", "items": {"type": "string"}}
                  },
                  "required": ["project_root", "task_description"]
              }),
          },
          ToolDefinition {
              name: "get_user_style_view",
              description: "Get user coding style preferences",
              input_schema: json!({
                  "type": "object",
                  "properties": {
                      "project_root": {"type": "string"},
                      "user_id": {"type": "string"},
                      "mode": {"type": "string", "enum": ["core", "full"], "default": "core"},
                      "context_budget_tokens": {"type": "integer", "default": 256}
                  },
                  "required": ["project_root"]
              }),
          },
          ToolDefinition {
              name: "get_project_brief_view",
              description: "Get project overview and key facts",
              input_schema: json!({
                  "type": "object",
                  "properties": {
                      "project_root": {"type": "string"},
                      "mode": {"type": "string", "enum": ["core", "full"], "default": "core"},
                      "context_budget_tokens": {"type": "integer", "default": 256}
                  },
                  "required": ["project_root"]
              }),
          },
          ToolDefinition {
              name: "get_pitfalls_view",
              description: "Get known pitfalls for specific scope",
              input_schema: json!({
                  "type": "object",
                  "properties": {
                      "project_root": {"type": "string"},
                      "scope_paths": {"type": "array", "items": {"type": "string"}},
                      "task_description": {"type": "string"},
                      "context_budget_tokens": {"type": "integer", "default": 256}
                  },
                  "required": ["project_root"]
              }),
          },
          ToolDefinition {
              name: "search_project_memory",
              description: "Search project memory with explanatory results",
              input_schema: json!({
                  "type": "object",
                  "properties": {
                      "project_root": {"type": "string"},
                      "query": {"type": "string"},
                      "top_k": {"type": "integer", "default": 5},
                      "types": {"type": "array", "items": {"type": "string"}},
                      "scope_paths": {"type": "array", "items": {"type": "string"}}
                  },
                  "required": ["project_root", "query"]
              }),
          },
      ]
  }
  ```

- [ ] Implement tool handlers:
  ```rust
  async fn handle_tool_call(name: &str, args: Value) -> Result<Value> {
      match name {
          "get_task_context" => {
              let req = TaskContextRequest::from_value(&args)?;
              let ctx = daemon_client.get_task_context(req).await?;
              Ok(serde_json::to_value(ctx)?)
          }
          "get_user_style_view" => {
              let project = args["project_root"].as_str()?;
              let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("core");
              let budget = args.get("context_budget_tokens").and_then(|v| v.as_i64()).unwrap_or(256);
              let view = daemon_client.get_user_style_view(project, mode, budget).await?;
              Ok(serde_json::to_value(view)?)
          }
          // ... other tools
      }
  }
  ```

- [ ] Implement `sqrl mcp` command:
  ```rust
  fn cmd_mcp() {
      let daemon = ensure_daemon_running()?;
      let server = McpServer::new(daemon);
      server.run_stdio();  // Blocks, handles MCP protocol
  }
  ```

### E2. Claude Code MCP config

- [ ] Create example config snippet:
  ```json
  {
    "mcpServers": {
      "ctx-memory": {
        "command": "sqrl",
        "args": ["mcp"]
      }
    }
  }
  ```

- [ ] (Optional) Implement `sqrl configure-claude`:
  - Detect `~/.claude/mcp.json`
  - Backup existing
  - Add `ctx-memory` server entry
  - Confirm with user

### E3. Update Rust HTTP client for new endpoints

- [ ] Add `get_task_context()` method:
  ```rust
  async fn get_task_context(&self, req: TaskContextRequest) -> Result<TaskContext>
  ```

- [ ] Update existing methods with mode and budget params:
  ```rust
  async fn get_user_style_view(&self, project_id: &str, mode: &str, budget: i64) -> Result<UserStyleView>
  async fn get_project_brief_view(&self, project_id: &str, mode: &str, budget: i64) -> Result<ProjectBriefView>
  ```

---

## Track F – Prompts & LLM (Ongoing)

**Dependencies:** None (can start early)
**Integrates with:** Track D

### F1. Extraction prompts

- [ ] Design `EXTRACT_USER_STYLE_PROMPT`
- [ ] Design `EXTRACT_PROJECT_FACTS_PROMPT`
- [ ] Design `EXTRACT_PITFALLS_PROMPT` (future)
- [ ] Test and iterate on prompt quality

### F2. Update prompts

- [ ] Design `UPDATE_DECISION_PROMPT`:
  ```
  Given existing memory and new observation, decide action:

  Existing: {existing_content}
  New: {new_content}

  Actions:
  - ADD_NEW: New info, keep both
  - UPDATE_EXISTING: Refine existing with new
  - NOOP: Already captured
  - DELETE_AND_ADD: Contradictory, replace

  Return: {action, merged_content}
  ```

### F3. View prompts

- [ ] Design `USER_STYLE_VIEW_PROMPT`
- [ ] Design `PROJECT_BRIEF_VIEW_PROMPT`
- [ ] Design `PITFALLS_VIEW_PROMPT`

### F4. LLM client (`llm.py`)

- [ ] Implement unified LLM client:
  ```python
  class LLMClient:
      async def call(self, prompt: str, model: str = None) -> str:
          # Route to OpenAI / Anthropic / Gemini based on config
          ...
  ```
- [ ] Implement retry with exponential backoff
- [ ] Add response parsing helpers

---

## Phase X – Hardening & Polish (Week 4+)

### Logging & Observability

- [ ] Add structured logging (Rust: `tracing`, Python: `structlog`)
- [ ] Add metrics for:
  - Events processed
  - Episodes created
  - Memory items added/updated
  - View generation latency

### Configuration

- [ ] Full `sqrl config` implementation:
  - Set/get user_id
  - Set/get API keys (OpenAI, Anthropic, Gemini)
  - Set default LLM model
  - Set daemon port

### CLI Polish

- [ ] Implement `sqrl status`:
  - Show daemon status
  - Show registered projects
  - Per project: event count, last episode, memory item counts, view staleness

### Testing

- [ ] Unit tests:
  - Rust: storage, events, config
  - Python: schemas, grouping, extraction parsing
- [ ] Integration tests:
  - Full flow: daemon → watcher → Python → views
  - MCP tool calls end-to-end

### Documentation

- [ ] User guide: installation, setup, usage
- [ ] API documentation
- [ ] Prompt engineering guide

---

## Timeline Summary

| Week | Track A | Track B | Track C | Track D | Track E |
|------|---------|---------|---------|---------|---------|
| 0 | Scaffold | Scaffold | Scaffold | - | - |
| 1 | Storage, Events, Config, CLI | Daemon start | Schemas (incl. TaskContext), Server stub | - | - |
| 2 | - | Watcher, HTTP client, Batch loop | Storage adapter | Grouping, Extraction | - |
| 3 | - | Integration | - | Update, Views, Task Context, Search | MCP layer (5 tools) |
| 4+ | - | Hardening | - | Prompt iteration, Abstention tuning | Integration |

---

## Parallel Work Assignment (3 developers)

**Developer 1 (Rust focus):**
- Phase 0: Rust scaffold
- Track A: All
- Track B: All
- Track E: All

**Developer 2 (Python focus):**
- Phase 0: Python scaffold
- Track C: All
- Track D: All
- Track F: LLM client

**Developer 3 (Full-stack / Prompts):**
- Phase 0: CI, docs
- Track F: All prompts
- Phase X: Testing, documentation
- Support on Track B/D as needed
