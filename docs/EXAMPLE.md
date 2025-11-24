# Squirrel: Complete Process Walkthrough

Detailed example demonstrating the entire Squirrel data flow from user coding to personalized AI context.

## Core Concept

Instead of trying to learn from every single log line individually, Squirrel treats one Claude Code session as a coherent **Episode**, then asks the LLM: "What stable preferences and project facts can we learn from this entire session?"

One coding session -> one story -> one memory update.

---

## Scenario

Developer "Alice" working on `inventory-api` (FastAPI project).

Alice's coding preferences:
- Type hints everywhere
- pytest with fixtures
- snake_case naming
- Async/await patterns
- Detailed docstrings

---

## ID Convention

Throughout this system, `project_id` is the **absolute filesystem path**:

```
project_id = "/Users/alice/projects/inventory-api"
```

This is the canonical identifier used internally in storage, events, and inter-process communication. Any URI-style formatting (e.g., `repo://...`) is only for display/API responses, not internal logic.

---

## Phase 1: Setup (One-time)

### Step 1.1: Install and Initialize

```bash
# Install sqrl CLI (Rust binary)
brew install sqrl

# Start global daemon (one per machine)
sqrl daemon start
# Output: Daemon started on 127.0.0.1:9468
# Output: Python memory service started on 127.0.0.1:8734

# Initialize project
cd ~/projects/inventory-api
sqrl init
# Output: Registered project: /Users/alice/projects/inventory-api
# Output: Created .ctx/ directory
# Output: Configured Claude Code MCP settings
```

### Step 1.2: Resulting File Structure

```
~/.sqrl/
├── config.toml           # Alice's API keys (OpenAI, Anthropic)
├── projects.json         # {"projects": ["/Users/alice/projects/inventory-api"]}
├── daemon.json           # {"pid": 12345, "host": "127.0.0.1", "port": 9468}
└── memory-service.json   # {"pid": 23456, "host": "127.0.0.1", "port": 8734}

~/projects/inventory-api/.ctx/
├── data.db               # Empty SQLite (events, episodes, memory_items tables)
└── views/
    └── .meta.json        # Empty metadata
```

---

## Phase 2: Learning (Passive Collection)

### Step 2.1: Alice Codes with Claude Code

Alice opens Claude Code and has this conversation:

```
Alice: "Add a new endpoint to get inventory items by category"

Claude Code: "I'll create a GET endpoint for filtering inventory..."

[Claude Code writes code, Alice provides feedback]

Alice: "Use async def, add type hints, and write a pytest fixture for this"

Claude Code: [Revises code with async, adds types, creates test fixture]
```

### Step 2.2: Claude Code Writes Logs

Claude Code automatically logs to:
```
~/.claude/projects/-Users-alice-projects-inventory-api/session_abc123.jsonl
```

Each line is a JSON object:
```json
{"type":"human","timestamp":"2025-11-25T10:00:00Z","session_id":"session_abc123","message":{"content":"Add a new endpoint to get inventory items by category"}}
{"type":"assistant","timestamp":"2025-11-25T10:00:05Z","session_id":"session_abc123","message":{"content":"I'll create a GET endpoint..."},"tool_use":[{"name":"Write","input":{"file_path":"src/routes/inventory.py","content":"..."}}]}
{"type":"human","timestamp":"2025-11-25T10:01:00Z","session_id":"session_abc123","message":{"content":"Use async def, add type hints, and write a pytest fixture for this"}}
{"type":"assistant","timestamp":"2025-11-25T10:01:10Z","session_id":"session_abc123","message":{"content":"..."},"tool_use":[{"name":"Edit","input":{"file_path":"src/routes/inventory.py","old_string":"def get_by_category","new_string":"async def get_by_category"}}]}
```

### Step 2.3: Rust Daemon Watches and Parses

`watcher.rs` detects new lines in the JSONL file:

```rust
// Pseudo-code
fn handle_new_line(line: &str, project_path: &Path) {
    let raw: Value = serde_json::from_str(line)?;

    let event = Event {
        id: generate_uuid(),
        timestamp: raw["timestamp"].parse()?,
        source_tool: SourceTool::ClaudeCode,
        event_type: match raw["type"] {
            "human" => EventType::UserMessage,
            "assistant" => EventType::AssistantResponse,
        },
        content: raw["message"]["content"].to_string(),
        tool_uses: parse_tool_uses(&raw["tool_use"]),
        file_paths: extract_file_paths(&raw),
        content_hash: compute_hash(&raw),
        project_id: project_path.to_string(),
        session_id: raw["session_id"].as_str().map(String::from),  // from Claude log
        processed_at: None,  // null = not yet sent to Python
    };

    event_store.insert(event)?;  // SQLite: <repo>/.ctx/data.db
}
```

Each event is tagged with a `session_id` and `processed_at` flag so Squirrel can group sessions and track which events have already been turned into memory.

**Event struct:**
```rust
struct Event {
    id: String,
    timestamp: DateTime<Utc>,
    source_tool: SourceTool,
    event_type: EventType,
    content: String,
    tool_uses: Vec<ToolUse>,
    file_paths: Vec<String>,
    content_hash: String,
    project_id: String,                    // absolute path: "/Users/alice/projects/inventory-api"
    session_id: Option<String>,            // from Claude log, enables session-level grouping
    processed_at: Option<DateTime<Utc>>,   // null = not yet sent to Python
}
```

The `processed_at` field makes it trivial to:
- Select unsent events: `WHERE processed_at IS NULL`
- Support retries and "send only new stuff" logic

**Example event:**
```rust
Event {
    id: "evt_001",
    timestamp: "2025-11-25T10:01:00Z",
    source_tool: ClaudeCode,
    event_type: UserMessage,
    content: "Use async def, add type hints, and write a pytest fixture for this",
    tool_uses: [],
    file_paths: [],
    content_hash: "abc123...",
    project_id: "/Users/alice/projects/inventory-api",
    session_id: Some("session_abc123"),
    processed_at: None,
}
```

### Step 2.4: Event Batching and HTTP POST

After 20 events accumulate (or 30 seconds pass), Rust batches and sends to Python:

```rust
// python_client.rs
let batch = event_store.query("SELECT * FROM events WHERE processed_at IS NULL LIMIT 20")?;

let response = http_client
    .post("http://127.0.0.1:8734/process_events")
    .json(&ProcessEventsRequest {
        project_id: "/Users/alice/projects/inventory-api",
        events: batch,
    })
    .send()
    .await?;

// Mark events as processed
let now = Utc::now();
for event in &batch {
    event_store.update_processed_at(event.id, now)?;
}
```

---

## Phase 3: Memory Extraction (Python Service)

### Step 3.1: Receive Events

`server.py` receives the POST:

```python
@app.post("/process_events")
async def process_events(request: ProcessEventsRequest):
    project_id = request.project_id  # "/Users/alice/projects/inventory-api"
    events = request.events  # List of 20 Event objects

    # Step 1: Group into episodes (by session_id or time gaps)
    episodes = extractor.group_episodes(events)

    # Step 2: Extract memory from each episode
    memory_items = []
    for episode in episodes:
        items = await extractor.extract_memories(episode)
        memory_items.extend(items)

    # Step 3: Apply update logic
    final_items = await updater.apply_updates(project_id, memory_items)

    # Step 4: Save to database
    storage.save_episodes(project_id, episodes)
    storage.save_memory_items(project_id, final_items)

    return {"episodes": episodes, "memory_items": final_items}
```

### Step 3.2: Episode Grouping

`extractor.py` groups events by session_id first, then by time gaps:

```python
def group_episodes(events: list[Event]) -> list[Episode]:
    # Primary grouping: by session_id
    by_session = defaultdict(list)
    for event in events:
        key = event.session_id or "no_session"
        by_session[key].append(event)

    episodes = []
    for session_id, session_events in by_session.items():
        # Secondary grouping: split by time gaps > 20 minutes
        session_events.sort(key=lambda e: e.timestamp)
        current_episode_events = []
        last_timestamp = None

        for event in session_events:
            if last_timestamp and (event.timestamp - last_timestamp) > timedelta(minutes=20):
                episodes.append(Episode(
                    id=generate_uuid(),
                    session_id=session_id,
                    events=current_episode_events,
                    start_time=current_episode_events[0].timestamp,
                    end_time=current_episode_events[-1].timestamp,
                ))
                current_episode_events = []

            current_episode_events.append(event)
            last_timestamp = event.timestamp

        if current_episode_events:
            episodes.append(Episode(
                id=generate_uuid(),
                session_id=session_id,
                events=current_episode_events,
                start_time=current_episode_events[0].timestamp,
                end_time=current_episode_events[-1].timestamp,
            ))

    return episodes
```

Result: 20 events from `session_abc123` grouped into 1 episode (all within 20 minutes, same session).

### Step 3.3: LLM-Based Memory Extraction

`extractor.py` calls LLM to extract both user_style and project_fact patterns:

```python
async def extract_memories(episode: Episode) -> list[MemoryItem]:
    context = "\n".join([
        f"[{e.event_type}] {e.content}"
        for e in episode.events
    ])

    response = await llm_client.call(
        model="gpt-4",
        prompt=EXTRACT_MEMORIES_PROMPT.format(context=context),
    )

    extracted = parse_llm_response(response)

    # Returns both user_style and project_fact items
    return [
        # User style preferences
        MemoryItem(
            id=generate_uuid(),
            memory_type="user_style",
            key="async_preference",
            content="Prefers async/await over synchronous code patterns",
            tags=["python", "async", "patterns"],
            importance=0.8,
            source_episode_id=episode.id,
        ),
        MemoryItem(
            id=generate_uuid(),
            memory_type="user_style",
            key="type_hint_preference",
            content="Requires type hints on all function signatures",
            tags=["python", "typing", "code-style"],
            importance=0.9,
            source_episode_id=episode.id,
        ),
        MemoryItem(
            id=generate_uuid(),
            memory_type="user_style",
            key="testing_preference",
            content="Uses pytest with fixtures for test setup",
            tags=["testing", "pytest", "fixtures"],
            importance=0.7,
            source_episode_id=episode.id,
        ),
        # Project facts
        MemoryItem(
            id=generate_uuid(),
            memory_type="project_fact",
            key="framework",
            content="Project uses FastAPI as the main web framework",
            tags=["framework", "fastapi", "python"],
            importance=0.9,
            source_episode_id=episode.id,
        ),
        MemoryItem(
            id=generate_uuid(),
            memory_type="project_fact",
            key="database",
            content="Uses PostgreSQL with async SQLAlchemy ORM",
            tags=["database", "postgresql", "sqlalchemy"],
            importance=0.8,
            source_episode_id=episode.id,
        ),
    ]
```

LLM Prompt:
```
Analyze this coding session and extract stable patterns.

Session transcript:
[UserMessage] Add a new endpoint to get inventory items by category
[AssistantResponse] I'll create a GET endpoint...
[UserMessage] Use async def, add type hints, and write a pytest fixture for this
[AssistantResponse] [edited file to use async def...]

Extract both:
1. User coding style preferences (how they like code written)
2. Stable project facts (framework, DB, key patterns, etc.)

Return as JSON array of {memory_type, key, content, tags, importance}.
```

### Step 3.4: Memory Update Logic

`updater.py` deduplicates against existing memory:

```python
async def apply_updates(project_id: str, new_items: list[MemoryItem]) -> list[MemoryItem]:
    existing_items = storage.query_memories(project_id)
    final_items = []

    for new_item in new_items:
        # Find existing item with same key
        similar = find_by_key(new_item.key, existing_items)

        if not similar:
            # ADD: completely new information
            final_items.append(new_item)
            continue

        # v0 (MVP): simple key-based matching
        # - If no existing item with same key: ADD
        # - Else: UPDATE (overwrite with new content)
        #
        # v1 (future): LLM-based decision for smarter merges
        # decision = await llm_client.call(UPDATE_DECISION_PROMPT...)
        # Handle ADD/UPDATE/NOOP/DELETE based on LLM response

        # For MVP, just update if key exists
        merged = MemoryItem(
            id=similar.id,  # keep existing ID
            memory_type=new_item.memory_type,
            key=new_item.key,
            content=new_item.content,  # use new content
            tags=list(set(similar.tags + new_item.tags)),  # merge tags
            importance=max(similar.importance, new_item.importance),
            source_episode_id=new_item.source_episode_id,
            updated_at=datetime.utcnow(),
        )
        final_items.append(merged)

    return final_items
```

Example update:
- Existing: `key="testing_preference", content="Uses pytest for testing"`
- New: `key="testing_preference", content="Uses pytest with fixtures for test setup"`
- Result: content updated to new value, tags merged

---

## Phase 4: View Generation and Consumption

### Step 4.1: Alice Starts New Coding Session

Next day, Alice opens Claude Code:

```
Alice: "Add a delete endpoint for inventory items"
```

### Step 4.2: Claude Code Calls MCP Tool

Claude Code (via MCP) automatically calls:
```json
{
  "tool": "get_user_style_view",
  "arguments": {
    "project_id": "/Users/alice/projects/inventory-api"
  }
}
```

### Step 4.3: Rust MCP Server Handles Request

`mcp.rs` receives the tool call:

```rust
async fn get_user_style_view_tool(project_id: &str) -> Result<String> {
    let ctx_dir = get_ctx_dir(project_id)?;
    let cache = ViewCache::new(&ctx_dir);

    // Check if cached view is fresh
    if !cache.is_stale("user_style")? {
        return cache.read_cache("user_style");
    }

    // Cache stale, fetch from Python
    let view = python_client
        .get("/views/user_style", &[("project_id", project_id)])
        .await?;

    cache.write_cache("user_style", &view)?;

    Ok(view)
}
```

Views are small, cached summaries of the memory state. Most of the time, Claude just reads the cached view; we only regenerate when enough new activity happens, which keeps token usage and latency low.

### Step 4.4: Python Generates Fresh View

`views.py` builds the view by merging global and project-specific memories:

```python
async def build_user_style_view(project_id: str) -> str:
    # Get project-specific user_style memories
    project_memories = storage.query_memories(
        project_id=project_id,
        memory_type="user_style",
        order_by="importance DESC",
        limit=20,
    )

    # Get global user preferences from ~/.sqrl/user.db
    # These are patterns observed across ALL projects
    global_memories = storage.query_global_user_style()

    # Merge: project-specific can override global
    # This ensures Alice's global preferences (like "pytest with fixtures")
    # still apply on new projects, and project-specific overrides can refine them.
    all_memories = merge_memories(project_memories, global_memories)

    view_text = await llm_client.call(
        model="gpt-4",
        prompt=VIEW_GENERATION_PROMPT.format(
            memories=format_memories(all_memories),
        ),
    )

    return view_text
```

Generated View (returned to Claude Code):
```
## User Coding Style

**Python Patterns:**
- Always use async/await for I/O operations
- Type hints required on all function signatures and return types
- Use snake_case for functions and variables

**Testing:**
- pytest with fixtures for test setup
- Prefer fixture-based dependency injection over setup/teardown

**Code Organization:**
- Keep functions under 30 lines
- Detailed docstrings for public APIs

**This Project (inventory-api):**
- FastAPI framework
- PostgreSQL with SQLAlchemy async
- Pydantic for request/response models
```

### Step 4.5: Claude Code Uses Context

Claude Code now has personalized context:

```
Alice: "Add a delete endpoint for inventory items"

Claude Code (with Squirrel context):
"I'll add an async delete endpoint with proper type hints:

async def delete_inventory_item(
    item_id: int,
    db: AsyncSession = Depends(get_db),
) -> dict[str, str]:
    """
    Delete an inventory item by ID.

    Args:
        item_id: The unique identifier of the item to delete
        db: Database session dependency

    Returns:
        Confirmation message

    Raises:
        HTTPException: If item not found
    """
    ...

I'll also add a pytest fixture for testing this endpoint..."
```

Response matches Alice's preferences: async, type hints, docstring, pytest fixture.

---

## Phase 5: Continuous Learning Loop

```
+---------------------------------------------------------------------+
|                         CONTINUOUS LOOP                             |
+---------------------------------------------------------------------+
|                                                                     |
|   Alice codes --> Claude logs --> Rust watches --> Events stored   |
|       |                                                 |           |
|       |                                                 v           |
|       |                                         Batch to Python     |
|       |                                                 |           |
|       |                                                 v           |
|       |                                    LLM extracts patterns    |
|       |                                                 |           |
|       |                                                 v           |
|       |                                      Memory updated         |
|       |                                                 |           |
|       v                                                 v           |
|   Claude Code <--- MCP get_view <--- Python generates view         |
|       |                                                             |
|       +----------------- Better code suggestions -------------------+
|                                                                     |
+---------------------------------------------------------------------+
```

---

## Data State After Example Session

### ~/.sqrl/user.db (Global User Memory)

| key | content | importance |
|-----|---------|------------|
| async_preference | Prefers async/await patterns | 0.8 |
| type_hint_requirement | Requires type hints everywhere | 0.9 |
| testing_framework | Uses pytest with fixtures | 0.7 |

### ~/projects/inventory-api/.ctx/data.db

**events table:**

| id | timestamp | source_tool | event_type | session_id | processed_at | content_hash |
|----|-----------|-------------|------------|------------|--------------|--------------|
| evt_001 | 2025-11-25T10:00:00Z | claude_code | user_message | session_abc123 | 2025-11-25T10:15:00Z | abc123 |
| evt_002 | 2025-11-25T10:00:05Z | claude_code | assistant_response | session_abc123 | 2025-11-25T10:15:00Z | def456 |
| ... | ... | ... | ... | ... | ... | ... |

**episodes table:**

| id | session_id | start_time | end_time | event_count |
|----|------------|------------|----------|-------------|
| ep_001 | session_abc123 | 2025-11-25T10:00:00Z | 2025-11-25T10:15:00Z | 20 |

**memory_items table:**

| id | memory_type | key | content | importance |
|----|-------------|-----|---------|------------|
| mem_001 | user_style | async_preference | Prefers async/await over synchronous code | 0.8 |
| mem_002 | user_style | type_hint_preference | Requires type hints on all function signatures | 0.9 |
| mem_003 | project_fact | framework | Project uses FastAPI as the main web framework | 0.9 |
| mem_004 | project_fact | database | Uses PostgreSQL with async SQLAlchemy ORM | 0.8 |

### ~/projects/inventory-api/.ctx/views/user_style.json

```json
{
  "view": "## User Coding Style\n\n**Python Patterns:**\n- Always use async/await...",
  "generated_at": "2025-11-25T10:20:00Z",
  "memory_count": 5,
  "token_count": 180
}
```

### ~/projects/inventory-api/.ctx/views/.meta.json

```json
{
  "user_style": {
    "last_generated": "2025-11-25T10:20:00Z",
    "events_count_at_generation": 20,
    "ttl_minutes": 10
  },
  "project_brief": {
    "last_generated": "2025-11-25T10:20:00Z",
    "events_count_at_generation": 20,
    "ttl_minutes": 10
  }
}
```

---

## Summary Table

| Phase | Component | Action |
|-------|-----------|--------|
| Setup | Rust CLI | `sqrl init` registers project, starts daemon |
| Learning | Rust Watcher | Monitors Claude Code JSONL, parses to Events with session_id |
| Learning | Rust Daemon | Batches events (WHERE processed_at IS NULL), POSTs to Python |
| Extraction | Python Extractor | Groups by session_id, extracts user_style + project_fact |
| Extraction | Python Updater | Deduplicates by key (v0), LLM-based merge (v1) |
| Consumption | Rust MCP | Receives tool call, serves cached view if fresh |
| Consumption | Python Views | Merges global + project memories, generates compact summary |
| Result | Claude Code | Uses personalized context to generate better code |

---

## MVP vs Future

**MVP (v0):**
- Single event source: Claude Code JSONL
- Simple key-based memory updates (match by key, overwrite)
- Two memory types: user_style, project_fact
- Two MCP tools: get_user_style_view, get_project_brief_view
- Basic caching with TTL

**Future (v1+):**
- Multi-tool ingestion: Cursor, Codex, Gemini CLI, Git
- LLM-based update decisions (ADD/UPDATE/NOOP/DELETE with semantic matching)
- Advanced memory types: pitfalls, macro recipes
- Cloud sync, team memory sharing
- Web dashboard
