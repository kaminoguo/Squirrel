# Squirrel: Complete Process Walkthrough

Detailed example demonstrating the entire Squirrel data flow from installation to personalized AI context.

## Core Concept

Squirrel watches AI tool logs, groups events into **Episodes** (4-hour time windows), and sends them to a unified Python Agent for analysis:

1. **Segment by Kind** - Not all sessions are coding tasks. Identify segment type first:
   - `EXECUTION_TASK` - coding, fixing bugs, running commands
   - `PLANNING_DECISION` - architecture, design, tech choices
   - `RESEARCH_LEARNING` - learning, exploring docs
   - `DISCUSSION` - brainstorming, chat
2. **Classify Outcomes** - Only for EXECUTION_TASK: SUCCESS | FAILURE | UNCERTAIN (with evidence)
3. **Extract Memories** - Based on segment kind, not just success/failure

Episode = batch of events from same repo within 4-hour window (internal batching, not a product concept).

**The key insight:** Not every session is a "task" with success/failure. Architecture discussions, research, and chat produce valuable memories without outcomes. We segment first, then extract appropriately.

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

### Step 1.2: CLI Selection (First Run)

```bash
sqrl config
# Interactive prompt: select which CLIs you use
# → Claude Code: yes
# → Codex CLI: yes
# → Gemini CLI: no
# → Cursor: yes
```

This stores CLI selection in `~/.sqrl/config.toml`:
```toml
[agents]
claude_code = true
codex_cli = true
gemini_cli = false
cursor = true
```

### Step 1.3: Project Initialization

```bash
cd ~/projects/inventory-api
sqrl init
```

What happens:
1. First `sqrl` command auto-starts daemon (lazy start)
2. `sqrl init` triggers agent via IPC
3. Creates `.sqrl/squirrel.db` for project memories
4. Agent scans for CLI log folders containing this project
5. Agent asks: ingest historical logs? (token-limited, not time-limited)
6. For each enabled CLI (from `config.agents`):
   - Configures MCP (adds Squirrel server to CLI's MCP config)
   - Injects instruction text to agent file (CLAUDE.md, AGENTS.md, .cursor/rules/)
7. Registers project in `~/.sqrl/projects.json`

File structure after init:
```
~/.sqrl/
├── config.toml           # API keys, settings, CLI selection
├── squirrel.db           # Global memories (lesson, fact, profile with scope=global)
├── projects.json         # List of initialized projects (for sqrl sync)
└── logs/                 # Daemon logs

~/projects/inventory-api/
├── .sqrl/
│   └── squirrel.db       # Project memories (lesson, fact with scope=project)
├── CLAUDE.md             # ← Squirrel instructions injected
└── AGENTS.md             # ← Squirrel instructions injected
```

### Step 1.4: Agent Instruction Injection

For each enabled CLI, Squirrel adds this block to the agent instruction file:

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

This increases the probability that AI tools will call Squirrel MCP tools at the right moments.

### Step 1.5: Syncing New CLIs

Weeks later, Alice enables Cursor globally:

```bash
sqrl config  # select Cursor

# Update all existing projects
sqrl sync
# → Adds MCP config + instructions for Cursor to all registered projects
```

### Step 1.6: Natural Language CLI

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
sqrl sync                                 # Update all projects with new CLI configs
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

### Step 3.1: Agent Analyzes Episode (Segment-First Approach)

The unified agent receives the Episode and uses segment-first analysis:

```python
async def ingest_episode(episode: dict) -> dict:
    # Build context from events
    context = "\n".join([
        f"[{e['kind']}] {e['content']}"
        for e in episode["events"]
    ])

    # LLM analyzes: segments first, then memories in ONE call
    response = await llm.call(INGEST_PROMPT.format(context=context))

    return {
        "segments": [
            {
                "id": "seg_1",
                "kind": "EXECUTION_TASK",
                "title": "Add category endpoint",
                "event_range": [0, 4],
                "outcome": {
                    "status": "SUCCESS",
                    "evidence": ["User said 'Perfect, tests pass!'"]
                }
            },
            {
                "id": "seg_2",
                "kind": "EXECUTION_TASK",
                "title": "Fix auth loop bug",
                "event_range": [5, 10],
                "outcome": {
                    "status": "SUCCESS",
                    "evidence": ["User said 'That fixed it, thanks!'"]
                }
            }
        ],
        "memories": [
            {
                "memory_type": "fact",
                "scope": "global",
                "key": "user.preferred_style",
                "value": "async_await",
                "text": "Prefers async/await with type hints for all handlers",
                "evidence_source": "success",
                "source_segments": ["seg_1"],
                "confidence": 0.9
            },
            {
                "memory_type": "lesson",
                "outcome": "failure",
                "scope": "project",
                "text": "Auth token refresh loops are NOT caused by localStorage or cookies - check useEffect cleanup first",
                "source_segments": ["seg_2"],
                "confidence": 0.9
            },
            {
                "memory_type": "lesson",
                "outcome": "success",
                "scope": "project",
                "text": "For auth redirect loops, fix useEffect cleanup to prevent re-triggering on token refresh",
                "source_segments": ["seg_2"],
                "confidence": 0.9
            },
            {
                "memory_type": "fact",
                "scope": "project",
                "text": "Tried localStorage fix (failed), tried cookies fix (failed), useEffect cleanup fix worked",
                "evidence_source": "neutral",
                "source_segments": ["seg_2"],
                "confidence": 0.85
            }
        ]
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

Analyze this session using SEGMENT-FIRST approach:

1. SEGMENT the episode by kind (each segment = 5-20 events):
   - EXECUTION_TASK: coding, fixing, running commands (CAN have outcome)
   - PLANNING_DECISION: architecture, design (has resolution: DECIDED/OPEN)
   - RESEARCH_LEARNING: learning, exploring (has resolution: ANSWERED/PARTIAL)
   - DISCUSSION: brainstorming, chat (no outcome)

2. For EXECUTION_TASK segments ONLY, classify outcome:
   - SUCCESS: with evidence (tests passed, user confirmed, etc.)
   - FAILURE: with evidence (error persists, user says "didn't work")
   - UNCERTAIN: no clear evidence (conservative default)

   IMPORTANT: Other segment kinds NEVER have SUCCESS/FAILURE.

3. Extract memories based on segment kind:
   - EXECUTION_TASK: lesson (with outcome), fact (knowledge discovered)
   - PLANNING_DECISION: fact (decisions), lesson (rationale), profile
   - RESEARCH_LEARNING: fact (knowledge), lesson (learnings)
   - DISCUSSION: profile (preferences), lesson (insights)

Return segments[] and memories[] with source_segment references.
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
1. Prioritizes lessons with outcome=failure (what NOT to do) first
2. Includes relevant lessons (outcome=success) and facts
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

### Memory (squirrel.db)

```sql
CREATE TABLE memories (
    id TEXT PRIMARY KEY,
    project_id TEXT,                          -- NULL for global/user-scope memories
    memory_type TEXT NOT NULL,                -- lesson | fact | profile

    -- For lessons (task-level patterns/pitfalls)
    outcome TEXT,                             -- success | failure | uncertain (lesson only)

    -- For facts
    fact_type TEXT,                           -- knowledge | process (optional)
    key TEXT,                                 -- declarative key: project.db.engine, user.preferred_style
    value TEXT,                               -- declarative value: PostgreSQL, async_await
    evidence_source TEXT,                     -- success | failure | neutral | manual (fact only)
    support_count INTEGER,                    -- approx episodes that support this fact
    last_seen_at TEXT,                        -- last episode where this was seen

    -- Content
    text TEXT NOT NULL,                       -- human-readable content
    embedding BLOB,                           -- 1536-dim float32 (text-embedding-3-small)
    metadata TEXT,                            -- JSON: anchors (files, components, endpoints)

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
```

**Status values:**
- `active` - Normal, appears in retrieval
- `inactive` - Soft deleted via `sqrl forget`, hidden but recoverable
- `invalidated` - Superseded by newer fact, hidden but keeps history

**Declarative key examples:**
- `project.db.engine` → PostgreSQL, MySQL, SQLite
- `project.api.framework` → FastAPI, Express, Rails
- `user.preferred_style` → async_await, callbacks, sync
- `user.comment_style` → minimal, detailed, jsdoc

Same key + different value → deterministic invalidation of old fact.

### User Profile (structured identity)

```sql
CREATE TABLE user_profile (
    user_id TEXT PRIMARY KEY,
    name TEXT,
    role TEXT,
    experience_level TEXT,
    company TEXT,
    primary_use_case TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

---

## Summary

| Phase | What Happens |
|-------|--------------|
| Install | Universal script or package manager |
| CLI Selection | `sqrl config` - select which CLIs you use (Claude Code, Codex, etc.) |
| Init | Creates DB, ingests history, configures MCP + injects agent instructions for enabled CLIs |
| Sync | `sqrl sync` - updates all projects when new CLIs enabled |
| Learning | Daemon watches CLI logs, parses to Events |
| Batching | Groups events into Episodes (4hr OR 50 events) |
| **Segmentation** | Agent segments by kind: EXECUTION_TASK / PLANNING_DECISION / RESEARCH_LEARNING / DISCUSSION |
| **Outcome** | For EXECUTION_TASK only: SUCCESS/FAILURE/UNCERTAIN (with evidence) |
| Extraction | Based on segment kind: lesson (with outcome), fact (with key/evidence_source), profile |
| **Declarative Keys** | Facts with project.* or user.* keys enable deterministic conflict detection |
| **Contradiction** | Same key + different value → old fact invalidated (no LLM); free-text → LLM judges |
| Dedup | Near-duplicate check (0.9 similarity) before ADD |
| Retrieval | MCP → Vector search (top 20) → LLM reranks + composes context prompt |
| Forget | `sqrl forget` → soft delete (status=inactive), recoverable |
| Idle | 2hr no activity → daemon stops, next command restarts |

### Why Segment-First Matters

Not all sessions are coding tasks with success/failure:
- Architecture discussions → produce decisions (fact), not outcomes
- Research sessions → produce knowledge (fact), not outcomes
- Brainstorming → produces insights (lesson) and preferences (profile)

Segment-first ensures we extract appropriate memories from each session type, and only apply SUCCESS/FAILURE to actual execution tasks.

### Why Contradiction Detection Matters

Facts change over time:
- Day 1: "Project uses PostgreSQL" (fact)
- Day 30: "Migrated to MySQL" (new fact)

Contradiction detection auto-invalidates old facts when new conflicting facts arrive, keeping retrieval clean and accurate.

---

## Key Design Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| **Unified Agent** | Single Python agent with tools | One LLM brain for all operations |
| **2-tier LLM** | strong_model + fast_model | Pro for complex reasoning, Flash for quick tasks |
| **Lazy Daemon** | Start on command, stop after 2hr idle | No system service complexity |
| Episode trigger | 4-hour window OR 50 events | Balance context vs LLM cost |
| **Segment-first** | Segment by kind before outcome classification | Not all sessions are tasks with outcomes |
| **Segment kinds** | EXECUTION_TASK / PLANNING / RESEARCH / DISCUSSION | Different session types produce different memories |
| **Outcome only for EXECUTION_TASK** | SUCCESS/FAILURE/UNCERTAIN with evidence | Avoid classifying discussions as "failures" |
| Memory extraction | Based on segment kind | Architecture produces facts, coding produces lessons |
| **Declarative keys** | project.* and user.* keys for facts | Deterministic conflict detection (no LLM) |
| **Evidence source** | success/failure/neutral/manual on facts | Track how a fact was learned |
| **Memory lifecycle** | status (active/inactive/invalidated) + validity | Soft delete + contradiction handling |
| **Fact contradiction** | Declarative key match + LLM for free-text | Auto-invalidate old when new conflicts |
| **Soft delete only (v1)** | `sqrl forget` → status=inactive | Recoverable, no hard purge until v2 |
| **Context compose** | LLM reranks + generates prompt (fast_model) | Better than math scoring, one call |
| **Natural language CLI** | Thin shell passes to agent | "By the way" - agent handles all |
| **Retroactive ingestion** | Token-limited, not time-limited | Fair for all project sizes |
| User profile | Separate table (structured identity) | name, role, experience_level - not learned |
| **2-layer DB** | Global (squirrel.db) + Project (squirrel.db) | Scope-based separation |
| **CLI selection** | User picks CLIs in `sqrl config` | Only configure what user actually uses |
| **Agent instruction injection** | Add Squirrel block to CLAUDE.md, AGENTS.md, etc. | Increase MCP call success rate |
| **sqrl sync** | Update all projects when new CLI enabled | User stays in control, no magic patching |
| Near-duplicate threshold | 0.9 similarity | Avoid redundant memories |
| Trivial query fast-path | Return empty <20ms | No wasted LLM calls |
| **Cross-platform** | Mac, Linux, Windows from v1 | All platforms supported |
| 100% passive | No user prompts during coding | Invisible during use |

---

## Memory Type Reference

3 memory types with scope flag:

| Type | Key Fields | Description | Example |
|------|------------|-------------|---------|
| `lesson` | outcome (success/failure/uncertain) | What worked or failed | "API 500 on null user_id", "Repository pattern works well" |
| `fact` | key, value, evidence_source | Project/user knowledge | key=project.db.engine, value=PostgreSQL |
| `profile` | (structured identity) | User background info | name, role, experience_level |

### Declarative Keys

Critical facts use declarative keys for deterministic conflict detection:

**Project-scoped keys:**
- `project.db.engine` - PostgreSQL, MySQL, SQLite
- `project.api.framework` - FastAPI, Express, Rails
- `project.language.main` - Python, TypeScript, Go
- `project.auth.method` - JWT, session, OAuth

**User-scoped keys (global):**
- `user.preferred_style` - async_await, callbacks, sync
- `user.preferred_language` - Python, TypeScript, Go
- `user.comment_style` - minimal, detailed, jsdoc

### Evidence Source (Facts Only)

How a fact was learned:
- `success` - Learned from successful task (high confidence)
- `failure` - Learned from failed task (valuable pitfall)
- `neutral` - Observed in planning/research/discussion
- `manual` - User explicitly stated via CLI

### Scope Matrix

| Scope | DB File | Description |
|-------|---------|-------------|
| Global | `~/.sqrl/squirrel.db` | User preferences, profile (applies to all projects) |
| Project | `<repo>/.sqrl/squirrel.db` | Project-specific lessons and facts |
