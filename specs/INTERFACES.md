# Squirrel Interfaces

All IPC, MCP, and CLI contracts with stable IDs.

## IPC Protocol

Unix socket at `/tmp/sqrl_agent.sock`. JSON-RPC 2.0 format.

### Request Format

```json
{
    "jsonrpc": "2.0",
    "method": "<method_name>",
    "params": { ... },
    "id": <integer>
}
```

### Response Format

```json
{
    "jsonrpc": "2.0",
    "result": { ... },
    "id": <integer>
}
```

### Error Format

```json
{
    "jsonrpc": "2.0",
    "error": {
        "code": <integer>,
        "message": "<string>",
        "data": { ... }
    },
    "id": <integer>
}
```

---

## IPC Methods

### IPC-001: ingest_episode

Daemon → Memory Service. Process episode with Memory Writer, return ops array.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "ingest_episode",
  "params": {
    "episode_id": "ep-2024-12-10-001",
    "project_id": "payment-api",
    "scope": "project",
    "owner_type": "user",
    "owner_id": "alice",
    "episode_summary": {
      "events": [
        {
          "ts": "2024-12-10T10:00:00Z",
          "role": "user",
          "kind": "message",
          "summary": "asked to call Stripe API"
        },
        {
          "ts": "2024-12-10T10:01:00Z",
          "role": "assistant",
          "kind": "tool_call",
          "tool_name": "Bash",
          "summary": "ran python script with requests library"
        },
        {
          "ts": "2024-12-10T10:01:30Z",
          "role": "tool",
          "kind": "tool_result",
          "summary": "SSLError: certificate verify failed"
        }
      ],
      "stats": {
        "error_count": 3,
        "retry_loops": 2,
        "tests_final_status": null,
        "user_frustration": "severe"
      }
    },
    "recent_memories": [
      {
        "id": "mem-123",
        "kind": "invariant",
        "tier": "long_term",
        "key": "project.http.client",
        "text": "The standard HTTP client is httpx."
      }
    ],
    "policy_hints": {
      "max_memories_per_episode": 3
    }
  },
  "id": 1
}
```

**Input Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| episode_id | string | Yes | Episode UUID |
| project_id | string | Yes | Repo/project identifier |
| scope | string | Yes | `global`, `project`, or `repo_path` |
| owner_type | string | Yes | `user`, `team`, or `org` |
| owner_id | string | Yes | User/team/org identifier |
| episode_summary | object | Yes | Compressed events_json (see SCHEMA-004) |
| recent_memories | array | No | Relevant existing memories for context |
| policy_hints | object | No | Policy constraints (max_memories_per_episode) |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "ops": [
      {
        "op": "ADD",
        "scope": "project",
        "owner_type": "user",
        "owner_id": "alice",
        "kind": "guard",
        "tier": "emergency",
        "polarity": -1,
        "key": null,
        "text": "Do not keep retrying requests with SSL in this project; switch to httpx after the first SSL error.",
        "ttl_days": 3,
        "confidence": 0.8
      },
      {
        "op": "ADD",
        "scope": "project",
        "owner_type": "user",
        "owner_id": "alice",
        "kind": "pattern",
        "tier": "short_term",
        "polarity": -1,
        "key": null,
        "text": "In this project, requests often hits SSL certificate errors; use httpx as the default HTTP client instead.",
        "ttl_days": 30,
        "confidence": 0.85
      },
      {
        "op": "UPDATE",
        "target_memory_id": "mem-123",
        "scope": "project",
        "owner_type": "user",
        "owner_id": "alice",
        "kind": "invariant",
        "tier": "long_term",
        "polarity": 1,
        "key": "project.http.client",
        "text": "The standard HTTP client is httpx. Do not use requests due to SSL issues.",
        "ttl_days": null,
        "confidence": 0.9
      }
    ],
    "episode_evidence": {
      "episode_id": "ep-2024-12-10-001",
      "source": "failure_then_success",
      "frustration": "severe"
    }
  },
  "id": 1
}
```

**Output Fields:**

| Field | Type | Description |
|-------|------|-------------|
| ops | array | Array of memory operations |
| ops[].op | string | `ADD`, `UPDATE`, `DEPRECATE`, or `IGNORE` |
| ops[].target_memory_id | string | For UPDATE/DEPRECATE: ID of memory to supersede |
| ops[].scope | string | `global`, `project`, or `repo_path` |
| ops[].owner_type | string | `user`, `team`, or `org` |
| ops[].owner_id | string | Owner identifier |
| ops[].kind | string | `preference`, `invariant`, `pattern`, `guard`, `note` |
| ops[].tier | string | `short_term`, `long_term`, `emergency` |
| ops[].polarity | integer | `+1` (recommend) or `-1` (avoid/anti-pattern) |
| ops[].key | string | Optional declarative key |
| ops[].text | string | Human-readable memory content |
| ops[].ttl_days | integer | Days until expiry (null = no expiry) |
| ops[].confidence | float | 0.0-1.0 confidence score |
| episode_evidence | object | Episode-level signals |
| episode_evidence.episode_id | string | Source episode |
| episode_evidence.source | string | How memory was derived |
| episode_evidence.frustration | string | Episode frustration level |

**Operation Semantics:**

| Op | Action |
|----|--------|
| ADD | Insert new memory with status='provisional' |
| UPDATE | Mark target_memory_id as deprecated, insert new memory |
| DEPRECATE | Mark target_memory_id as status='deprecated' |
| IGNORE | No action (duplicate or not worth storing) |

**Errors:**

| Code | Message | When |
|------|---------|------|
| -32001 | Episode empty | No events in episode |
| -32002 | Invalid project | Project path doesn't exist |
| -32003 | LLM error | Memory Writer call failed |

**Model Tier:** strong_model

---

### IPC-002: embed_text

Daemon → Memory Service. Generate embedding vector for text.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "embed_text",
  "params": {
    "text": "add a webhook endpoint that calls our partner API"
  },
  "id": 2
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "embedding": [0.123, -0.456, 0.789, ...]
  },
  "id": 2
}
```

**Input Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| text | string | Yes | Text to embed |

**Output Fields:**

| Field | Type | Description |
|-------|------|-------------|
| embedding | array | 1536-dim float32 vector |

**Errors:**

| Code | Message | When |
|------|---------|------|
| -32040 | Empty text | Text string empty |
| -32041 | Embedding error | Embedding API failed |

**Model Tier:** embedding model (text-embedding-3-small default)

---

### IPC-003: compose_context

Daemon → Memory Service. (Optional) LLM-based context composition.

**Note:** v1 uses template-based composition. This method is optional.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "compose_context",
  "params": {
    "task": "add a webhook endpoint that calls our partner API",
    "memories": [
      {
        "id": "mem-1",
        "kind": "invariant",
        "tier": "long_term",
        "text": "The standard HTTP client is httpx."
      },
      {
        "id": "mem-2",
        "kind": "pattern",
        "tier": "short_term",
        "text": "SSL errors with requests → switch to httpx"
      }
    ],
    "token_budget": 400
  },
  "id": 3
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "context_prompt": "Project invariants:\n- The standard HTTP client is httpx.\n\nRecent patterns:\n- SSL errors with requests → switch to httpx",
    "used_memory_ids": ["mem-1", "mem-2"]
  },
  "id": 3
}
```

**Errors:**

| Code | Message | When |
|------|---------|------|
| -32010 | Empty task | Task string empty |

**Model Tier:** fast_model

---

### IPC-004: search_memories

Daemon → Memory Service (via daemon). Semantic search across memories.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "search_memories",
  "params": {
    "project_id": "payment-api",
    "query": "HTTP client SSL",
    "top_k": 10,
    "filters": {
      "kind": "pattern",
      "tier": null,
      "status": "active",
      "key": null
    }
  },
  "id": 4
}
```

**Input Fields:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| project_id | string | Yes | - | Project identifier |
| query | string | Yes | - | Search query |
| top_k | integer | No | 10 | Max results |
| filters.kind | string | No | - | Filter by kind |
| filters.tier | string | No | - | Filter by tier |
| filters.status | string | No | - | Filter by status |
| filters.key | string | No | - | Filter by declarative key |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "results": [
      {
        "id": "mem-1",
        "kind": "pattern",
        "tier": "short_term",
        "status": "active",
        "key": null,
        "text": "SSL errors with requests → switch to httpx",
        "score": 0.89,
        "confidence": 0.85
      }
    ]
  },
  "id": 4
}
```

**Errors:**

| Code | Message | When |
|------|---------|------|
| -32010 | Project not initialized | No .sqrl/squirrel.db |
| -32012 | Empty query | Query string empty |

---

### IPC-005: forget_memory

CLI → Daemon. Soft delete memory.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "method": "forget_memory",
  "params": {
    "id": "mem-123",
    "confirm": true
  },
  "id": 5
}
```

**Input Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| id | string | No | Memory ID (if known) |
| query | string | No | Search query (if ID unknown) |
| confirm | boolean | No | Skip confirmation (default false) |

**Response:**
```json
{
  "jsonrpc": "2.0",
  "result": {
    "forgotten": 1,
    "ids": ["mem-123"]
  },
  "id": 5
}
```

**Errors:**

| Code | Message | When |
|------|---------|------|
| -32030 | Memory not found | ID doesn't exist |
| -32031 | Confirmation required | Multiple matches, needs confirm |

---

## External Interfaces

### EXT-001: get_task_context

MCP/CLI → Daemon. Get relevant memories for a task.

**Request:**
```json
{
  "project_root": "/home/alice/projects/payment-api",
  "task": "add a webhook endpoint that calls our partner API",
  "owner_type": "user",
  "owner_id": "alice",
  "context_budget_tokens": 400
}
```

**Response:**
```json
{
  "context_prompt": "Project invariants:\n- The standard HTTP client is httpx.\n\nRecent patterns:\n- SSL errors with requests → switch to httpx",
  "memory_ids": ["mem-1", "mem-2"]
}
```

**Note:** Daemon handles retrieval internally:
1. Calls IPC-002 embed_text for task vector
2. Queries SQLite for matching memories
3. Ranks by similarity, status, tier, kind
4. Optionally calls IPC-003 compose_context OR uses templates
5. Updates memory_metrics.use_count and opportunities

---

## MCP Tools

### MCP-001: squirrel_get_task_context

Wraps EXT-001 for MCP clients.

**Tool Definition:**
```json
{
    "name": "squirrel_get_task_context",
    "description": "Get relevant memories for a coding task. Call BEFORE fixing bugs, refactoring, or adding features.",
    "inputSchema": {
        "type": "object",
        "properties": {
            "project_root": {
                "type": "string",
                "description": "Absolute path to project root"
            },
            "task": {
                "type": "string",
                "description": "Description of the task"
            },
            "context_budget_tokens": {
                "type": "integer",
                "default": 400,
                "description": "Max tokens for context"
            }
        },
        "required": ["project_root", "task"]
    }
}
```

---

### MCP-002: squirrel_search_memory

Wraps IPC-004 for MCP clients.

**Tool Definition:**
```json
{
    "name": "squirrel_search_memory",
    "description": "Search memories by semantic query",
    "inputSchema": {
        "type": "object",
        "properties": {
            "project_root": {
                "type": "string",
                "description": "Absolute path to project root"
            },
            "query": {
                "type": "string",
                "description": "Search query"
            },
            "top_k": {
                "type": "integer",
                "default": 10,
                "description": "Max results"
            },
            "kind": {
                "type": "string",
                "description": "Filter by kind (preference, invariant, pattern, guard, note)"
            },
            "tier": {
                "type": "string",
                "description": "Filter by tier (short_term, long_term, emergency)"
            }
        },
        "required": ["project_root", "query"]
    }
}
```

---

## CLI Commands

### CLI-001: sqrl init

Initialize project for Squirrel.

**Usage:** `sqrl init [--skip-history]`

**Flags:**

| Flag | Description |
|------|-------------|
| `--skip-history` | Don't ingest historical logs |

**Actions:**
1. Create `.sqrl/squirrel.db`
2. Scan CLI log folders for project mentions
3. Ingest history (unless --skip-history) - see Historical Processing below
4. Configure MCP for enabled CLIs
5. Inject instructions into agent files
6. Register in `~/.sqrl/projects.json`

**Historical Processing (ADR-011):**

Timeline starts from **earliest session log** for the project, not today's date.

| Step | Action |
|------|--------|
| 1 | Find all session logs mentioning this project |
| 2 | Sort by timestamp (oldest first) |
| 3 | Process episodes chronologically |
| 4 | For each episode: run Memory Writer, then simulate CR-Memory evaluation |
| 5 | Memories earn tier promotions based on actual historical usage |

**Result:** After init, memories already have proven tiers (short_term/long_term) based on historical evidence. No cold start.

---

### CLI-002: sqrl config

Configure CLI selection and settings.

**Usage:** `sqrl config [key] [value]`

**Interactive mode:** `sqrl config` (no args)
**Set value:** `sqrl config llm.model claude-sonnet`
**Get value:** `sqrl config llm.model`

---

### CLI-003: sqrl sync

Update all projects with new CLI configs.

**Usage:** `sqrl sync`

**Actions:**
1. Read enabled CLIs from config
2. For each registered project:
   - Add missing MCP configs
   - Add missing agent instructions

---

### CLI-004: sqrl search

Search memories.

**Usage:** `sqrl search "<query>" [--kind <kind>] [--tier <tier>]`

---

### CLI-005: sqrl forget

Soft delete memory.

**Usage:** `sqrl forget <id>` or `sqrl forget "<query>"`

---

### CLI-006: sqrl export

Export memories as JSON.

**Usage:** `sqrl export [--kind <kind>] [--project]`

**Examples:**
- `sqrl export --kind pattern` - Export all patterns
- `sqrl export --kind invariant --project` - Export project invariants

---

### CLI-007: sqrl import

Import memories from JSON.

**Usage:** `sqrl import <file>`

---

### CLI-008: sqrl status

Show daemon and memory stats.

**Usage:** `sqrl status`

---

### CLI-009: sqrl update

Self-update via axoupdater.

**Usage:** `sqrl update`

---

### CLI-010: sqrl flush

Force episode boundary and Memory Writer processing.

**Usage:** `sqrl flush`

---

### CLI-011: sqrl policy

Manage memory policy.

**Usage:** `sqrl policy [show|reload]`

- `sqrl policy show` - Display current policy from memory_policy.toml
- `sqrl policy reload` - Reload policy without daemon restart (v2)

---

## Project Initialization

### INIT-001: sqrl init - Agent & MCP Integration

Specifies how `CLI-001 sqrl init` discovers AI tools, wires Squirrel as an MCP server, and manages generated files.

---

### INIT-001-A: Agent Registry & Detection

| Agent ID | Agent Name | Detection Signals | Rules File(s) | MCP Config Path | v1 Support |
|----------|------------|-------------------|---------------|-----------------|------------|
| claude | Claude Code | `CLAUDE.md`, `.mcp.json` | `CLAUDE.md` | `.mcp.json` | yes |
| cursor | Cursor | `.cursor/`, `.cursor/mcp.json` | `AGENTS.md` | `.cursor/mcp.json` | yes |
| codex_cli | OpenAI Codex CLI | `.codex/config.toml` | `AGENTS.md` | `.codex/config.toml` | yes |
| gemini | Gemini CLI | `.gemini/settings.json` | `AGENTS.md` | `.gemini/settings.json` | yes |
| copilot | GitHub Copilot | `.vscode/`, `AGENTS.md` | `AGENTS.md` | `.vscode/mcp.json` | future |
| windsurf | Windsurf | `.windsurf/` | `AGENTS.md` | `.windsurf/mcp_config.json` | future |
| aider | Aider | `.aider.conf.yml` | `AGENTS.md` | `.mcp.json` | future |
| cline | Cline | `.clinerules` | `.clinerules` | - | future |
| zed | Zed | `.zed/settings.json` | `AGENTS.md` | `.zed/settings.json` | future |

**Detection Rules:**

1. **Global source of truth:** `~/.sqrl/config.toml` `[agents]` section defines which agents are enabled
2. **CLI override:** `sqrl init --agents <ids>` targets only specified agents (even if disabled in config)
3. **Detection semantics:**
   - If agent is enabled in global config OR explicitly listed via `--agents`: `sqrl init` MUST configure it, creating rules/MCP files if missing
   - If agent is disabled but detection signals are present: `sqrl init` MAY log a hint but MUST NOT modify files without explicit enablement

---

### INIT-001-B: MCP Configuration Merge

**Merge Behavior (v1 strategy: merge-only):**

For each agent with an MCP config path:

1. **If config file does not exist:** Create minimal valid config with Squirrel MCP server entry
2. **If config exists:**
   - Parse existing config
   - Add or update the Squirrel server entry
   - MUST NOT remove or alter any non-Squirrel MCP servers
   - MUST NOT delete/rename/reorder other servers
   - Preserve unknown fields and formatting as much as practical

**Squirrel MCP Server Contract:**

| Field | Value | Notes |
|-------|-------|-------|
| Server ID | `squirrel` | Key name in MCP servers object |
| command | `sqrl` | Squirrel CLI binary |
| args | `["mcp"]` | Launch MCP server mode |

**Format-Specific Notes:**

| Format | Path Examples | Server Location |
|--------|---------------|-----------------|
| JSON | `.mcp.json`, `.cursor/mcp.json` | `mcpServers.squirrel` |
| TOML | `.codex/config.toml` | `[mcp_servers.squirrel]` |

---

### INIT-001-C: Instructions File Management

On `sqrl init`, for each configured agent:

1. Ensure the appropriate rules file(s) exist per INIT-001-A table
2. **If rules file does not exist:** Create with minimal Squirrel section
3. **If rules file exists:** Append/update Squirrel section using managed block markers
4. MUST NOT remove or rewrite unrelated user content

**Squirrel Instructions Block:**

```markdown
<!-- START Squirrel Instructions -->
## Squirrel Memory System

This project uses Squirrel for persistent memory across sessions.

**MCP Tools Available:**
- `squirrel_get_task_context` - Get relevant memories before coding tasks
- `squirrel_search_memory` - Search memories by query

**When to Use:**
- Before fixing bugs: call `squirrel_get_task_context` to recall past issues
- Before refactoring: search for architectural decisions
- After completing tasks: memories are extracted automatically from logs
<!-- END Squirrel Instructions -->
```

---

### INIT-001-D: .gitignore Management

**Managed Block Format:**

```gitignore
# START Squirrel Generated Files
/.sqrl/
# END Squirrel Generated Files
```

**Behavior:**

1. **If `.gitignore` does not exist:** Create with managed block
2. **If `.gitignore` exists with block:** Replace content between markers only
3. **If `.gitignore` exists without block:** Append block at end
4. MUST NOT change any lines outside these markers

---

### INIT-001-E: Backup Behavior

Before modifying any existing file (rules files, MCP configs, `.gitignore`):

1. Create sibling backup with suffix `.sqrl.bak` (e.g., `.mcp.json.sqrl.bak`)
2. If `.sqrl.bak` already exists, it MAY be overwritten (latest backup wins)
3. MUST NOT delete or touch any other backup schemes the user might have

---

## Internal Interfaces

### INT-001: CR-Memory Evaluation

CR-Memory evaluation is an internal background job. It does NOT expose a dedicated IPC method in v1.

**Trigger:** Periodic cron or after N episodes processed.

**Behavior:** See FLOW-004 in ARCHITECTURE.md and algorithm in POLICY.md.

