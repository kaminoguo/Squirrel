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

Daemon → Agent. Process episode, extract memories.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| episode.id | string | Yes | Episode UUID |
| episode.repo | string | Yes | Absolute project path |
| episode.start_ts | string | Yes | ISO 8601 |
| episode.end_ts | string | Yes | ISO 8601 |
| episode.events | array | Yes | Array of SCHEMA-002 events |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| memories_created | integer | Number of new memories |
| memories_updated | integer | Number of merged/updated |
| memories_invalidated | integer | Number of contradicted facts |
| segments | array | Segment summaries |

**Errors:**
| Code | Message | When |
|------|---------|------|
| -32001 | Episode empty | No events in episode |
| -32002 | Invalid repo | Repo path doesn't exist |
| -32003 | LLM error | LLM call failed |

**Model Tier:** strong_model

---

### IPC-002: get_task_context

MCP → Agent (via daemon). Retrieve relevant memories for task.

**Input:**
| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| project_root | string | Yes | - | Absolute project path |
| task | string | Yes | - | Task description |
| context_budget_tokens | integer | No | 400 | Max tokens in response |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| context_prompt | string | Ready-to-inject prompt text |
| memory_ids | array | IDs of selected memories |
| tokens_used | integer | Actual token count |

**Errors:**
| Code | Message | When |
|------|---------|------|
| -32010 | Project not initialized | No .sqrl/squirrel.db |
| -32011 | Empty task | Task string empty |

**Model Tier:** fast_model

**Fast Path:** Returns empty in <20ms for trivial tasks (typo fixes, comments).

---

### IPC-003: search_memories

MCP → Agent (via daemon). Semantic search across memories.

**Input:**
| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| project_root | string | Yes | - | Absolute project path |
| query | string | Yes | - | Search query |
| top_k | integer | No | 10 | Max results |
| filters.memory_type | string | No | - | Filter by type |
| filters.outcome | string | No | - | Filter by outcome |
| filters.key | string | No | - | Filter by declarative key |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| results | array | Matched memories with scores |
| results[].id | string | Memory ID |
| results[].text | string | Memory content |
| results[].score | float | Similarity score |
| results[].memory_type | string | Type |

**Errors:**
| Code | Message | When |
|------|---------|------|
| -32010 | Project not initialized | No .sqrl/squirrel.db |
| -32012 | Empty query | Query string empty |

---

### IPC-004: execute_command

CLI → Agent. Natural language or direct command.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| command | string | Yes | User input (natural language or direct) |
| cwd | string | Yes | Current working directory |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| response | string | Agent response text |
| action_taken | string | What was done |
| memories_affected | integer | Memories created/updated/deleted |

**Errors:**
| Code | Message | When |
|------|---------|------|
| -32020 | Unknown command | Can't interpret input |
| -32021 | Permission denied | Requires confirmation |

**Model Tier:** fast_model

---

### IPC-005: forget_memory

CLI → Agent. Soft delete memory.

**Input:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| id | string | No | Memory ID (if known) |
| query | string | No | Search query (if ID unknown) |
| confirm | boolean | No | Skip confirmation (default false) |

**Output:**
| Field | Type | Description |
|-------|------|-------------|
| forgotten | integer | Number of memories soft-deleted |
| ids | array | IDs of affected memories |

**Errors:**
| Code | Message | When |
|------|---------|------|
| -32030 | Memory not found | ID doesn't exist |
| -32031 | Confirmation required | Multiple matches, needs confirm |

---

## MCP Tools

### MCP-001: squirrel_get_task_context

Wraps IPC-002 for MCP clients.

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

Wraps IPC-003 for MCP clients.

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
3. Ingest recent history (unless --skip-history)
4. Configure MCP for enabled CLIs
5. Inject instructions into agent files
6. Register in `~/.sqrl/projects.json`

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

**Usage:** `sqrl search "<query>"`

---

### CLI-005: sqrl forget

Soft delete memory.

**Usage:** `sqrl forget <id>` or `sqrl forget "<query>"`

---

### CLI-006: sqrl export

Export memories as JSON.

**Usage:** `sqrl export [type] [--project]`

**Examples:**
- `sqrl export lesson` - Export all lessons
- `sqrl export fact --project` - Export project facts

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
