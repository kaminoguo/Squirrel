# CLI Analysis Report: Codex, Gemini & Claude Code Integration Surface

Analysis of OpenAI Codex CLI, Google Gemini CLI, and Anthropic Claude Code for Squirrel integration.

**Goal**: Identify stable public interfaces Squirrel can use without modifying CLI code.

**Note**: Codex and Gemini are open source (analyzed from source). Claude Code is closed source (analyzed from observable behavior and official docs).

---

## 1. Host Architecture & Extension Points

### 1.1 CLI Entry Points

#### Codex CLI (codex-rs)

**Entry**: `codex-rs/cli/src/main.rs`

Uses **clap v4** with derive macros. Structure:
- `MultitoolCli` - root parser with flattened config overrides
- Subcommands: `Exec`, `Login`, `Logout`, `Mcp`, `McpServer`, `AppServer`, `Resume`, `Sandbox`, `Apply`, `Cloud`, `Features`

**Project Root**: Current working directory (cwd). Git root discovery for project docs.

```
codex [OPTIONS] [SUBCOMMAND]
codex -c key=value      # TOML config override
codex resume <id>       # Resume session by UUID
codex mcp               # MCP management
```

#### Gemini CLI

**Entry**: `packages/cli/src/gemini.tsx`

Uses **Yargs** for argument parsing. Main flow:
1. `parseArguments()` via Yargs
2. Settings loading and migration
3. Config initialization via `loadCliConfig(cwd)`
4. Session resumption logic

**Project Root**: `process.cwd()` passed to `loadCliConfig()`.

```
gemini [OPTIONS]
gemini -p "prompt"      # Non-interactive
gemini -i              # Force interactive
```

#### Claude Code (Closed Source)

**Entry**: Not available (closed source)

**Project Root**: Current working directory. Stores project data in `~/.claude/projects/<path-encoded>/`.

```
claude [OPTIONS]
claude -p "prompt"              # Non-interactive (headless)
claude --resume <session-id>    # Resume session
claude --output-format json     # Structured output
```

### 1.2 Public Extension Points

#### Codex CLI

| Extension Point | Location | Format |
|-----------------|----------|--------|
| Global config | `~/.codex/config.toml` | TOML |
| Credentials | `~/.codex/.credentials.json` | JSON |
| History | `~/.codex/history.jsonl` | JSONL |
| Logs | `~/.codex/log/` | Various |
| MCP servers | `[mcp_servers.<name>]` in config.toml | TOML |
| Project docs | `AGENTS.md`, `AGENTS.override.md` | Markdown |
| Env vars | `CODEX_HOME`, `CODEX_API_KEY` | - |

#### Gemini CLI

| Extension Point | Location | Format |
|-----------------|----------|--------|
| User settings | `~/.gemini/settings.json` | JSON |
| System settings (Linux) | `/etc/gemini-cli/settings.json` | JSON |
| System settings (macOS) | `/Library/Application Support/GeminiCli/settings.json` | JSON |
| System settings (Windows) | `C:\ProgramData\gemini-cli\settings.json` | JSON |
| Project settings | `.gemini/settings.json` | JSON |
| Project env | `.gemini/.env` | dotenv |
| Extensions | `.gemini/extensions/` | Directory |
| Context files | `GEMINI.md` | Markdown |
| MCP servers | `mcpServers` key in settings.json | JSON |
| Sessions | `~/.gemini/tmp/<project-hash>/chats/` | JSON |
| Env vars | `GEMINI_API_KEY`, `GEMINI_MODEL`, etc. | - |

#### Claude Code

| Extension Point | Location | Format |
|-----------------|----------|--------|
| User settings | `~/.claude/settings.json` | JSON |
| Local settings | `~/.claude/settings.local.json` | JSON |
| Project settings | `.claude/settings.json` | JSON |
| Project local | `.claude/settings.local.json` | JSON (gitignored) |
| User MCP | `~/.claude/mcp.json` | JSON |
| Project MCP | `.mcp.json` | JSON |
| Sessions | `~/.claude/projects/<path-encoded>/<session-id>.jsonl` | JSONL |
| Global history | `~/.claude/history.jsonl` | JSONL |
| Context file | `CLAUDE.md`, `.claude/CLAUDE.md` | Markdown |
| Custom commands | `.claude/commands/` | Shell/Markdown |
| Subagents | `.claude/agents/` | Markdown |
| Env vars | `ANTHROPIC_API_KEY`, `ANTHROPIC_MODEL`, etc. | - |

---

## 2. Logging & State Files (Squirrel Capture Pipeline)

### 2.1 Log/State Locations

#### Codex CLI

**History File**: `~/.codex/history.jsonl`
- Global, append-only
- One JSON object per line
- All sessions in one file

**Format** (JSONL):
```json
{"session_id":"<uuid>","ts":<unix_seconds>,"text":"<message>"}
```

**Schema**:
- `session_id`: UUID v7 (time-based) string
- `ts`: Unix timestamp in seconds
- `text`: Message content

**Code Reference**: `codex-rs/core/src/message_history.rs`

#### Gemini CLI

**Session Files**: `~/.gemini/tmp/<project-hash>/chats/session-<timestamp>-<uuid>.json`
- Per-project, per-session
- Full JSON files (not JSONL)

**Format** (JSON):
```json
{
  "sessionId": "<uuid>",
  "projectHash": "<sha256>",
  "startTime": "<ISO-8601>",
  "lastUpdated": "<ISO-8601>",
  "messages": [
    {
      "id": "<uuid>",
      "timestamp": "<ISO-8601>",
      "content": "<PartListUnion>",
      "type": "user" | "gemini" | "info" | "error" | "warning",
      "toolCalls": [...],      // if type=gemini
      "thoughts": [...],       // if type=gemini
      "tokens": {...},         // if type=gemini
      "model": "..."           // if type=gemini
    }
  ]
}
```

**Code Reference**: `packages/core/src/services/chatRecordingService.ts`

#### Claude Code

**Session Files**: `~/.claude/projects/<path-encoded>/<session-id>.jsonl`
- Per-project directory (path with `/` replaced by `-`)
- Per-session JSONL files
- Also agent subfiles: `agent-<short-id>.jsonl`

**Global History**: `~/.claude/history.jsonl`
- User prompts only (display text)
- Cross-project

**Format** (Session JSONL - multiple record types):
```json
// User message
{"type":"user","message":{"role":"user","content":"..."},"sessionId":"<uuid>","cwd":"/path","timestamp":"<ISO-8601>","uuid":"<uuid>","parentUuid":"<uuid>","gitBranch":"...","version":"2.0.52"}

// File history snapshot
{"type":"file-history-snapshot","messageId":"<uuid>","snapshot":{"trackedFileBackups":{...},"timestamp":"<ISO-8601>"}}

// Summary (conversation title)
{"type":"summary","summary":"Short description","leafUuid":"<uuid>"}
```

**Schema** (User Message):
- `type`: "user" | "assistant" | "file-history-snapshot" | "summary"
- `sessionId`: UUID string
- `cwd`: Working directory at message time
- `timestamp`: ISO 8601
- `uuid`: Message ID
- `parentUuid`: Parent message for tree structure
- `gitBranch`: Git branch name
- `version`: Claude Code version
- `message.role`: "user" | "assistant"
- `message.content`: Message text (may include XML tags for commands)

### 2.2 Schema Details

#### Codex History Entry

| Field | Type | Description |
|-------|------|-------------|
| `session_id` | string | UUID v7 (time-ordered) |
| `ts` | number | Unix seconds |
| `text` | string | Raw message text |

#### Gemini MessageRecord

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | UUID v4 |
| `timestamp` | string | ISO 8601 |
| `content` | PartListUnion | Google GenAI SDK format |
| `type` | enum | user/gemini/info/error/warning |
| `toolCalls` | array | Tool invocation records (gemini only) |
| `thoughts` | array | Reasoning steps (gemini only) |
| `tokens` | object | Token usage (gemini only) |
| `model` | string | Model name (gemini only) |

#### Gemini ToolCallRecord

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique tool call ID |
| `name` | string | Tool name |
| `args` | object | Tool arguments |
| `result` | PartListUnion | Tool output |
| `status` | Status | Execution status |
| `timestamp` | string | ISO 8601 |

### 2.3 Configurability

#### Codex

| Config | Mechanism | Notes |
|--------|-----------|-------|
| `CODEX_HOME` | Env var | Override `~/.codex` |
| `history.persistence` | config.toml | `save-all` or `none` |
| `history.max_bytes` | config.toml | Max file size (TODO) |

#### Gemini

| Config | Mechanism | Notes |
|--------|-----------|-------|
| `GEMINI_CONFIG_DIR` | Env var | Override config dir |
| - | - | No toggle for session recording |

---

## 3. MCP Integration

### 3.1 MCP Server Configuration

#### Claude Code (`.mcp.json` or `~/.claude/mcp.json`)

```json
{
  "mcpServers": {
    "squirrel": {
      "type": "stdio",
      "command": "sqrl",
      "args": ["mcp"],
      "env": { "SQRL_PROJECT": "/path/to/project" }
    }
  }
}
```

Alternative transports:
```json
{
  "mcpServers": {
    "http_server": {
      "type": "http",
      "url": "https://example.com/mcp"
    },
    "sse_server": {
      "type": "sse",
      "url": "https://example.com/sse"
    }
  }
}
```

**File Precedence**:
1. `.mcp.json` (project root) - highest priority
2. `~/.claude/mcp.json` (user global)

#### Codex (`~/.codex/config.toml`)

```toml
[mcp_servers.squirrel]
command = "sqrl"
args = ["mcp"]
env = { "SQRL_PROJECT" = "/path/to/project" }
env_vars = ["HOME", "PATH"]  # Pass through from parent
cwd = "/optional/working/dir"
startup_timeout_sec = 10
tool_timeout_sec = 60
enabled_tools = ["get_task_context"]   # Allowlist
disabled_tools = ["dangerous_tool"]    # Denylist
enabled = true

# OR HTTP transport:
[mcp_servers.remote]
url = "https://example.com/mcp"
bearer_token_env_var = "MY_TOKEN"
http_headers = { "X-Custom" = "value" }
```

**Code Reference**: `codex-rs/core/src/config/types.rs:18-194`

#### Gemini (`~/.gemini/settings.json` or `.gemini/settings.json`)

```json
{
  "mcpServers": {
    "squirrel": {
      "command": "sqrl",
      "args": ["mcp"],
      "env": { "SQRL_PROJECT": "/path/to/project" },
      "cwd": "/optional/working/dir",
      "timeout": 60000,
      "trust": true,
      "description": "Squirrel memory tools",
      "includeTools": ["get_task_context"],
      "excludeTools": ["dangerous_tool"]
    }
  }
}
```

Alternative transports:
```json
{
  "mcpServers": {
    "sse_server": { "url": "https://example.com/sse" },
    "http_server": { "httpUrl": "https://example.com/http" },
    "ws_server": { "tcp": "ws://localhost:8080" }
  }
}
```

**Code Reference**: `packages/core/src/config/config.ts:176-207`

### 3.2 Transport Mechanisms

| CLI | Transports Supported |
|-----|---------------------|
| Claude Code | Stdio, HTTP, SSE |
| Codex | Stdio, StreamableHTTP |
| Gemini | Stdio, SSE, StreamableHTTP, WebSocket |

All three primarily use **Stdio** for local MCP servers:
- Spawn process with command + args
- Communicate via stdin/stdout
- JSON-RPC over newline-delimited JSON

### 3.3 Tool Schema Support

All three CLIs support standard MCP tool schemas:
- JSON Schema for parameters
- `description` field for AI context
- Tool name constraints: `^[a-zA-Z0-9_-]+$` (max 64 chars)

**Codex tool naming**: `<server_name>__<tool_name>` (delimiter: `__`)

### 3.4 Error Handling

#### Codex
- Startup timeout: 10 seconds default
- Tool timeout: 60 seconds default
- Failed servers logged, tools unavailable
- No automatic retry

#### Gemini
- Default timeout: 10 minutes
- Status states: DISCONNECTED, CONNECTING, CONNECTED, DISCONNECTING
- Discovery states: NOT_STARTED, IN_PROGRESS, COMPLETED
- Reconnection handled by client manager

#### Claude Code
- Per-tool permission prompts (user approval required)
- Startup timeout behavior not documented
- Failed servers shown in status display
- MCP server logs visible in conversation

**Recommendation for Squirrel MCP**:
- Fast startup (< 5 seconds)
- Quick tool responses (< 10 seconds for views)
- Return clear error messages in MCP error format
- Graceful degradation if memory unavailable

---

## 4. Project & Session Identification

### 4.1 Project Root

| CLI | Method |
|-----|--------|
| Claude Code | `cwd`, stored as path-encoded directory name |
| Codex | `cwd`, climbs to find `.git` for project docs |
| Gemini | `process.cwd()`, stored in `Config.targetDir` |

### 4.2 Session ID

| CLI | Generation | Format |
|-----|------------|--------|
| Claude Code | UUID v4 | Standard UUID |
| Codex | UUID v7 | Time-based, sortable |
| Gemini | `crypto.randomUUID()` | UUID v4 |

### 4.3 Project Identification

| CLI | Method |
|-----|--------|
| Claude Code | Path-encoded directory: `/home/user/project` → `home-user-project` |
| Codex | No explicit project ID; uses cwd filtering |
| Gemini | `SHA256(projectRoot)` as `projectHash` |

**Squirrel Mapping**:
```
Squirrel project_id = canonical(cwd)     # Absolute path

Session ID extraction:
- Claude Code: sessionId field from JSONL
- Codex: session_id field from JSONL
- Gemini: sessionId field from JSON
```

---

## 5. Cross-Platform Behavior

### 5.1 Platform-Specific Code

#### Codex

| Platform | Module | Mechanism |
|----------|--------|-----------|
| Linux | `linux-sandbox/` | Landlock + seccomp |
| Windows | `windows-sandbox-rs/` | Restricted tokens + ACLs |
| macOS | Core (cfg guards) | Seatbelt |

Key `cfg(target_os)` locations:
- `core/src/sandboxing/mod.rs`
- `core/src/shell.rs`
- `core/src/config/mod.rs` (managed preferences)
- `cli/src/wsl_paths.rs` (WSL path normalization)

#### Gemini

Platform-specific in `settings.ts`:
```typescript
if (platform() === 'darwin') {
  return '/Library/Application Support/GeminiCli/settings.json';
} else if (platform() === 'win32') {
  return 'C:\\ProgramData\\gemini-cli\\settings.json';
} else {
  return '/etc/gemini-cli/settings.json';
}
```

Sandbox selection:
- macOS: `sandbox-exec` preferred
- Linux/Windows: docker/podman

### 5.2 Path Considerations for Squirrel

- Use `PathBuf` / `path.join()` for cross-platform paths
- Handle `~` expansion explicitly
- Normalize paths before comparison
- Handle Windows drive letters
- WSL: Be aware of `/mnt/c/` vs `C:\` translation

---

## 6. Config & UX Patterns

### 6.1 Config Structure

#### Claude Code

Hierarchy (highest to lowest precedence):
1. CLI arguments
2. Environment variables (`ANTHROPIC_*`)
3. `.claude/settings.local.json` (project local, gitignored)
4. `.claude/settings.json` (project)
5. `~/.claude/settings.local.json` (user local)
6. `~/.claude/settings.json` (user)

**MCP Config** (separate hierarchy):
1. `.mcp.json` (project root)
2. `~/.claude/mcp.json` (user global)

#### Codex

Hierarchy (highest to lowest precedence):
1. Managed preferences (macOS device profiles)
2. `/etc/codex/managed_config.toml` (system)
3. `~/.codex/config.toml` (user)
4. CLI args (`-c key=value`)

#### Gemini

Hierarchy (highest to lowest precedence):
1. CLI arguments
2. Environment variables
3. `.gemini/settings.json` (project)
4. `~/.gemini/settings.json` (user)
5. System settings (platform-specific)

### 6.2 Naming Conventions

| CLI | Files | Env Vars |
|-----|-------|----------|
| Claude Code | `~/.claude/`, `.claude/`, `settings.json`, `CLAUDE.md` | `ANTHROPIC_*` |
| Codex | `~/.codex/`, `config.toml`, `AGENTS.md` | `CODEX_*` |
| Gemini | `~/.gemini/`, `.gemini/`, `settings.json`, `GEMINI.md` | `GEMINI_*` |

### 6.3 AI Context Documentation

| CLI | File | Purpose |
|-----|------|---------|
| Codex | `AGENTS.md` | Project instructions for AI |
| Gemini | `GEMINI.md` | Context file loaded into system prompt |
| Claude Code | `CLAUDE.md` | Same pattern |

**Squirrel should**: Create `SQUIRREL.md` in same style for consistent UX.

---

## 7. Stable vs Unstable Interfaces

### 7.1 Stable (Safe to Depend On)

#### Claude Code
- `~/.claude/` directory structure
- `~/.claude/projects/<path-encoded>/` session storage
- `.mcp.json` format for MCP servers
- `CLAUDE.md` context file pattern
- Session JSONL format (type, sessionId, timestamp, message)
- `ANTHROPIC_API_KEY` env var

#### Codex
- `~/.codex/` directory structure
- `config.toml` format and keys
- `history.jsonl` format
- `CODEX_HOME` env var
- MCP config schema in config.toml
- `AGENTS.md` filename pattern

#### Gemini
- `~/.gemini/` directory structure
- `settings.json` schema
- Session file format in `tmp/<hash>/chats/`
- `GEMINI.md` context file
- `mcpServers` config schema
- `GEMINI_API_KEY`, `GEMINI_MODEL` env vars

### 7.2 Unstable (Avoid)

#### Claude Code
- Internal tool implementation details
- Exact JSONL record schema (may add fields)
- Agent subfile naming conventions
- Permission system internals
- Hooks implementation details

#### Codex
- Internal Rust module structure
- Sandbox environment variables (`CODEX_SANDBOX_*`)
- OpenTelemetry internals
- Cloud task internals

#### Gemini
- Internal TypeScript module structure
- Extension loader internals
- Specific UI component implementations
- Internal hook system

---

## 8. Squirrel Integration Strategy

### 8.1 Input Pipeline (Watching)

**Claude Code**:
```
Watch: ~/.claude/projects/*/*.jsonl
Parse: JSONL, extract type, sessionId, timestamp, message
Filter: By path-encoded directory for project scoping
Note: Ignore agent-*.jsonl subfiles (subprocess sessions)
```

**Codex**:
```
Watch: ~/.codex/history.jsonl
Parse: JSONL, extract session_id, ts, text
Filter: By session_id for session grouping
```

**Gemini**:
```
Watch: ~/.gemini/tmp/*/chats/session-*.json
Parse: JSON, extract full ConversationRecord
Filter: By projectHash for project scoping
```

### 8.2 Output Pipeline (MCP Server)

Register Squirrel as MCP server in all three CLIs:

**Claude Code** (`.mcp.json` in project root):
```json
{
  "mcpServers": {
    "squirrel": {
      "type": "stdio",
      "command": "sqrl",
      "args": ["mcp"]
    }
  }
}
```

**Codex** (`~/.codex/config.toml`):
```toml
[mcp_servers.squirrel]
command = "sqrl"
args = ["mcp"]
startup_timeout_sec = 5
tool_timeout_sec = 30
```

**Gemini** (`~/.gemini/settings.json`):
```json
{
  "mcpServers": {
    "squirrel": {
      "command": "sqrl",
      "args": ["mcp"],
      "timeout": 30000,
      "description": "Task-aware coding memory"
    }
  }
}
```

### 8.3 Session Correlation

```
Event from Claude Code log:
  project_id = decode(path-encoded directory name)
  session_id = sessionId field from JSONL

Event from Codex log:
  project_id = canonical(cwd from sqrl daemon)
  session_id = session_id field from JSONL

Event from Gemini log:
  project_id = reverse_lookup(projectHash) OR cwd
  session_id = sessionId field from JSON
```

### 8.4 What NOT to Do

- Do NOT patch Codex, Gemini, or Claude Code source code
- Do NOT rely on internal env vars (`CODEX_SANDBOX_*`)
- Do NOT assume specific internal module structure
- Do NOT hardcode tool timeout expectations
- Do NOT assume MCP server is always available (graceful degradation)
- Do NOT depend on undocumented JSONL record types

---

## Appendix: Key File Paths

### Claude Code
```
~/.claude/
├── settings.json             # User settings
├── settings.local.json       # User local settings (gitignored)
├── mcp.json                  # Global MCP servers
├── history.jsonl             # Global prompt history
└── projects/
    └── <path-encoded>/       # Per-project data
        ├── <session-id>.jsonl  # Session transcripts
        └── agent-<id>.jsonl    # Subagent sessions

<repo>/
├── .claude/
│   ├── settings.json         # Project settings
│   ├── settings.local.json   # Project local (gitignored)
│   ├── commands/             # Custom slash commands
│   └── agents/               # Custom subagents
├── .mcp.json                 # Project MCP servers
└── CLAUDE.md                 # Context file for AI
```

### Codex
```
~/.codex/
├── config.toml          # Main config + MCP servers
├── .credentials.json    # Auth tokens
├── history.jsonl        # Session history (JSONL)
└── log/                 # Debug logs

<repo>/
├── AGENTS.md            # Project context for AI
└── AGENTS.override.md   # Local override
```

### Gemini
```
~/.gemini/
├── settings.json        # User settings + MCP servers
├── mcp-oauth-tokens.json # OAuth tokens
└── tmp/<project-hash>/
    ├── chats/
    │   └── session-*.json  # Session files
    ├── shell_history
    └── checkpoints/

<repo>/
├── .gemini/
│   ├── settings.json    # Project settings
│   ├── .env             # Project env vars
│   └── extensions/      # Installed extensions
└── GEMINI.md            # Context file for AI
```
