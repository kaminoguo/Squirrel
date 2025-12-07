# Squirrel: Complete Process Walkthrough

Detailed example demonstrating the entire Squirrel data flow from installation to personalized AI context.

## Core Concept

Squirrel watches AI tool logs, groups events into **Episodes** (4-hour time windows), and sends them to a unified Python Agent for analysis:

1. **Segment Tasks** - Identify distinct user goals within the episode
2. **Classify Outcomes** - SUCCESS | FAILURE | UNCERTAIN for each task
3. **Extract Memories** - SUCCESS→recipe/project_fact, FAILURE→pitfall, ALL→process, UNCERTAIN→skip

Episode = batch of events from same repo within 4-hour window (internal batching, not a product concept).

**The key insight:** Passive learning requires knowing WHAT succeeded before extracting patterns. We don't ask users to confirm - we infer from conversation flow.

---

## Scenario

Developer "Alice" working on `inventory-api` (FastAPI project).

Alice's coding preferences:
- Type hints everywhere
- pytest with fixtures
- Async/await patterns

---

## Phase 1: Installation & Setup

### Step 1.1: Install Squirrel

```bash
# Universal install (Mac/Linux/Windows)
curl -sSL https://sqrl.dev/install.sh | sh

# Or platform-specific
brew install sqrl          # Mac
winget install sqrl        # Windows
```

### Step 1.2: First Command Starts Daemon

```bash
cd ~/projects/inventory-api
sqrl init
```

What happens:
1. First `sqrl` command auto-starts daemon (lazy start)
2. `sqrl init` triggers agent via IPC
3. Agent scans for CLI log folders containing this project
4. Agent asks: ingest historical logs? (token-limited, not time-limited)
5. Creates `.sqrl/squirrel.db` for project memories
6. Detects installed CLIs (Claude Code, Codex, Gemini CLI, Cursor)
7. Offers to configure MCP for each detected CLI

File structure after init:
```
~/.sqrl/
├── config.toml           # API keys, settings
├── squirrel.db           # Global individual (user_style, user_profile)
├── group.db              # Global team (team_style, team_profile) - synced, paid
└── logs/                 # Daemon logs

~/projects/inventory-api/.sqrl/
├── squirrel.db           # Project individual (process, pitfall, recipe, project_fact)
└── group.db              # Project team (shared_*, team_process) - synced, paid
```

### Step 1.3: Natural Language CLI

The agent handles all CLI commands:

```bash
sqrl "what do you know about auth here"
sqrl "show my coding style"
sqrl "I prefer functional programming"
sqrl "configure gemini cli to use squirrel"
sqrl "forget the memory about deprecated API"
```

Or direct commands:
```bash
sqrl search "database patterns"
sqrl status
sqrl config set llm.model claude-sonnet
```

---

## Phase 2: Learning (Passive Collection)

### Step 2.1: Alice Codes (Two Tasks in One Session)

```
# Task 1: Add endpoint (SUCCESS)
Alice: "Add a new endpoint to get inventory items by category"
Claude Code: "I'll create a GET endpoint..."
Alice: "Use async def, add type hints, and write a pytest fixture"
Claude Code: [Revises code with async, types, fixture]
Alice: "Perfect, tests pass!"

# Task 2: Fix auth bug (FAILURE then SUCCESS)
Alice: "There's an auth loop bug when tokens expire"
Claude Code: "Let me check localStorage..."
[Error persists]
Alice: "Still broken"
Claude Code: "Let me try checking cookies..."
[Error persists]
Alice: "That didn't work either"
Claude Code: "I think the issue is in useEffect cleanup..."
[Implements fix]
Alice: "That fixed it, thanks!"
```

### Step 2.2: Rust Daemon Watches Logs

Daemon watches log files from all supported CLIs:
```
~/.claude/projects/**/*.jsonl      # Claude Code
~/.codex-cli/logs/**/*.jsonl       # Codex CLI
~/.gemini/logs/**/*.jsonl          # Gemini CLI
```

Parses into normalized Events:

```rust
let event = Event {
    id: "evt_001",
    repo: "/Users/alice/projects/inventory-api",
    kind: "user",           // user | assistant | tool | system
    content: "Use async def, add type hints...",
    file_paths: vec![],
    ts: "2025-11-25T10:01:00Z",
    processed: false,
};
storage.save_event(event)?;
```

### Step 2.3: Episode Batching

Episodes flush on **4-hour time window** OR **50 events max** (whichever comes first):

```rust
fn should_flush_episode(buffer: &EventBuffer) -> bool {
    let window_hours = 4;
    let max_events = 50;

    buffer.events.len() >= max_events ||
    buffer.oldest_event_age() >= Duration::hours(window_hours)
}

// Flush triggers IPC call to Python Agent
fn flush_episode(repo: &str, events: Vec<Event>) {
    let episode = Episode {
        id: generate_uuid(),
        repo: repo.to_string(),
        start_ts: events.first().ts,
        end_ts: events.last().ts,
        events: events,
    };

    ipc_client.send(json!({
        "method": "ingest_episode",
        "params": { "episode": episode },
        "id": 1
    }));
}
```

---

## Phase 3: Memory Extraction (Python Agent)

### Step 3.1: Agent Analyzes Episode

The unified agent receives the Episode and uses its tools to analyze and store memories:

```python
async def ingest_episode(episode: dict) -> dict:
    # Build context from events
    context = "\n".join([
        f"[{e['kind']}] {e['content']}"
        for e in episode["events"]
    ])

    # LLM analyzes: tasks, outcomes, and memories in ONE call
    response = await llm.call(INGEST_PROMPT.format(context=context))

    return {
        "tasks": [
            {
                "task": "Add category endpoint",
                "outcome": "SUCCESS",
                "evidence": "User said 'Perfect, tests pass!'",
                "memories": [
                    {
                        "type": "user_style",
                        "content": "Prefers async/await with type hints for all handlers",
                        "importance": "high",
                        "repo": "global",
                    },
                    {
                        "type": "process",  # Always recorded for export
                        "content": "Added GET /items/category endpoint with async handler, type hints, pytest fixture",
                        "importance": "medium",
                        "repo": "/Users/alice/projects/inventory-api",
                    }
                ]
            },
            {
                "task": "Fix auth loop bug",
                "outcome": "SUCCESS",  # Eventually succeeded after failures
                "evidence": "User said 'That fixed it, thanks!'",
                "memories": [
                    {
                        "type": "pitfall",
                        "content": "Auth token refresh loops are NOT caused by localStorage or cookies - check useEffect cleanup first",
                        "importance": "high",
                        "repo": "/Users/alice/projects/inventory-api",
                    },
                    {
                        "type": "recipe",
                        "content": "For auth redirect loops, fix useEffect cleanup to prevent re-triggering on token refresh",
                        "importance": "high",
                        "repo": "/Users/alice/projects/inventory-api",
                    },
                    {
                        "type": "process",  # Always recorded for export
                        "content": "Tried localStorage fix (failed), tried cookies fix (failed), useEffect cleanup fix worked",
                        "importance": "medium",
                        "repo": "/Users/alice/projects/inventory-api",
                    }
                ]
            }
        ],
        "confidence": 0.9
    }
```

INGEST Prompt:
```
Analyze this coding session (~4 hours of activity):

[user] Add a new endpoint to get inventory items by category
[assistant] I'll create a GET endpoint...
[user] Use async def, add type hints, and write a pytest fixture
[assistant] [Revises code with async, types, fixture]
[user] Perfect, tests pass!
[user] There's an auth loop bug when tokens expire
[assistant] Let me check localStorage...
[user] Still broken
[assistant] Let me try checking cookies...
[user] That didn't work either
[assistant] I think the issue is in useEffect cleanup...
[user] That fixed it, thanks!

Analyze this session:
1. Identify distinct Tasks (user goals like "add endpoint", "fix bug")
2. For each Task, determine:
   - outcome: SUCCESS | FAILURE | UNCERTAIN
   - evidence: why you classified it this way (quote user if possible)
3. For SUCCESS tasks: extract recipe (reusable pattern) or project_fact memories
4. For FAILURE tasks: extract pitfall memories (what NOT to do)
5. For tasks with failed attempts before success: extract BOTH pitfall AND recipe
6. For ALL tasks: extract process memory (what happened, for export/audit)

Return only high-confidence memories. When in doubt, skip (except process - always record).
```

### Step 3.2: Near-Duplicate Check + Save Memory

Agent uses its DB tools to check for duplicates:

```python
# Before saving, check for near-duplicates
candidates = await db_tools.search_similar(
    repo=memory.repo,
    content=memory.content,
    memory_types=[memory.memory_type],
    top_k=5
)
for candidate in candidates:
    if candidate.similarity >= 0.9:
        # Near-duplicate found - merge or skip
        return await merge_or_skip(memory, candidate)

# No duplicate, save new memory
await db_tools.add_memory(memory)
```

---

## Phase 4: Context Retrieval (MCP)

### Step 4.1: MCP Tool Call

Claude Code (or other AI tool) calls Squirrel via MCP:

```json
{
  "tool": "squirrel_get_task_context",
  "arguments": {
    "project_root": "/Users/alice/projects/inventory-api",
    "task": "Add a delete endpoint for inventory items",
    "context_budget_tokens": 400
  }
}
```

### Step 4.2: Vector Search (Candidate Retrieval)

The agent receives the MCP request via IPC and retrieves candidates:

```python
async def get_task_context(project_root: str, task: str, budget: int) -> dict:
    # For trivial queries, return empty fast (<20ms)
    if is_trivial_task(task):  # "fix typo", "add comment"
        return {"context_prompt": "", "memory_ids": [], "tokens_used": 0}

    # Vector search retrieves top 20 candidates from both DBs
    candidates = await retrieval.search(
        task=task,
        project_root=project_root,
        include_global=True,  # user_style from global DB
        top_k=20
    )
    # candidates now contains ~20 memories ranked by embedding similarity
```

### Step 4.3: LLM Rerank + Compose (fast_model)

LLM reranks candidates and composes a context prompt in ONE call:

```python
    # LLM reranks + composes context prompt (uses fast_model)
    response = await llm.call(
        model=config.fast_model,  # gemini-3-flash
        prompt=COMPOSE_PROMPT.format(
            task=task,
            candidates=format_candidates(candidates),
            budget=budget
        )
    )

    return {
        "context_prompt": response.prompt,      # Ready-to-inject text
        "memory_ids": response.selected_ids,    # For tracing
        "tokens_used": count_tokens(response.prompt)
    }
```

COMPOSE_PROMPT:
```
Task: {task}

Candidate memories (ranked by similarity):
{candidates}

Select the most relevant memories for this task. Then compose a context prompt that:
1. Prioritizes pitfalls (what NOT to do) first
2. Includes relevant recipes and project_facts
3. Resolves conflicts between memories (newer wins)
4. Merges related memories to save tokens
5. Stays within {budget} tokens

Return:
- selected_ids: list of memory IDs you selected
- prompt: the composed context prompt for the AI tool
```

### Step 4.4: Response

```json
{
  "context_prompt": "## Context from Squirrel\n\n**Style Preferences:**\n- Use async/await with type hints for all handlers [mem_abc123]\n\n**Project Facts:**\n- This project uses FastAPI with Pydantic models [mem_def456]\n\n**Relevant for this task:** You're adding an HTTP endpoint, so follow the async pattern and define a Pydantic response model.",
  "memory_ids": ["mem_abc123", "mem_def456"],
  "tokens_used": 89
}
```

The AI tool injects this `context_prompt` directly into its system prompt for better responses.

---

## Phase 5: Daemon Lifecycle

### Lazy Start
```
User runs: sqrl search "auth"
        ↓
CLI checks if daemon running (Unix socket)
        ↓
Not running → CLI starts daemon in background
        ↓
Daemon starts → CLI connects → command executed
```

### Idle Shutdown
```
Daemon tracks last activity timestamp
        ↓
Every minute: check if idle > 2 hours
        ↓
If idle: flush any pending Episodes → graceful shutdown
        ↓
Next sqrl command → daemon starts again
```

No manual daemon management. No system services. Just works.

---

## Data Schema

### Event (normalized from all CLIs)

```sql
CREATE TABLE events (
    id TEXT PRIMARY KEY,
    repo TEXT NOT NULL,
    kind TEXT NOT NULL,        -- user | assistant | tool | system
    content TEXT NOT NULL,
    file_paths TEXT,           -- JSON array
    ts TEXT NOT NULL,
    processed INTEGER DEFAULT 0
);
```

### Individual Memory (squirrel.db - Free)

```sql
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
```

### Team Memory (group.db - Paid)

```sql
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
    team_id TEXT NOT NULL,
    contributed_by TEXT NOT NULL,
    source_memory_id TEXT,            -- original individual memory (if promoted)
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);
```

### User Profile (separate from memories)

```sql
CREATE TABLE user_profile (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,       -- JSON value
    updated_at TEXT NOT NULL
);
-- Keys: name, role, preferred_languages, common_frameworks, etc.
```

---

## Summary

| Phase | What Happens |
|-------|--------------|
| Install | Universal script or package manager |
| First Command | Lazy daemon start, no system service |
| Init | Agent scans logs, optional history ingestion, configures MCP |
| Learning | Daemon watches CLI logs, parses to Events |
| Batching | Groups events into Episodes (4hr OR 50 events) |
| **Success Detection** | Agent segments Tasks, classifies SUCCESS/FAILURE/UNCERTAIN |
| Extraction | SUCCESS→recipe/project_fact, FAILURE→pitfall, ALL→process, UNCERTAIN→skip |
| Dedup | Near-duplicate check (0.9 similarity) before ADD |
| Retrieval | MCP → Vector search (top 20 from both DBs) → LLM reranks + composes context prompt |
| Share (opt-in) | `sqrl share` promotes individual memory to team DB |
| Team Sync | group.db syncs with cloud (paid) or self-hosted |
| Idle | 2hr no activity → daemon stops, next command restarts |

### Why Success Detection Matters

Without success detection, we'd blindly store patterns without knowing if they worked:
- User tries 5 approaches, only #5 works
- Old approach: Store all 5 as "patterns" (4 are wrong!)
- With success detection: Store #1-4 as pitfalls, #5 as recipe

This is the core insight: passive learning REQUIRES outcome classification.

---

## Key Design Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| **Unified Agent** | Single Python agent with tools | One LLM brain for all operations |
| **2-tier LLM** | strong_model + fast_model | Pro for complex reasoning, Flash for quick tasks |
| **Lazy Daemon** | Start on command, stop after 2hr idle | No system service complexity |
| Episode trigger | 4-hour window OR 50 events | Balance context vs LLM cost |
| Success detection | LLM classifies outcomes (strong_model) | Core insight for passive learning |
| Task segmentation | LLM decides, no rules engine | Simple, semantic understanding |
| Memory extraction | Outcome-based | Learn from both success and failure |
| **Process memory** | Always recorded | Audit trail, exportable for sharing |
| **Context compose** | LLM reranks + generates prompt (fast_model) | Better than math scoring, one call |
| **Natural language CLI** | Thin shell passes to agent | "By the way" - agent handles all |
| **Retroactive ingestion** | Token-limited, not time-limited | Fair for all project sizes |
| User profile | Separate table from user_style | Structured vs unstructured |
| **3-layer DB** | Individual (squirrel.db) + Team (group.db) | Free vs Paid, local vs cloud |
| **Team sharing** | Manual opt-in via `sqrl share` | User controls what's shared |
| **Team DB location** | Cloud (default) / Self-hosted / Local | Flexibility for enterprise |
| Near-duplicate threshold | 0.9 similarity | Avoid redundant memories |
| Trivial query fast-path | Return empty <20ms | No wasted LLM calls |
| **Cross-platform** | Mac, Linux, Windows from v1 | All platforms supported |
| 100% passive | No user prompts during coding | Invisible during use |

---

## Phase 6: Team Sharing (Paid)

### Step 6.1: Alice Shares a Pitfall

Alice finds a critical pitfall that should help the team:

```bash
sqrl share mem_abc123 --as shared_pitfall
```

What happens:
1. Agent reads memory from `squirrel.db`
2. Copies to `group.db` with type `shared_pitfall`
3. Sets `contributed_by: alice`, `source_memory_id: mem_abc123`
4. Syncs `group.db` to cloud

### Step 6.2: Bob Gets Team Context

Bob joins Alice's team and works on the same project:

```bash
sqrl team join abc-team-id
```

When Bob asks Claude Code to help with auth:
1. MCP calls `squirrel_get_task_context`
2. Vector search queries BOTH:
   - Bob's `squirrel.db` (his individual memories)
   - Team's `group.db` (shared team memories including Alice's pitfall)
3. LLM composes context with both individual and team memories
4. Bob gets Alice's auth pitfall in context without ever experiencing it himself

### Step 6.3: Export for Onboarding

Team lead exports all shared recipes for new developers:

```bash
sqrl export shared_recipe --project --format json > onboarding.json
```

New developer imports:
```bash
sqrl import onboarding.json
```

---

## Memory Type Reference

| Type | Scope | DB | Sync | Description |
|------|-------|-----|------|-------------|
| `user_style` | Global | squirrel.db | Local | Your coding preferences |
| `user_profile` | Global | squirrel.db | Local | Your info (name, role) |
| `process` | Project | squirrel.db | Local | What happened (exportable) |
| `pitfall` | Project | squirrel.db | Local | Issues you encountered |
| `recipe` | Project | squirrel.db | Local | Patterns that worked |
| `project_fact` | Project | squirrel.db | Local | Project knowledge |
| `team_style` | Global | group.db | Cloud | Team coding standards |
| `team_profile` | Global | group.db | Cloud | Team info |
| `team_process` | Project | group.db | Cloud | Shared what-happened |
| `shared_pitfall` | Project | group.db | Cloud | Team-wide issues |
| `shared_recipe` | Project | group.db | Cloud | Team-approved patterns |
| `shared_fact` | Project | group.db | Cloud | Team project knowledge |
