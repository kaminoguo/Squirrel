# Squirrel

Local-first memory system for AI coding agents. Watches your sessions, extracts patterns from corrections, syncs to agent config files.

**Not a research prototype.** Production tool that makes Claude Code, Cursor, and other AI agents actually remember what you teach them.

## How It Works

```
You correct AI behavior during coding
        ↓
Squirrel watches logs passively (invisible)
        ↓
Memory pipeline extracts patterns
        ↓
Three outputs:
  1. User Style    → synced to CLAUDE.md, .cursorrules, etc.
  2. Project Memory → exposed via MCP tool
  3. Doc Awareness  → tracks doc staleness (coming soon)
```

## Memory Architecture

### User Style (Global)

Personal preferences synced to agent config files. Applies across all projects.

```markdown
<!-- START Squirrel User Style -->
- Prefer async/await over callbacks
- Never use emoji in code
- Keep commits concise
<!-- END Squirrel User Style -->
```

**Storage:** `~/.sqrl/user_style.db`

**Sync targets:** `~/.claude/CLAUDE.md`, `~/.cursor/rules/`, `~/.codex/instructions.md`

### Project Memory (Per-Project)

Project-specific knowledge accessed via MCP.

```
squirrel_get_memory → returns:

## frontend
- Uses shadcn/ui components
- State management with Zustand

## backend
- FastAPI with Pydantic models
- Use httpx for HTTP client
```

**Storage:** `<repo>/.sqrl/memory.db`

**Access:** MCP tool `squirrel_get_memory`

### Doc Awareness (Coming Soon)

Indexes project docs, detects when code changes but docs don't.

| Feature | Description |
|---------|-------------|
| Doc Tree | Summarized doc structure via MCP |
| Doc Debt | Detects stale docs after commits |
| Auto Hooks | Git hooks remind to update docs |

## Installation

```bash
# Initialize in project
cd ~/my-project
sqrl init

# Daemon runs as system service (auto-started)
```

## CLI

| Command | Description |
|---------|-------------|
| `sqrl init` | Initialize project, start daemon |
| `sqrl on` | Enable watcher |
| `sqrl off` | Disable watcher |
| `sqrl config` | Open Dashboard in browser |
| `sqrl status` | Show status |
| `sqrl goaway` | Remove Squirrel from project |

## Supported Tools

| Tool | Logs | Style Sync | MCP |
|------|------|------------|-----|
| Claude Code | ✓ | ✓ | ✓ |
| Cursor | ✓ | ✓ | ✓ |
| Codex CLI | ✓ | ✓ | - |
| Gemini CLI | ✓ | ✓ | - |

## Architecture

```
Rust Daemon          Python Memory Service
(I/O, storage, MCP)  (LLM operations)
       │                    │
       └──── IPC (Unix socket, JSON-RPC 2.0) ────┘
```

| Component | Responsibility |
|-----------|----------------|
| Rust Daemon | Log watching, SQLite, MCP server, CLI, Dashboard |
| Python Service | User Scanner, Memory Extractor, Style Syncer |

**Local-first:** All data on your machine. LLM calls only for extraction.

## Team Features (Cloud)

| Feature | Free | Team |
|---------|------|------|
| User style sync | ✓ | ✓ |
| Project memory | ✓ | ✓ |
| Doc awareness | ✓ | ✓ |
| Team shared memory | - | ✓ |
| Cross-machine sync | - | ✓ |
| Analytics | - | ✓ |

**Cloud launching soon.**

## Development

```bash
git clone https://github.com/anthropics/squirrel.git
cd squirrel
devenv shell

test-all     # Run tests
dev-daemon   # Start dev daemon
fmt          # Format code
lint         # Lint code
```

## License

AGPL-3.0
