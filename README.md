# Squirrel

Local-first memory system for AI coding tools. Learns from your successes AND failures, providing personalized, task-aware context via MCP.

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
│  │ (ONNX model) │  │ (similarity) │                            │
│  └──────────────┘  └──────────────┘                            │
└─────────────────────────────────────────────────────────────────┘
```

| Component | Language | Role |
|-----------|----------|------|
| **Rust Daemon** | Rust | Log watcher, SQLite + sqlite-vec, MCP server, thin CLI |
| **Python Agent** | Python | Unified agent with tools, ONNX embeddings, retrieval |

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
Agent retrieves relevant memories
        ↓
Scores by: similarity + importance + recency
        ↓
Returns within token budget + "why" explanations
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

## MCP Tools

| Tool | Purpose |
|------|---------|
| `squirrel_get_task_context` | Task-aware memory with "why" explanations |
| `squirrel_search_memory` | Semantic search across all memory |

## Memory Types

| Type | Description | Example |
|------|-------------|---------|
| `user_style` | Coding preferences | "Prefers async/await" |
| `project_fact` | Project knowledge | "Uses PostgreSQL 15" |
| `pitfall` | Known issues | "API returns 500 on null user_id" |
| `recipe` | Successful patterns | "Use repository pattern for DB" |

## Storage Layout

```
~/.sqrl/
├── config.toml                 # User settings, API keys
├── squirrel.db                 # Global SQLite (user_style, user_profile)
└── logs/                       # Daemon logs

<repo>/.sqrl/
├── squirrel.db                 # Project SQLite (project memories)
└── config.toml                 # Project overrides (optional)
```

### Database Layers

| Layer | Location | Contents |
|-------|----------|----------|
| **User** | `~/.sqrl/squirrel.db` | user_style, user_profile |
| **Project** | `<project>/.sqrl/squirrel.db` | project_fact, pitfall, recipe |

## Configuration

```toml
# ~/.sqrl/config.toml
[llm]
api_key = "sk-ant-..."
model = "claude-sonnet-4-20250514"
small_model = "claude-haiku"      # For agent operations

[daemon]
idle_timeout_hours = 2            # Stop after N hours inactive
```

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
│       ├── embeddings.py       # ONNX embeddings
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

**In:**
- Passive log watching (4 CLIs)
- Success detection (SUCCESS/FAILURE/UNCERTAIN classification)
- Unified Python agent with tools
- Natural language CLI
- MCP integration (2 tools)
- Lazy daemon (start on demand, stop after 2hr idle)
- Retroactive log ingestion on init (token-limited)
- 4 memory types + user_profile
- Near-duplicate deduplication (0.9 threshold)
- Cross-platform (Mac, Linux, Windows)
- `sqrl update` command

**v1.1:** Auto-update, LLM-based retrieval reranking, memory consolidation

**v2:** Hooks output, file injection (AGENTS.md/GEMINI.md), cloud sync, team sharing

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
