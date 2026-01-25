# Squirrel Architecture

High-level system boundaries and data flow.

## Design Principles

| Principle | Description |
|-----------|-------------|
| **Passive** | Daemon watches logs, never intercepts tool calls |
| **Distributed-first** | All extraction happens locally, no central server required |
| **LLM Autonomy** | LLM decides what to extract, not hardcoded rules |
| **Simple** | No complex evaluation loops, just use_count based ordering |
| **Doc Aware** | Daemon tracks project docs, exposes structure to AI tools |

## System Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        User's Machine                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐       │
│  │ Claude Code  │    │    Cursor    │    │  Codex CLI   │       │
│  └──────┬───────┘    └──────┬───────┘    └──────┬───────┘       │
│         │                   │                   │                │
│         └─────────┬─────────┴─────────┬─────────┘                │
│                   │                   │                          │
│                   ▼                   ▼                          │
│         ┌─────────────────┐  ┌─────────────────┐                 │
│         │   Log Files     │  │   MCP Client    │                 │
│         │ (watched)       │  │   (queries)     │                 │
│         └────────┬────────┘  └────────┬────────┘                 │
│                  │                    │                          │
│                  ▼                    ▼                          │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                    RUST DAEMON                             │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │  │
│  │  │ Log Watcher │  │ Doc Watcher │  │ Git Watcher │        │  │
│  │  │   (notify)  │  │   (notify)  │  │   (notify)  │        │  │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘        │  │
│  │         │                │                │               │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │  │
│  │  │ MCP Server  │  │ CLI Handler │  │  Dashboard  │        │  │
│  │  │   (rmcp)    │  │   (clap)    │  │   (axum)    │        │  │
│  │  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘        │  │
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
│  │  │                  User Scanner                        │  │  │
│  │  │  - Cheap model (Gemini Flash)                       │  │  │
│  │  │  - Identifies behavioral patterns                    │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                  Memory Extractor                    │  │  │
│  │  │  - Strong model (Gemini Pro)                        │  │  │
│  │  │  - Extracts user style + project memory             │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────────┐  │  │
│  │  │                  Doc Summarizer                      │  │  │
│  │  │  - Cheap model (Gemini Flash)                       │  │  │
│  │  │  - Summarizes doc content for tree display          │  │  │
│  │  └─────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────┘  │
│                              │                                   │
└──────────────────────────────┼───────────────────────────────────┘
                               │ HTTPS
                               ▼
                    ┌─────────────────────┐
                    │    LLM Providers    │
                    │  (Google/Anthropic) │
                    └─────────────────────┘
```

## Component Boundaries

### ARCH-001: Rust Daemon

**Responsibility:** Local I/O, storage, MCP server, CLI, doc awareness.

| Module | Purpose |
|--------|---------|
| Log Watcher | File system events for AI tool logs (notify) |
| Doc Watcher | File system events for project docs (notify) |
| Git Watcher | Detects `.git/` creation, auto-installs hooks |
| MCP Server | Serves memories + doc tree to AI tools (rmcp) |
| CLI Handler | `sqrl init`, `sqrl on/off`, `sqrl goaway`, `sqrl config`, `sqrl status` (clap) |
| SQLite Storage | User style + project memory + doc index storage |
| Service Manager | System service (systemd/launchd/Task Scheduler) |
| Dashboard | Web UI for configuration (axum) |

**Owns:**
- SQLite read/write
- sqlite-vec vector queries
- use_count tracking
- MCP protocol handling
- Doc index storage
- Doc debt tracking
- Git hook installation

**Never Contains:**
- LLM API calls
- Tool interception

---

### ARCH-002: Python Memory Service

**Responsibility:** All LLM operations.

| Module | Purpose | Model |
|--------|---------|-------|
| User Scanner | Identify messages with behavioral patterns | gemini-3.0-flash |
| Memory Extractor | Extract behavioral adjustments | gemini-3.0-pro |
| Doc Summarizer | Summarize doc content for tree display | gemini-3.0-flash |
| Style Syncer | Write user style to agent.md | N/A |

**Owns:**
- All LLM API calls
- Session-end extraction pipeline
- Memory extraction logic
- Doc summarization
- agent.md file writes

**Never Contains:**
- File watching
- Direct database access (via IPC only)
- MCP protocol
- Embedding generation (v1 uses use_count ordering, no semantic search)

---

### ARCH-003: Storage Layer

| Database | Location | Contains |
|----------|----------|----------|
| User Style DB | `~/.sqrl/user_style.db` | Personal development style |
| Project Memory DB | `<repo>/.sqrl/memory.db` | Project-specific memories |
| Project Config | `<repo>/.sqrl/config.yaml` | Tool settings, doc patterns |
| Docs Index | `<repo>/.sqrl/memory.db` | Doc tree with summaries (same DB) |
| Doc Debt | `<repo>/.sqrl/memory.db` | Tracked doc debt per commit (same DB) |

---

## Memory Types

### User Style (Personal)

Extracted from coding sessions, synced to agent.md files.

| Example |
|---------|
| "Commits should use minimal English" |
| "AI should maintain critical, calm attitude" |
| "Never use emoji in code or comments" |
| "Prefer async/await over callbacks" |

**Storage:** Database + synced to `~/.sqrl/personal-style.md` and agent config files.

**Team (B2B):** Team admin can promote personal styles to team-level via Dashboard.

---

### Project Memory

Project-specific knowledge, organized by category.

| Category | Description |
|----------|-------------|
| `frontend` | Frontend architecture, components, styling |
| `backend` | Backend architecture, APIs, database |
| `docs_test` | Documentation, testing conventions |
| `other` | Everything else |

**Subcategories:** Default is `main`. Users can add custom subcategories via Dashboard.

**Storage:** `<repo>/.sqrl/memory.db`

**Access:** Via MCP tool, returns all categories grouped.

---

## Data Flow

### FLOW-001: Memory Extraction (Input)

**Timing:** Session-end processing (not per-message)

```
1. User codes with AI CLI
2. CLI writes to log file
3. Daemon detects file change (notify)
4. Daemon buffers events until session boundary:
   - Time gap (>10 min idle)
5. Daemon sends session data to Memory Service
6. User Scanner (Flash model):
   - Scans user messages only (cheap)
   - Identifies messages with behavioral patterns
   - Returns indices of flagged messages
   - If no patterns found, skip to step 9
7. Memory Extractor (Pro model):
   - Only processes flagged messages + AI context
   - Asks: "What should AI do differently?"
   - Extracts memories with confidence scores
   - Only memories with confidence > 0.8 proceed
   - Compares with existing memories:
     - Duplicate: increment use_count
     - Similar: merge/update
     - New: add
8. Style Syncer:
   - Updates user style in database
   - Syncs to agent.md files
9. Daemon stores project memories in SQLite
```

**Key Design Decisions:**
- Two-stage pipeline keeps costs low (Flash filters, Pro only sees relevant slices)
- Session-end processing provides full context, reduces mid-conversation noise
- Confidence threshold (>0.8) gates quality without user review in v1
- 10-minute idle timeout balances responsiveness with context completeness

---

### FLOW-002: Memory Retrieval (Output)

```
1. User says "use Squirrel" or triggers MCP
2. AI CLI calls MCP tool: squirrel_get_memory
3. Daemon queries project memory database
4. Returns all memories grouped by category:

   ## frontend
   - memory 1
   - memory 2

   ## backend
   - memory 3

   ## docs_test
   (none)

   ## other
   - memory 4

5. Memories ordered by use_count (most used first)
```

---

### FLOW-003: Garbage Collection

```
Periodic cleanup (configurable interval):

1. Find memories with use_count = 0 AND age > threshold
2. Mark as candidates for deletion
3. On next extraction, if similar memory extracted:
   - Cancel deletion, merge instead
4. Otherwise, delete after grace period
```

---

### FLOW-004: Doc Indexing

**Timing:** On doc file change (create/modify/rename)

```
1. Daemon detects doc file change (notify)
2. Check if file matches configured patterns:
   - Extensions: md, mdc, txt, rst (configurable)
   - Include paths: specs/, docs/, .claude/, .cursor/
   - Exclude paths: node_modules/, target/, .git/
3. If match, send file content to Python service
4. Doc Summarizer (Flash model):
   - Generates 1-2 sentence summary
   - Returns summary text
5. Daemon stores in docs_index table:
   - path, summary, content_hash, last_indexed
6. MCP tool can now return updated tree
```

**Key Design Decisions:**
- Only index files matching configured patterns (avoid noise)
- Content hash detects actual changes (not just mtime)
- LLM summarizes for human+AI readability

---

### FLOW-005: Git Detection and Hook Installation

**Timing:** On `.git/` directory creation

```
1. Daemon watches project directory
2. Detects `.git/` directory created (git init)
3. Automatically installs git hooks:
   - post-commit: records doc debt
   - pre-push: warns if doc debt exists (optional block)
4. Logs: "Git detected, hooks installed"
```

**Key Design Decisions:**
- Auto-install removes friction (no manual hook setup)
- Hooks call hidden internal commands
- User can disable in config if needed

---

### FLOW-006: Doc Debt Detection

**Timing:** On git commit (via post-commit hook)

```
1. Post-commit hook calls: sqrl _internal docguard-record
2. Daemon analyzes commit diff:
   - Which code files changed?
   - Which docs should have been updated? (based on rules)
3. Detection rules (priority order):
   a. User config (.sqrl/config.yaml mappings)
   b. Reference-based (code contains SCHEMA-001 → SCHEMAS.md)
   c. Pattern-based (*.rs → ARCHITECTURE.md)
4. If code changed but related docs didn't:
   - Record doc debt entry
5. Doc debt visible via:
   - sqrl status
   - MCP tool: squirrel_get_doc_debt
   - Dashboard
```

**Key Design Decisions:**
- Deterministic detection (no LLM for debt detection)
- Config > References > Patterns (priority order)
- Debt is informational, enforcement is optional (pre-push or CI)

---

## CLI Commands

| Command | Action |
|---------|--------|
| `sqrl` | Show help |
| `sqrl init` | Initialize project silently (creates .sqrl/ with defaults) |
| `sqrl init --no-history` | Initialize without processing historical logs |
| `sqrl on` | Enable watcher daemon |
| `sqrl off` | Disable watcher daemon |
| `sqrl goaway` | Remove all Squirrel data from project |
| `sqrl config` | Open configuration in browser |
| `sqrl status` | Show project status including doc debt |

Memory and configuration management via Dashboard (web UI).

**Hidden internal commands** (called by hooks, not user-facing):
- `sqrl _internal docguard-record` - Record doc debt after commit
- `sqrl _internal docguard-check` - Check doc debt before push

---

## Dashboard (Web UI)

| Feature | Free | Team (B2B) |
|---------|------|------------|
| View/edit personal style | ✅ | ✅ |
| View/edit project memory | ✅ | ✅ |
| Memory categories management | ✅ | ✅ |
| Tool configuration (Claude Code, Cursor, etc.) | ✅ | ✅ |
| Doc patterns configuration | ✅ | ✅ |
| Doc debt view | ✅ | ✅ |
| Team style management | ❌ | ✅ |
| Team member management | ❌ | ✅ |
| Cloud sync | ❌ | ✅ |
| Activity analytics | ❌ | ✅ |

---

## Team Features (B2B)

### Data Hierarchy

```
Personal Style (local, not synced)
      ↓ override
Team Style (cloud synced, read-only for members)
      ↓ merged with
Project Memory (cloud synced, team shared)
```

### Sync Flow

```
Local daemon extract → Push to cloud → Cloud deduplicates/merges → Sync to team
```

### Roles

| Role | Permissions |
|------|-------------|
| Owner | All + billing |
| Admin | Edit team style, manage members |
| Member | Contribute project memory, view all |

---

## Technology Stack

| Category | Technology | Notes |
|----------|------------|-------|
| **Rust Daemon** | | |
| Storage | SQLite + sqlite-vec | Local-first, vector search |
| IPC | JSON-RPC 2.0 | Over Unix socket |
| MCP SDK | rmcp | Official Rust SDK |
| CLI | clap | Minimal commands |
| File Watching | notify | Cross-platform |
| **Python Service** | | |
| LLM Client | LiteLLM | Multi-provider |
| Agent Framework | PydanticAI | Structured outputs |
| **Build** | | |
| Rust | cargo-dist | Single binary |
| Python | PyInstaller | Bundled |

---

## Platform Support

| Platform | Log Locations | Socket |
|----------|---------------|--------|
| macOS | `~/Library/Application Support/*/` | `/tmp/sqrl.sock` |
| Linux | `~/.config/*/` | `/tmp/sqrl.sock` |
| Windows | `%APPDATA%\*\` | `\\.\pipe\sqrl` |

---

## Security

| Boundary | Enforcement |
|----------|-------------|
| Daemon has no network | Rust compile-time |
| LLM keys in Python only | Environment variables |
| Project isolation | Separate DB per project |
| No secrets in memories | Extractor constraint |
