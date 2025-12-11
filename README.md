# Squirrel

Local-first memory system for AI coding tools. Learns from your coding sessions, providing personalized, task-aware context via MCP.

## What It Does

```
You code with Claude Code / Cursor / Windsurf
                    ↓
    Squirrel watches logs (100% passive, invisible)
                    ↓
    Memory Writer analyzes episodes:
      - What worked / what failed
      - User preferences detected
      - Project invariants discovered
      - Patterns worth remembering
                    ↓
    Extracts memories (5 kinds):
      preference (user style)
      invariant (project facts)
      pattern (reusable solutions)
      guard (risk rules)
      note (lightweight ideas)
                    ↓
    AI tools call MCP → get personalized context
                    ↓
          Better suggestions + avoid past mistakes
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        RUST DAEMON                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐│
│  │Log Watch │  │ SQLite   │  │MCP Server│  │   CLI Handler    ││
│  │ (notify) │  │sqlite-vec│  │  (rmcp)  │  │     (clap)       ││
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────────┬─────────┘│
│       │             │             │                 │          │
│  ┌────┴─────┐  ┌────┴─────┐  ┌────┴─────┐                      │
│  │  Guard   │  │  Commit  │  │Retrieval │                      │
│  │Intercept │  │  Layer   │  │ Engine   │                      │
│  └──────────┘  └──────────┘  └──────────┘                      │
│                            ↕ SQLite                            │
└─────────────────────────────────────────────────────────────────┘
                             ↕ Unix socket (JSON-RPC 2.0)
┌─────────────────────────────────────────────────────────────────┐
│                   PYTHON MEMORY SERVICE                         │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │                    Memory Writer                          │ │
│  │  - Single strong-model call per episode                   │ │
│  │  - Returns ops[] (ADD/UPDATE/DEPRECATE)                   │ │
│  └───────────────────────────────────────────────────────────┘ │
│  ┌──────────────────┐  ┌──────────────────┐                    │
│  │    Retrieval     │  │    CR-Memory     │                    │
│  │  (embed_text)    │  │  (background)    │                    │
│  └──────────────────┘  └──────────────────┘                    │
│                            ↕ LiteLLM                           │
└─────────────────────────────────────────────────────────────────┘
                             ↕ HTTPS
                    ┌─────────────────────┐
                    │    LLM Providers    │
                    │ (Anthropic/OpenAI)  │
                    └─────────────────────┘
```

| Component | Language | Role |
|-----------|----------|------|
| **Rust Daemon** | Rust | Log watcher, SQLite + sqlite-vec, MCP server, CLI, guard interception |
| **Python Memory Service** | Python | Memory Writer (LLM), embeddings, CR-Memory evaluation |

### Technology Stack

| Layer | Technology | Notes |
|-------|------------|-------|
| **Storage** | SQLite + sqlite-vec | Local-first, vector search |
| **IPC** | JSON-RPC 2.0 over Unix socket | Daemon ↔ Memory Service |
| **MCP SDK** | rmcp (official Rust SDK) | modelcontextprotocol/rust-sdk |
| **Agent Framework** | PydanticAI + LiteLLM | Multi-provider LLM support |
| **Embeddings** | OpenAI text-embedding-3-small | 1536-dim, API-based |
| **Build/Release** | cargo-dist | Generates Homebrew, MSI, installers |
| **Auto-update** | axoupdater | dist's official updater |

## Quick Start

```bash
# Install (detects OS automatically)
curl -sSL https://sqrl.dev/install.sh | sh

# Initialize a project
cd ~/my-project
sqrl init

# Search memories
sqrl search "database patterns"

# Check status
sqrl status
```

## Memory Kinds

| Kind | Purpose | Example |
|------|---------|---------|
| `preference` | User style/interaction preferences | "User prefers async/await over callbacks" |
| `invariant` | Stable project facts / architectural decisions | "This project uses httpx as HTTP client" |
| `pattern` | Reusable debugging or design patterns | "SSL errors with requests → switch to httpx" |
| `guard` | Risk rules ("don't do X", "ask before Y") | "Don't retry requests with SSL errors" |
| `note` | Lightweight notes / ideas / hypotheses | "Consider migrating to FastAPI v1.0" |

### Memory Tiers

| Tier | Purpose | Behavior |
|------|---------|----------|
| `short_term` | Trial memories, may expire | Default for new memories |
| `long_term` | Validated, stable memories | Promoted after proving value |
| `emergency` | High-severity guards | Affects tool execution (blocks dangerous actions) |

### Memory Lifecycle

```
New memory created → status: provisional, tier: short_term
        ↓
CR-Memory evaluates (use_count, opportunities, regret_hits)
        ↓
If proves value → status: active, maybe tier: long_term
If never used → status: deprecated
```

## Key Concepts

### P1: Future-Impact

Memory value = regret reduction in future episodes. A memory is valuable only if it helps later - reducing repeated bugs, confusion, or wasted tokens.

### P2: AI-Primary

The Memory Writer is the decision-maker, not a form-filler. It decides what to extract, how to phrase it, what operations to perform.

### P3: Declarative

We declare objectives and constraints. The LLM + CR-Memory evaluation loop discover the policy.

## Guard Interception

Emergency guards can block dangerous tool calls without LLM latency:

```
Memory Writer creates guard:
  "Don't retry requests with SSL errors"
  guard_pattern: {tool: ["Bash"], command_contains: ["requests"], recent_errors_contains: ["SSLError"]}
        ↓
Daemon about to execute Bash command
        ↓
Guard pattern matches → block/warn/prompt user
        ↓
No LLM call needed (<5ms)
```

## Storage

```
~/.sqrl/
├── config.toml                 # User settings
├── squirrel.db                 # Global memories (preferences)
└── logs/                       # Daemon logs

<repo>/.sqrl/
├── squirrel.db                 # Project memories (invariants, patterns, guards)
└── memory_policy.toml          # Policy overrides (optional)
```

## Configuration

```toml
# ~/.sqrl/config.toml

[llm]
provider = "anthropic"          # anthropic | openai | gemini | ollama | ...

# 2-tier model design (via LiteLLM)
strong_model = "claude-sonnet-4-20250514"   # Memory Writer
fast_model = "claude-haiku"                  # Embeddings, context compose

[embedding]
provider = "openai"
model = "text-embedding-3-small"

[daemon]
idle_timeout_hours = 2
```

## MCP Tools

| Tool | Purpose |
|------|---------|
| `squirrel_get_task_context` | Get relevant memories for current task |
| `squirrel_search_memory` | Semantic search across memories |
| `squirrel_report_outcome` | Report task success/failure for CR-Memory |

## CLI Commands

```bash
sqrl init                       # Initialize project
sqrl search "query"             # Search memories
sqrl status                     # Show stats
sqrl forget <id|query>          # Delete memory
sqrl flush                      # Force episode processing
sqrl export [--kind <kind>]     # Export memories
sqrl policy                     # Show/edit memory policy
```

## Development Setup

### Option 1: Nix/devenv (Recommended)

```bash
git clone https://github.com/kaminoguo/Squirrel.git
cd Squirrel
devenv shell

# Available commands:
test-all    # Run all tests
dev-daemon  # Start daemon in dev mode
fmt         # Format all code
lint        # Lint all code
```

### Option 2: Manual Setup

```bash
# Rust 1.83+
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Python via uv
curl -LsSf https://astral.sh/uv/install.sh | sh
```

## v1 Scope

- Passive log watching (Claude Code, Cursor, Windsurf)
- Memory Writer: single strong-model LLM call per episode
- 5 memory kinds (preference, invariant, pattern, guard, note)
- 3 tiers (short_term, long_term, emergency)
- Status lifecycle: provisional → active → deprecated
- CR-Memory: background promotion/deprecation based on usage
- Guard interception: deterministic pattern matching (<5ms)
- Declarative keys for fast lookup (project.*, user.*)
- MCP integration
- Cross-platform (Mac, Linux, Windows)

**v2:** Team/cloud sync, multi-IDE support, web UI

## Contributing

See `specs/CONTRIBUTING.md` for guidelines.

1. Fork and clone
2. Create branch: `git checkout -b yourname/feat-description`
3. Make changes, run tests
4. Commit: `feat(scope): description`
5. Push and create PR

## License

AGPL-3.0
