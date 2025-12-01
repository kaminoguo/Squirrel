# Squirrel

Local-first memory system for AI coding tools. Learns your coding patterns and provides personalized, task-aware context via MCP.

## What It Does

```
You code with Claude Code / Codex / Cursor / Gemini CLI
                    ↓
    Squirrel watches logs and learns (passive)
                    ↓
    AI tools call MCP → get personalized context
                    ↓
          Better code suggestions
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        RUST DAEMON                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐│
│  │Log Watch │  │ SQLite   │  │MCP Server│  │      CLI         ││
│  │(4 CLIs)  │  │sqlite-vec│  │(2 tools) │  │sqrl init/status  ││
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └──────────────────┘│
│       └─────────────┴─────────────┴─────────────────────────────│
│                            ↕ Unix socket IPC                    │
└─────────────────────────────────────────────────────────────────┘
                             ↕
┌─────────────────────────────────────────────────────────────────┐
│                      PYTHON MEMORY SERVICE                      │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │              Router Agent (Dual Mode)                      │ │
│  │  ┌─────────────────────┐  ┌─────────────────────────────┐ │ │
│  │  │   INGEST Mode       │  │      ROUTE Mode             │ │ │
│  │  │ events → memories   │  │ task → relevant memories    │ │ │
│  │  │ ADD/UPDATE/NOOP     │  │ + "why" explanations        │ │ │
│  │  └─────────────────────┘  └─────────────────────────────┘ │ │
│  └───────────────────────────────────────────────────────────┘ │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │  Embeddings  │  │  Retrieval   │  │   "Why" Generator    │  │
│  │ (ONNX model) │  │ (similarity) │  │ (heuristic templates)│  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

| Component | Language | Role |
|-----------|----------|------|
| **Rust Daemon** | Rust | Log watcher, SQLite + sqlite-vec, MCP server, CLI |
| **Memory Service** | Python | Router Agent (dual-mode), ONNX embeddings, retrieval |

## Quick Start

```bash
brew install sqrl
sqrl daemon start
cd ~/my-project && sqrl init
# Done - Squirrel now watches and learns
```

## MCP Tools

| Tool | Purpose |
|------|---------|
| `squirrel_get_task_context` | Task-aware memory with "why" explanations |
| `squirrel_search_memory` | Semantic search across all memory |

## How It Works

### Input: Passive Log Watching

```
~/.claude/projects/**/*.jsonl  ──┐
~/.codex-cli/logs/**/*.jsonl   ──┼──→ Rust Daemon ──→ Events ──→ Episodes
~/.gemini/logs/**/*.jsonl      ──┤                        ↓
~/.cursor-tutor/logs/**/*.jsonl──┘               Python INGEST mode
                                                         ↓
                                              ADD/UPDATE/NOOP memories
```

### Output: MCP Tools

```
AI calls squirrel_get_task_context("Add delete endpoint")
                    ↓
            Rust MCP Server
                    ↓ IPC
         Python ROUTE mode + Retrieval
                    ↓
      Returns relevant memories + "why" explanations
```

## Memory Types

| Type | Description | Example |
|------|-------------|---------|
| `user_style` | Coding preferences | "Prefers async/await" |
| `project_fact` | Project knowledge | "Uses PostgreSQL 15" |
| `pitfall` | Known issues | "API returns 500 on null user_id" |
| `recipe` | Common patterns | "Use repository pattern for DB" |

## Project Structure

```
Squirrel/
├── agent/                      # Rust daemon + CLI + MCP
│   └── src/
│       ├── daemon.rs           # Process management
│       ├── watcher.rs          # Multi-CLI log watching
│       ├── storage.rs          # SQLite + sqlite-vec
│       ├── ipc.rs              # Unix socket client
│       ├── mcp.rs              # MCP server (2 tools)
│       └── cli.rs              # CLI commands
│
├── memory_service/             # Python Memory Service
│   └── squirrel_memory/
│       ├── server.py           # Unix socket IPC server
│       ├── router_agent.py     # Dual-mode router (INGEST/ROUTE)
│       ├── embeddings.py       # ONNX embeddings
│       └── retrieval.py        # Similarity search + "why"
│
├── DEVELOPMENT_PLAN.md         # Implementation roadmap
├── EXAMPLE.md                  # Detailed walkthrough
└── README.md                   # This file
```

### Runtime Directories

```
~/.sqrl/
├── config.toml                 # User settings, API keys
├── squirrel.db                 # Global SQLite (user_style)
├── projects.json               # Registered repos
└── logs/                       # Daemon logs

<repo>/.sqrl/
├── squirrel.db                 # Project SQLite (project memories)
└── config.toml                 # Project overrides (optional)
```

## Development Setup

### Prerequisites

```bash
# Rust 1.83+
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python via uv
curl -LsSf https://astral.sh/uv/install.sh | sh

# SQLite (usually pre-installed)
sqlite3 --version
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

### Run

```bash
# Start daemon
cd agent && cargo run -- daemon start

# Initialize project
cd ~/my-project && sqrl init

# Configure Claude Code MCP (add to ~/.claude/mcp.json)
# "squirrel": {"command": "sqrl", "args": ["mcp"]}
```

## Configuration

```toml
# ~/.sqrl/config.toml
[user]
id = "alice"

[llm]
anthropic_api_key = "sk-ant-..."
default_model = "claude-sonnet-4-20250514"

[daemon]
socket_path = "/tmp/sqrl_router.sock"
```

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

## v1 Scope

**In:** Passive log watching (4 CLIs), 2 MCP tools, 4 memory types, dual-mode Router Agent, SQLite + sqlite-vec

**Future (v2):** Hooks output, file injection (AGENTS.md/GEMINI.md), cloud sync, team sharing

## License

AGPL-3.0
