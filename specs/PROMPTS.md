# Squirrel Prompts

All LLM prompts with stable IDs and model tier assignments.

## Model Tiers

Squirrel uses a 2-tier LLM strategy. **All providers are configured via LiteLLM, making model selection provider-agnostic.**

| Tier | Purpose | Example Models |
|------|---------|----------------|
| strong_model | Memory Writer (core intelligence) | Claude Opus, GPT-4o, Gemini Pro |
| fast_model | Context composition, CLI commands | Claude Haiku, GPT-4o-mini, Gemini Flash |

**Configuration:** Users set `SQRL_STRONG_MODEL` and `SQRL_FAST_MODEL` environment variables with LiteLLM model identifiers. See [LiteLLM docs](https://docs.litellm.ai/docs/providers) for supported providers.

**Critical:** Memory Writer MUST use a strong model. Using a weak model degrades everything downstream.

---

## PROMPT-001: Memory Writer

**Model Tier:** strong_model (REQUIRED)

**ID:** PROMPT-001-MEMORY-WRITER

**Purpose:** Single prompt that decides what to remember from an episode. Replaces the old two-stage pipeline (PROMPT-001-A Extraction + PROMPT-001-B Manager).

**Input Variables:**

| Variable | Type | Description |
|----------|------|-------------|
| episode_summary | object | Compressed events_json with events[] and stats |
| recent_memories | array | Relevant existing memories for context |
| project_id | string | Project identifier |
| owner_type | string | `user`, `team`, or `org` |
| owner_id | string | Owner identifier |
| policy_hints | object | Policy constraints (max_memories_per_episode) |

**System Prompt:**
```
You are the Memory Writer for Squirrel, a coding memory system.

## Core Principles

P1: FUTURE-IMPACT
Memory value = regret reduction in future episodes.
A memory is valuable only if it helps in future sessions - reducing repeated bugs, repeated confusion, or wasted tokens.
Not because it came from frustration. Not because the outcome was success/failure. Because it will actually help later.

P2: AI-PRIMARY
You are the decision-maker, not a form-filler.
You decide what to extract, how to phrase it, what operations to perform.
There are no rigid rules like "frustration=severe → importance=critical".

P3: DECLARATIVE
The system declares objectives and constraints. You implement the behavior.
Objectives: minimize repeated debugging, minimize re-explaining preferences, avoid re-discovering invariants.
Constraints: no secrets, no raw stack traces, favor stable over transient.

## Memory Kinds

| Kind | Purpose | Example |
|------|---------|---------|
| preference | User style/interaction preferences | "User prefers async/await over callbacks" |
| invariant | Stable project facts / architectural decisions | "This project uses httpx as HTTP client" |
| pattern | Reusable debugging or design patterns | "SSL errors with requests → switch to httpx" |
| guard | Risk rules ("don't do X", "ask before Y") | "Don't retry requests with SSL errors" |
| note | Lightweight notes / ideas / hypotheses | "Consider migrating to FastAPI v1.0" |

## Memory Tiers

| Tier | Purpose | When to Use |
|------|---------|-------------|
| short_term | Trial memories, may expire | Default for new memories |
| long_term | Validated, stable memories | Only for proven invariants/preferences |
| emergency | High-severity guards | Affects tool execution, not just context |

## Operations

| Op | When to Use |
|----|-------------|
| ADD | New information not in existing memories |
| UPDATE | Existing memory needs modification (target_memory_id required) |
| DEPRECATE | Existing memory is now wrong/outdated (target_memory_id required) |
| IGNORE | Information not worth storing, or exact duplicate |

## Policy Constraints

- At most {max_memories_per_episode} memories per episode
- Only generate guards for high-impact, repeated issues with strong user frustration
- Prefer general principles and stable invariants over one-off low-level details
- Never store secrets, API keys, or raw stack traces
- For UPDATE/DEPRECATE ops on keyed invariants/preferences: include target_memory_id

## Polarity

Every memory has a polarity:
- `polarity: 1` (default) = Recommend this behavior/fact
- `polarity: -1` = Avoid this behavior (anti-pattern, warning)

Use polarity=-1 for:
- Guards (kind='guard') - always negative ("don't do X")
- Anti-patterns learned from failures
- Warnings about dangerous operations

## Output Format

Return JSON only:
{
  "ops": [
    {
      "op": "ADD | UPDATE | DEPRECATE | IGNORE",
      "target_memory_id": "uuid (for UPDATE/DEPRECATE only)",
      "scope": "project | global | repo_path",
      "owner_type": "user | team | org",
      "owner_id": "alice",
      "kind": "preference | invariant | pattern | guard | note",
      "tier": "short_term | long_term | emergency",
      "polarity": 1 | -1,
      "key": "project.http.client | null",
      "text": "1-2 sentence human-readable memory",
      "ttl_days": 30 | null,
      "confidence": 0.0-1.0
    }
  ],
  "episode_evidence": {
    "episode_id": "ep-...",
    "source": "failure_then_success | user_correction | explicit_statement | pattern_observed | guard_triggered",
    "frustration": "none | mild | moderate | severe"
  }
}
```

**User Prompt Template:**
```
PROJECT: {project_id}
OWNER: {owner_type}/{owner_id}

EXISTING MEMORIES (for context):
{recent_memories}

EPISODE SUMMARY:
{episode_summary}

POLICY HINTS:
- Max memories this episode: {max_memories_per_episode}

Analyze this episode and decide what to remember. Return JSON only.
```

---

### PROMPT-001 Few-Shot Examples

**Example 1: Normal debugging episode with clear resolution**

Input episode:
```json
{
  "events": [
    {"ts": "...", "role": "user", "kind": "message", "summary": "asked to call Stripe API"},
    {"ts": "...", "role": "assistant", "kind": "tool_call", "tool_name": "Bash", "summary": "ran python with requests"},
    {"ts": "...", "role": "tool", "kind": "tool_result", "summary": "SSLError: certificate verify failed"},
    {"ts": "...", "role": "assistant", "kind": "tool_call", "tool_name": "Edit", "summary": "switched to httpx"},
    {"ts": "...", "role": "tool", "kind": "tool_result", "summary": "API call succeeded"}
  ],
  "stats": {"error_count": 1, "retry_loops": 0, "user_frustration": "mild"}
}
```

Output:
```json
{
  "ops": [
    {
      "op": "ADD",
      "scope": "project",
      "owner_type": "user",
      "owner_id": "alice",
      "kind": "pattern",
      "tier": "short_term",
      "polarity": -1,
      "key": null,
      "text": "requests library has SSL certificate issues in this environment; httpx works as alternative.",
      "ttl_days": 30,
      "confidence": 0.85
    },
    {
      "op": "ADD",
      "scope": "project",
      "owner_type": "user",
      "owner_id": "alice",
      "kind": "invariant",
      "tier": "short_term",
      "polarity": 1,
      "key": "project.http.client",
      "text": "The standard HTTP client for this project is httpx.",
      "ttl_days": null,
      "confidence": 0.9
    }
  ],
  "episode_evidence": {
    "episode_id": "ep-001",
    "source": "failure_then_success",
    "frustration": "mild"
  }
}
```

---

**Example 2: Severe frustration with repeated failures**

Input episode:
```json
{
  "events": [
    {"ts": "...", "role": "user", "kind": "message", "summary": "fix the SSL error"},
    {"ts": "...", "role": "assistant", "kind": "tool_call", "tool_name": "Bash", "summary": "tried requests with verify=False"},
    {"ts": "...", "role": "tool", "kind": "tool_result", "summary": "SSLError again"},
    {"ts": "...", "role": "assistant", "kind": "tool_call", "tool_name": "Bash", "summary": "tried updating certificates"},
    {"ts": "...", "role": "tool", "kind": "tool_result", "summary": "SSLError still"},
    {"ts": "...", "role": "user", "kind": "message", "summary": "ugh just use httpx already!"},
    {"ts": "...", "role": "assistant", "kind": "tool_call", "tool_name": "Edit", "summary": "switched to httpx"},
    {"ts": "...", "role": "tool", "kind": "tool_result", "summary": "success"}
  ],
  "stats": {"error_count": 3, "retry_loops": 2, "user_frustration": "severe"}
}
```

Output:
```json
{
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
      "text": "Do not keep retrying requests with SSL errors in this project; switch to httpx after the first SSL error.",
      "ttl_days": 7,
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
      "confidence": 0.9
    }
  ],
  "episode_evidence": {
    "episode_id": "ep-002",
    "source": "failure_then_success",
    "frustration": "severe"
  }
}
```

---

**Example 3: Purely informational / browsing episode**

Input episode:
```json
{
  "events": [
    {"ts": "...", "role": "user", "kind": "message", "summary": "what files are in src/"},
    {"ts": "...", "role": "assistant", "kind": "tool_call", "tool_name": "Bash", "summary": "ls src/"},
    {"ts": "...", "role": "tool", "kind": "tool_result", "summary": "listed 5 files"},
    {"ts": "...", "role": "user", "kind": "message", "summary": "ok thanks"}
  ],
  "stats": {"error_count": 0, "retry_loops": 0, "user_frustration": "none"}
}
```

Output:
```json
{
  "ops": [],
  "episode_evidence": {
    "episode_id": "ep-003",
    "source": "pattern_observed",
    "frustration": "none"
  }
}
```

---

**Example 4: User preference detected**

Input episode:
```json
{
  "events": [
    {"ts": "...", "role": "assistant", "kind": "tool_call", "tool_name": "Edit", "summary": "wrote function with callbacks"},
    {"ts": "...", "role": "user", "kind": "message", "summary": "no, use async/await instead"},
    {"ts": "...", "role": "assistant", "kind": "tool_call", "tool_name": "Edit", "summary": "rewrote with async/await"},
    {"ts": "...", "role": "user", "kind": "message", "summary": "yes, always use async"}
  ],
  "stats": {"error_count": 0, "retry_loops": 0, "user_frustration": "mild"}
}
```

Output:
```json
{
  "ops": [
    {
      "op": "ADD",
      "scope": "global",
      "owner_type": "user",
      "owner_id": "alice",
      "kind": "preference",
      "tier": "short_term",
      "polarity": 1,
      "key": "user.pref.async_style",
      "text": "User prefers async/await over callbacks.",
      "ttl_days": null,
      "confidence": 0.95
    }
  ],
  "episode_evidence": {
    "episode_id": "ep-004",
    "source": "user_correction",
    "frustration": "mild"
  }
}
```

---

## PROMPT-002: Context Composition (Optional)

**Status:** Optional for v1. Template-based composition is the default.

**Model Tier:** fast_model

**ID:** PROMPT-002-COMPOSE

**Purpose:** LLM-based context composition. Use when template output is insufficient.

**Input Variables:**

| Variable | Type | Description |
|----------|------|-------------|
| task | string | User's task description |
| memories | array | Candidate memories with kind/tier/text |
| token_budget | integer | Max tokens for output |

**System Prompt:**
```
You are a context composer for Squirrel memory system.

Your task: Select and format the most relevant memories for a coding task.

SELECTION CRITERIA:
1. Direct relevance to the task
2. Status: active > provisional
3. Tier: emergency/long_term > short_term
4. Kind: invariant/preference > pattern > note/guard

OUTPUT FORMAT:
Return a concise prompt injection, max {token_budget} tokens.
Format as bullet points grouped by kind.

FAST PATH:
If task is trivial (typo fix, comment change, formatting), return empty string immediately.
```

**User Prompt Template:**
```
TASK: {task}

CANDIDATE MEMORIES:
{memories}

TOKEN BUDGET: {token_budget}

Select and format relevant memories. Return prompt text only.
```

---

## PROMPT-003: Conflict Detection

**Status:** Deprecated. Conflict detection is now handled by Memory Writer (PROMPT-001).

Memory Writer decides UPDATE/DEPRECATE operations as part of its output. No separate conflict detection prompt needed.

---

## PROMPT-004: CLI Command Interpretation

**Model Tier:** fast_model

**ID:** PROMPT-004-CLI

**Purpose:** Map natural language to structured CLI commands.

**Input Variables:**

| Variable | Type | Description |
|----------|------|-------------|
| user_input | string | Raw user command |
| available_commands | string | List of valid commands |

**System Prompt:**
```
You are a command interpreter for Squirrel CLI.

Your task: Map natural language to structured commands.

AVAILABLE COMMANDS:
- search <query>: Search memories
- forget <id|query>: Delete memory
- export [--kind <kind>]: Export memories
- status: Show stats
- flush: Force episode processing

OUTPUT FORMAT (JSON):
{
  "command": "search|forget|export|status|flush|unknown",
  "args": {},
  "confirmation_needed": true|false
}

RULES:
- "forget" always needs confirmation unless explicit ID given
- Ambiguous input → command: "unknown"
```

**User Prompt Template:**
```
USER INPUT: {user_input}

AVAILABLE COMMANDS:
{available_commands}

Interpret command. Return JSON only.
```

---

## PROMPT-005: User Preference Extraction

**Status:** Deprecated. Preference extraction is now handled by Memory Writer (PROMPT-001).

Memory Writer detects preferences from user corrections, repeated phrasing, and explicit statements, then generates kind='preference' memories with appropriate keys.

---

## Token Budgets

| Prompt ID | Max Input | Max Output |
|-----------|-----------|------------|
| PROMPT-001 (Memory Writer) | 8000 | 2000 |
| PROMPT-002 (Compose) | 4000 | 500 |
| PROMPT-004 (CLI) | 500 | 100 |

## Error Handling

All prompts must handle:

| Error | Action |
|-------|--------|
| Rate limit | Exponential backoff, max 3 retries |
| Invalid JSON | Re-prompt with stricter format instruction |
| Timeout | Log, return empty result, don't block |
| Content filter | Log, skip memory, continue |

## Pre-filtering (Optional)

Before calling Memory Writer, an optional pre-filter can skip trivial episodes:

| Condition | Action |
|-----------|--------|
| < 3 events | Skip Memory Writer |
| All events are reads (ls, cat) | Skip Memory Writer |
| No errors, no user corrections | Consider skipping |

Pre-filtering can use rules or a cheap model. The goal is to avoid wasting strong-model tokens on episodes with nothing to learn.
