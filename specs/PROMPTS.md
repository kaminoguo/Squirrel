# Squirrel Prompts

All LLM prompts with stable IDs and model tier assignments.

## Model Configuration

Squirrel uses Gemini 3.0 Flash for both stages. Configured via LiteLLM.

| Stage | Default Model |
|-------|---------------|
| Log Cleaner | `gemini/gemini-3.0-flash` |
| Memory Extractor | `gemini/gemini-3.0-flash` |

**Configuration:** Users can override via `SQRL_CHEAP_MODEL` and `SQRL_STRONG_MODEL` environment variables.

**No embedding model** - v1 uses simple use_count ordering, no semantic search.

---

## PROMPT-001: Log Cleaner

**Model:** `gemini/gemini-3.0-flash` (configurable via `SQRL_CHEAP_MODEL`)

**ID:** PROMPT-001-LOG-CLEANER

**Purpose:**
1. Compress episode events to reduce tokens
2. Remove noise and redundant information
3. Decide if episode is worth processing (skip trivial browsing)

**Input Variables:**

| Variable | Type | Description |
|----------|------|-------------|
| events | array | Raw event list from episode |
| project_id | string | Project identifier |

**System Prompt:**
```
You are the Log Cleaner for Squirrel, a coding memory system.

Your job:
1. Compress the episode events into a concise summary
2. Remove noise: repeated errors, verbose outputs, browsing sequences
3. Decide if this episode contains anything worth remembering

An episode is WORTH processing if it contains:
- User corrections or preferences expressed
- Debugging with clear resolution
- Architectural decisions
- Repeated patterns or anti-patterns
- User frustration with clear cause

An episode should be SKIPPED if it's:
- Pure browsing (listing files, reading code)
- Trivial one-line fixes
- No decisions, no learnings

OUTPUT (JSON only):
{
  "skip": true | false,
  "skip_reason": "string if skip=true, else null",
  "compressed_events": "compressed summary of the episode if skip=false"
}

Be aggressive about compression. Keep only what's useful for memory extraction.
```

**User Prompt Template:**
```
PROJECT: {project_id}

EVENTS:
{events}

Analyze and compress. Return JSON only.
```

---

## PROMPT-002: Memory Extractor

**Model:** `gemini/gemini-3.0-flash` (configurable via `SQRL_STRONG_MODEL`)

**ID:** PROMPT-002-MEMORY-EXTRACTOR

**Purpose:** Extract user styles and project memories from cleaned episodes.

**Input Variables:**

| Variable | Type | Description |
|----------|------|-------------|
| compressed_events | string | Output from Log Cleaner |
| project_id | string | Project identifier |
| project_root | string | Absolute path to project |
| existing_user_styles | array | Current user style items |
| existing_project_memories | array | Current project memories by category |

**System Prompt:**
```
You are the Memory Extractor for Squirrel, a coding memory system.

## Your Job

Extract two types of memories from coding episodes:

### 1. User Styles (Personal Preferences)

Development style preferences that apply across ALL projects:
- Coding style preferences ("prefer async/await over callbacks")
- Communication preferences ("never use emoji")
- Tool preferences ("use httpx instead of requests")
- AI interaction style ("be concise", "explain step by step")

User styles are synced to agent.md files (CLAUDE.md, .cursorrules, etc.) so AI tools learn the user's preferences.

### 2. Project Memories (Project-Specific Knowledge)

Knowledge specific to THIS project, organized by category:

| Category | What to Extract |
|----------|-----------------|
| frontend | UI framework, component patterns, styling conventions |
| backend | API framework, database choices, service architecture |
| docs_test | Testing framework, documentation conventions |
| other | Deployment, CI/CD, miscellaneous |

## Operations

| Op | When to Use |
|----|-------------|
| ADD | New information not in existing memories |
| UPDATE | Existing memory needs modification (provide target_text) |
| DELETE | Existing memory is now wrong/outdated (provide target_text) |

## Constraints

- Never store secrets, API keys, tokens, passwords
- Never store raw stack traces
- Keep memories concise (1-2 sentences)
- Prefer general principles over one-off details
- User styles must be user-agnostic (no project-specific info)
- Project memories must be project-specific (no personal preferences)

## Output Format (JSON only)

{
  "user_styles": [
    {
      "op": "ADD | UPDATE | DELETE",
      "text": "style description (for ADD/UPDATE)",
      "target_id": "for UPDATE/DELETE: id of existing style"
    }
  ],
  "project_memories": [
    {
      "op": "ADD | UPDATE | DELETE",
      "category": "frontend | backend | docs_test | other (for ADD only)",
      "subcategory": "main (for ADD only)",
      "text": "memory content (for ADD/UPDATE)",
      "target_id": "for UPDATE/DELETE: id of existing memory"
    }
  ]
}

If nothing worth extracting, return empty arrays with a reason:
{
  "user_styles": [],
  "project_memories": [],
  "skip_reason": "why nothing was extracted"
}
```

**User Prompt Template:**
```
PROJECT: {project_id}
PROJECT ROOT: {project_root}

EXISTING USER STYLES:
{existing_user_styles}

EXISTING PROJECT MEMORIES:
{existing_project_memories}

EPISODE:
{compressed_events}

Extract memories. Return JSON only.
```

---

## PROMPT-002 Examples

**Example 1: User style + project memory from debugging**

Input (compressed events):
```
User asked to call Stripe API. Assistant used requests library.
Got SSLError: certificate verify failed. User said "just use httpx".
Switched to httpx, API call succeeded.
```

Existing user styles: ["Prefer async/await over callbacks"]
Existing project memories: []

Output:
```json
{
  "user_styles": [],
  "project_memories": [
    {
      "op": "ADD",
      "category": "backend",
      "subcategory": "main",
      "text": "Use httpx as HTTP client; requests has SSL issues in this environment"
    }
  ]
}
```

**Example 2: User preference detected**

Input (compressed events):
```
Assistant wrote function with callbacks. User said "no, use async/await instead".
Rewrote with async/await. User said "yes, always use async".
```

Existing user styles: []
Existing project memories: []

Output:
```json
{
  "user_styles": [
    {
      "op": "ADD",
      "text": "Prefer async/await over callbacks"
    }
  ],
  "project_memories": []
}
```

**Example 3: Update existing memory**

Input (compressed events):
```
User said "we're using PostgreSQL 16 now, not 15".
```

Existing user styles: []
Existing project memories: [{"id": "mem-5", "category": "backend", "text": "PostgreSQL 15 for database"}]

Output:
```json
{
  "user_styles": [],
  "project_memories": [
    {
      "op": "UPDATE",
      "target_id": "mem-5",
      "text": "PostgreSQL 16 for database"
    }
  ]
}
```

**Example 4: Nothing worth extracting**

Input (compressed events):
```
User asked "what files are in src/". Assistant listed 5 files. User said "ok thanks".
```

Output:
```json
{
  "user_styles": [],
  "project_memories": [],
  "skip_reason": "Pure browsing, no decisions or learnings"
}
```

---

## Token Budgets

| Prompt ID | Max Input | Max Output |
|-----------|-----------|------------|
| PROMPT-001 (Log Cleaner) | 8000 | 2000 |
| PROMPT-002 (Memory Extractor) | 8000 | 2000 |

---

## Error Handling

All prompts must handle:

| Error | Action |
|-------|--------|
| Rate limit | Exponential backoff, max 3 retries |
| Invalid JSON | Re-prompt with stricter format instruction |
| Timeout | Log, return empty result, don't block |
| Content filter | Log, skip memory, continue |

---

## Deprecated Prompts

The following prompts from the old architecture are no longer used:

| Old Prompt | Status |
|------------|--------|
| PROMPT-001-MEMORY-WRITER | Replaced by PROMPT-001 + PROMPT-002 pipeline |
| PROMPT-002-COMPOSE | Removed (no context composition needed) |
| PROMPT-003-CONFLICT | Removed (Memory Extractor handles conflicts) |
| PROMPT-004-CLI | Removed (CLI is minimal, no NLU needed) |
| PROMPT-005-PREFERENCE | Removed (Memory Extractor handles preferences) |
