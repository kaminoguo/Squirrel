# Squirrel: Complete Process Walkthrough

Detailed example demonstrating the entire Squirrel data flow from user coding to personalized AI context.

## Core Concept

Squirrel treats one coding session as a coherent **Episode**, then asks the Router Agent: "What stable patterns can we learn from this session?"

One coding session → one episode → one memory update decision.

---

## Scenario

Developer "Alice" working on `inventory-api` (FastAPI project).

Alice's coding preferences:
- Type hints everywhere
- pytest with fixtures
- Async/await patterns

---

## Phase 1: Setup

### Step 1.1: Install and Initialize

```bash
brew install sqrl
sqrl daemon start
# Output: Daemon started, Python memory service spawned
# Output: Listening on /tmp/sqrl_router.sock

cd ~/projects/inventory-api
sqrl init
# Output: Registered project: /Users/alice/projects/inventory-api
# Output: Created .sqrl/ directory
```

### Step 1.2: File Structure

```
~/.sqrl/
├── config.toml           # API keys
├── squirrel.db           # Global SQLite (user_style memories)
├── projects.json         # ["/Users/alice/projects/inventory-api"]
└── logs/daemon.log

~/projects/inventory-api/.sqrl/
└── squirrel.db           # Project SQLite (events, episodes, memories)
```

---

## Phase 2: Learning (Passive Collection)

### Step 2.1: Alice Codes with Claude Code

```
Alice: "Add a new endpoint to get inventory items by category"

Claude Code: "I'll create a GET endpoint..."

Alice: "Use async def, add type hints, and write a pytest fixture"

Claude Code: [Revises code with async, types, fixture]
```

### Step 2.2: Claude Code Writes Logs

Claude Code logs to `~/.claude/projects/-Users-alice-projects-inventory-api/session_abc123.jsonl`:

```json
{"type":"human","timestamp":"2025-11-25T10:00:00Z","session_id":"session_abc123","message":{"content":"Add a new endpoint..."}}
{"type":"assistant","timestamp":"2025-11-25T10:00:05Z","session_id":"session_abc123","message":{"content":"I'll create..."},"tool_use":[...]}
{"type":"human","timestamp":"2025-11-25T10:01:00Z","session_id":"session_abc123","message":{"content":"Use async def, add type hints..."}}
```

### Step 2.3: Rust Daemon Watches and Parses

`watcher.rs` detects new lines:

```rust
let event = Event {
    id: "evt_001",
    cli: "claude_code",
    repo: "/Users/alice/projects/inventory-api",
    event_type: "user_message",
    content: "Use async def, add type hints...",
    file_paths: vec![],
    timestamp: "2025-11-25T10:01:00Z",
    processed: false,
};
storage.save_event(event)?;
```

### Step 2.4: Episode Batching and IPC

After 20 events (or 30 seconds), Rust groups into episodes and sends via IPC:

```rust
// Group events into episodes (same repo + CLI + time gap < 20min)
let episodes = group_episodes(&events);

// Send to Python via Unix socket
let request = json!({
    "method": "router_agent",
    "params": {
        "mode": "ingest",
        "payload": {
            "episode": episodes[0],
            "events": events
        }
    },
    "id": 1
});
ipc_client.send(&request).await?;
```

---

## Phase 3: Memory Extraction (Python Service)

### Step 3.1: IPC Server Receives Request

`server.py` handles the Unix socket connection:

```python
async def handle_request(request: dict) -> dict:
    if request["method"] == "router_agent":
        mode = request["params"]["mode"]
        payload = request["params"]["payload"]
        return await router_agent(mode, payload)
```

### Step 3.2: Router Agent INGEST Mode

`router_agent.py` processes the episode:

```python
async def ingest_mode(payload: dict) -> dict:
    episode = payload["episode"]
    events = payload["events"]

    # Build context from events
    context = "\n".join([
        f"[{e['event_type']}] {e['content']}"
        for e in events
    ])

    # LLM decides: is there a memorable pattern?
    response = await llm.call(INGEST_PROMPT.format(context=context))

    # Returns decision
    return {
        "action": "ADD",  # or UPDATE, NOOP
        "memory": {
            "type": "user_style",
            "content": "Prefers async/await with type hints for all handlers",
            "repo": episode["repo"]
        },
        "confidence": 0.85
    }
```

INGEST Prompt:
```
Analyze this coding session. Extract stable patterns worth remembering.

Session:
[user_message] Add a new endpoint to get inventory items by category
[assistant_response] I'll create a GET endpoint...
[user_message] Use async def, add type hints, and write a pytest fixture

Decide:
- Is there a memorable pattern? (coding style, project fact, pitfall, recipe)
- Does it duplicate existing memory?

Return: {action: ADD|UPDATE|NOOP, memory: {type, content}, confidence: 0.0-1.0}
```

### Step 3.3: Rust Saves Memory

If confidence >= 0.7 and action is ADD/UPDATE:

```rust
let memory = Memory {
    id: generate_uuid(),
    content_hash: hash(&result.memory.content),
    content: result.memory.content,
    memory_type: "user_style",
    repo: episode.repo,
    embedding: embed(&result.memory.content),  // 384-dim ONNX
    confidence: result.confidence,
    created_at: now(),
    updated_at: now(),
};
storage.save_memory(memory)?;
```

---

## Phase 4: Context Retrieval

### Step 4.1: Next Day, Alice Starts New Session

```
Alice: "Add a delete endpoint for inventory items"
```

### Step 4.2: Claude Code Calls MCP Tool

```json
{
  "tool": "squirrel_get_task_context",
  "arguments": {
    "project_root": "/Users/alice/projects/inventory-api",
    "task": "Add a delete endpoint for inventory items",
    "max_tokens": 400
  }
}
```

### Step 4.3: Rust MCP Server Handles Request

`mcp.rs`:

```rust
async fn handle_get_task_context(args: Value) -> Result<Value> {
    let repo = args["project_root"].as_str()?;
    let task = args["task"].as_str()?;

    // 1. Fetch candidates via IPC
    let candidates = ipc_client.fetch_memories(FetchParams {
        repo: repo.to_string(),
        task: Some(task.to_string()),
        max_results: 20,
    }).await?;

    // 2. Call Router Agent ROUTE mode
    let result = ipc_client.router_agent("route", json!({
        "task": task,
        "candidates": candidates
    })).await?;

    Ok(result)
}
```

### Step 4.4: Python Retrieval + ROUTE Mode

`retrieval.py` fetches candidates:

```python
async def retrieve_candidates(repo: str, task: str, top_k: int = 20) -> list[Memory]:
    # Embed task
    task_embedding = embedding_model.embed(task)

    # Query sqlite-vec for similar memories
    candidates = await db.query("""
        SELECT * FROM memories
        WHERE repo = ?
        ORDER BY vec_distance(embedding, ?) ASC
        LIMIT ?
    """, [repo, task_embedding, top_k])

    return candidates
```

`router_agent.py` ROUTE mode:

```python
async def route_mode(payload: dict) -> dict:
    task = payload["task"]
    candidates = payload["candidates"]

    # LLM selects relevant memories and generates "why"
    response = await llm.call(ROUTE_PROMPT.format(
        task=task,
        candidates=format_candidates(candidates)
    ))

    return {
        "memories": [
            {
                "type": "user_style",
                "content": "Prefers async/await with type hints",
                "why": "Relevant because you're adding an HTTP endpoint"
            }
        ],
        "tokens_used": 120
    }
```

### Step 4.5: Response to Claude Code

```json
{
  "task": "Add a delete endpoint for inventory items",
  "memories": [
    {
      "type": "user_style",
      "content": "Prefers async/await with type hints for all handlers",
      "why": "Relevant because you're adding an HTTP endpoint"
    },
    {
      "type": "project_fact",
      "content": "Uses FastAPI with Pydantic models",
      "why": "Relevant because delete endpoint needs proper response model"
    }
  ],
  "tokens_used": 156
}
```

### Step 4.6: Claude Code Uses Context

```
Alice: "Add a delete endpoint for inventory items"

Claude Code (with Squirrel context):
"I'll add an async delete endpoint with type hints:

async def delete_inventory_item(
    item_id: int,
    db: AsyncSession = Depends(get_db),
) -> DeleteResponse:
    ...

I'll also add a pytest fixture for testing..."
```

---

## Phase 5: Data State

### ~/.sqrl/squirrel.db

```sql
-- Global user_style memories (cross-project)
SELECT * FROM memories WHERE memory_type = 'user_style';

| id      | content                                      | confidence |
|---------|----------------------------------------------|------------|
| mem_001 | Prefers async/await with type hints          | 0.85       |
| mem_002 | Uses pytest with fixtures                    | 0.80       |
```

### ~/projects/inventory-api/.sqrl/squirrel.db

```sql
-- Events
| id      | cli         | event_type     | timestamp            | processed |
|---------|-------------|----------------|----------------------|-----------|
| evt_001 | claude_code | user_message   | 2025-11-25T10:00:00Z | 1         |
| evt_002 | claude_code | assistant_resp | 2025-11-25T10:00:05Z | 1         |

-- Episodes
| id      | cli         | start_ts             | end_ts               | processed |
|---------|-------------|----------------------|----------------------|-----------|
| ep_001  | claude_code | 2025-11-25T10:00:00Z | 2025-11-25T10:15:00Z | 1         |

-- Project memories
| id      | memory_type  | content                           | confidence |
|---------|--------------|-----------------------------------|------------|
| mem_003 | project_fact | Uses FastAPI with Pydantic models | 0.90       |
| mem_004 | project_fact | PostgreSQL with async SQLAlchemy  | 0.85       |
```

---

## Summary

| Phase | Component | Action |
|-------|-----------|--------|
| Setup | CLI | `sqrl init` registers project |
| Learning | Rust Watcher | Monitors 4 CLI log paths, parses to Events |
| Learning | Rust Daemon | Groups events into Episodes, sends via IPC |
| Extraction | Python INGEST | Router Agent decides ADD/UPDATE/NOOP |
| Retrieval | Rust MCP | Receives tool call, forwards to Python |
| Retrieval | Python ROUTE | Selects relevant memories, generates "why" |
| Result | Claude Code | Uses personalized context |

---

## IPC Protocol Reference

```
Transport: Unix socket /tmp/sqrl_router.sock

Request:
{
  "method": "router_agent" | "fetch_memories",
  "params": { ... },
  "id": 1
}

Response:
{
  "result": { ... },
  "id": 1
}
```

### router_agent (INGEST)
```json
{
  "method": "router_agent",
  "params": {
    "mode": "ingest",
    "payload": {"episode": {...}, "events": [...]}
  }
}
// Returns: {action, memory, confidence}
```

### router_agent (ROUTE)
```json
{
  "method": "router_agent",
  "params": {
    "mode": "route",
    "payload": {"task": "...", "candidates": [...]}
  }
}
// Returns: {memories: [{type, content, why}], tokens_used}
```

### fetch_memories
```json
{
  "method": "fetch_memories",
  "params": {
    "repo": "/path/to/repo",
    "task": "optional task for similarity",
    "memory_types": ["user_style", "project_fact"],
    "max_results": 20
  }
}
// Returns: [Memory, Memory, ...]
```
