# Squirrel

Local-first memory system for AI coding tools. Learns from your successes AND failures, providing personalized, task-aware context via MCP. Supports individual and team memory layers.

## What It Does

```
You code with Claude Code / Codex / Cursor / Gemini CLI
                    ↓
    Squirrel watches logs (100% passive, invisible)
                    ↓
    LLM analyzes: What succeeded? What failed?
                    ↓
    SUCCESS → recipe/project_fact memories
    FAILURE → pitfall memories (what NOT to do)
    ALL → process memories (what happened, exportable)
                    ↓
    AI tools call MCP → get personalized context
                    ↓
          Better code suggestions + avoid past mistakes
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        RUST DAEMON                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐│
│  │Log Watch │  │ SQLite   │  │MCP Server│  │   CLI (thin)     ││
│  │(4 CLIs)  │  │sqlite-vec│  │          │  │ passes to agent  ││
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └──────────────────┘│
│       └─────────────┴─────────────┴─────────────────────────────│
│                            ↕ Unix socket IPC                    │
└─────────────────────────────────────────────────────────────────┘
                             ↕
┌─────────────────────────────────────────────────────────────────┐
│                      PYTHON AGENT                               │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │              Squirrel Agent (single LLM brain)            │ │
│  │                                                           │ │
│  │  Tools:                                                   │ │
│  │  ├── Memory: ingest, search, get_context, forget          │ │
│  │  ├── Filesystem: find_cli_configs, read/write_file        │ │
│  │  ├── Config: init_project, get/set_mcp_config             │ │
│  │  └── DB: query/add/update_memory, get_stats               │ │
│  │                                                           │ │
│  │  Entry points (all go through same agent):                │ │
│  │  ├── IPC: daemon sends Episode → agent ingests            │ │
│  │  ├── IPC: MCP tool call → agent retrieves                 │ │
│  │  └── IPC: CLI command → agent executes                    │ │
│  └───────────────────────────────────────────────────────────┘ │
│  ┌──────────────┐  ┌──────────────┐                            │
│  │  Embeddings  │  │  Retrieval   │                            │
│  │  (API-based) │  │ (similarity) │                            │
│  └──────────────┘  └──────────────┘                            │
└─────────────────────────────────────────────────────────────────┘
```

| Component | Language | Role |
|-----------|----------|------|
| **Rust Daemon** | Rust | Log watcher, SQLite + sqlite-vec, MCP server, thin CLI |
| **Python Agent** | Python | Unified agent with tools, API embeddings, retrieval |

## Quick Start

```bash
# Install (detects OS automatically)
curl -sSL https://sqrl.dev/install.sh | sh

# Natural language - just talk to it
sqrl "setup for this project"
sqrl "what do you know about auth here"
sqrl "show my coding style"

# Or direct commands if you prefer
sqrl init
sqrl search "database patterns"
sqrl status
```

## Installation

| Platform | Method |
|----------|--------|
| **Mac** | `brew install sqrl` or install script |
| **Linux** | Install script, AUR (Arch), nixpkg (NixOS) |
| **Windows** | `winget install sqrl` or `scoop install sqrl` |

Universal install script works on all platforms:
```bash
curl -sSL https://sqrl.dev/install.sh | sh
```

## How It Works

### Daemon Lifecycle

```
First sqrl command → daemon starts automatically
        ↓
Daemon watches logs, processes events
        ↓
2 hours no activity → daemon stops (saves resources)
        ↓
Next sqrl command → daemon starts again
```

No manual daemon management. No system services. Just works.

### Project Initialization

```bash
cd ~/my-project
sqrl init
```

This:
1. Scans CLI log folders for logs mentioning this project
2. Ingests recent history (token-limited, small projects get all, large projects get recent)
3. Creates `.sqrl/squirrel.db` for project memories
4. Detects which CLIs you use and offers to configure MCP

Skip history ingestion:
```bash
sqrl init --skip-history
```

### Passive Learning (Write Path)

```
User codes with Claude Code normally
        ↓
Claude Code writes to ~/.claude/projects/**/*.jsonl
        ↓
Rust Daemon tails JSONL files → normalized Events
        ↓
Buffers events, flushes as Episode (4hr window OR 50 events)
        ↓
Python Agent analyzes Episode:
  - Segments into Tasks ("fix auth bug", "add endpoint")
  - Classifies: SUCCESS | FAILURE | UNCERTAIN
  - Extracts memories:
      SUCCESS → recipe or project_fact
      FAILURE → pitfall
      UNCERTAIN → skip
        ↓
Near-duplicate check (0.9 threshold) → store or merge
```

### Context Retrieval (Read Path)

```
Claude Code calls MCP: squirrel_get_task_context
        ↓
Vector search retrieves candidate memories (top 20)
        ↓
LLM reranks candidates + composes context prompt:
  - Selects relevant memories
  - Resolves conflicts between memories
  - Merges related memories
  - Generates structured prompt with memory IDs
        ↓
Returns ready-to-inject context prompt
        ↓
Claude Code uses context for better response
```

For trivial queries ("fix typo"), agent returns empty fast (<20ms).

## Natural Language CLI

The CLI understands natural language. Same agent that handles memory handles your commands:

```bash
sqrl "init this project but skip old logs"
sqrl "what went wrong with the postgres migration"
sqrl "forget the memory about deprecated API"
sqrl "configure gemini cli to use squirrel"
sqrl "show my profile"
sqrl "I prefer functional programming style"
```

Direct commands still work:
```bash
sqrl init --skip-history
sqrl search "postgres"
sqrl forget <memory-id>
sqrl config set llm.model claude-sonnet
```

### Team Commands

```bash
# Share individual memory to team (manual, opt-in)
sqrl share <memory-id>              # Promote to team DB
sqrl share <memory-id> --as pitfall # Share with type conversion

# Export/Import memories
sqrl export pitfall                 # Export all pitfalls as JSON
sqrl export recipe --project        # Export project recipes
sqrl import memories.json           # Import memories

# Team management (paid)
sqrl team join <team-id>            # Join a team
sqrl team create "Backend Team"     # Create team
sqrl team sync                      # Force sync with cloud
```

## MCP Tools

| Tool | Purpose |
|------|---------|
| `squirrel_get_task_context` | Task-aware memory with "why" explanations |
| `squirrel_search_memory` | Semantic search across all memory |

## Memory Types

### Individual Memories (Free)

| Type | Scope | Description | Example |
|------|-------|-------------|---------|
| `user_style` | Global | Your coding preferences | "Prefers async/await" |
| `user_profile` | Global | Your info (name, role, skills) | "Backend dev, 5yr Python" |
| `process` | Project | What happened (exportable) | "Tried X, failed, then Y worked" |
| `pitfall` | Project | Issues you encountered | "API returns 500 on null user_id" |
| `recipe` | Project | Patterns that worked for you | "Use repository pattern for DB" |
| `project_fact` | Project | Project knowledge you learned | "Uses PostgreSQL 15" |

### Team Memories (Paid - Cloud Sync)

| Type | Scope | Description | Example |
|------|-------|-------------|---------|
| `team_style` | Global | Team coding standards | "Team uses ESLint + Prettier" |
| `team_profile` | Global | Team info | "Backend team, 5 devs" |
| `team_process` | Project | Shared what-happened logs | "Sprint 12: migrated to Redis" |
| `shared_pitfall` | Project | Team-wide known issues | "Never use ORM for bulk inserts" |
| `shared_recipe` | Project | Team-approved patterns | "Use factory pattern for tests" |
| `shared_fact` | Project | Team project knowledge | "Prod DB is on AWS RDS" |

## Storage Layout

```
~/.sqrl/
├── config.toml                 # User settings, API keys
├── squirrel.db                 # Global individual (user_style, user_profile)
├── group.db                    # Global team (team_style, team_profile) - synced
└── logs/                       # Daemon logs

<repo>/.sqrl/
├── squirrel.db                 # Project individual (process, pitfall, recipe, project_fact)
├── group.db                    # Project team (shared_*, team_process) - synced
└── config.toml                 # Project overrides (optional)
```

### 3-Layer Database Architecture

| Layer | DB File | Contents | Sync |
|-------|---------|----------|------|
| **Global Individual** | `~/.sqrl/squirrel.db` | user_style, user_profile | Local only |
| **Global Team** | `~/.sqrl/group.db` | team_style, team_profile | Cloud (paid) |
| **Project Individual** | `<repo>/.sqrl/squirrel.db` | process, pitfall, recipe, project_fact | Local only |
| **Project Team** | `<repo>/.sqrl/group.db` | shared_pitfall, shared_recipe, shared_fact, team_process | Cloud (paid) |

### Team Database Options

| Mode | Location | Use Case |
|------|----------|----------|
| **Cloud** (default) | Squirrel Cloud | Teams, auto-sync, paid tier |
| **Self-hosted** | Your server | Enterprise, data sovereignty |
| **Local file** | `group.db` file | Offline, manual export/import |

## Configuration

```toml
# ~/.sqrl/config.toml
[llm]
provider = "gemini"               # gemini | openai | anthropic | ollama | ...
api_key = "..."
base_url = ""                     # Optional, for local models (Ollama, LMStudio)

# 2-tier model design
strong_model = "gemini-2.5-pro"   # Complex reasoning (episode ingestion)
fast_model = "gemini-3-flash"     # Fast tasks (context compose, CLI, dedup)

[embedding]
provider = "openai"               # openai | gemini | cohere | ...
model = "text-embedding-3-small"  # 1536-dim, $0.10/M tokens

[daemon]
idle_timeout_hours = 2            # Stop after N hours inactive

[team]                            # Paid tier
enabled = false                   # Enable team features
team_id = ""                      # Your team ID
sync_mode = "cloud"               # cloud | self-hosted | local
sync_url = ""                     # Custom sync URL (self-hosted only)
```

### LLM Usage

| Task | Model | Why |
|------|-------|-----|
| Episode Ingestion | strong_model | Multi-step reasoning over ~10K tokens |
| Context Compose | fast_model | Rerank + generate structured prompt |
| CLI Interpretation | fast_model | Parse natural language commands |
| Near-duplicate Check | fast_model | Simple comparison |

## Project Structure

```
Squirrel/
├── agent/                      # Rust daemon + CLI + MCP
│   └── src/
│       ├── daemon.rs           # Lazy start, idle shutdown
│       ├── watcher.rs          # Multi-CLI log watching
│       ├── storage.rs          # SQLite + sqlite-vec
│       ├── ipc.rs              # Unix socket client
│       ├── mcp.rs              # MCP server
│       └── cli.rs              # Thin CLI, passes to agent
│
├── memory_service/             # Python Agent
│   └── squirrel_memory/
│       ├── server.py           # Unix socket IPC server
│       ├── agent.py            # Unified agent with tools
│       ├── tools/              # Tool implementations
│       ├── embeddings.py       # API embeddings (OpenAI, etc.)
│       └── retrieval.py        # Similarity search
│
└── docs/
    ├── DEVELOPMENT_PLAN.md
    └── EXAMPLE.md
```

## Development Setup

### Prerequisites

```bash
# Rust 1.83+
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python via uv
curl -LsSf https://astral.sh/uv/install.sh | sh
```

### Build

```bash
git clone https://github.com/kaminoguo/Squirrel.git
cd Squirrel

# Rust
cd agent && cargo build && cargo test

# Python
cd ../memory_service
uv venv && uv pip install -e ".[dev]"
source .venv/bin/activate && pytest
```

## v1 Scope

**Individual Features (Free):**
- Passive log watching (4 CLIs)
- Success detection (SUCCESS/FAILURE/UNCERTAIN classification)
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

**v2:** Hooks output, file injection (AGENTS.md/GEMINI.md), team analytics, memory marketplace

## Contributing

1. Fork and clone
2. Create branch: `git checkout -b yourname/feat-description`
3. Make changes, run tests
4. Commit: `feat(scope): description`
5. Push and create PR

### Code Style

```bash
# Rust
cargo fmt && cargo clippy

# Python
ruff check --fix . && ruff format .
```

## License

AGPL-3.0
