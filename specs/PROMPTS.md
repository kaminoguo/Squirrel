# Squirrel Prompts

All LLM prompts with stable IDs and model tier assignments.

## Model Configuration

Squirrel uses different models for each stage based on task complexity.

| Stage | Default Model | Rationale |
|-------|---------------|-----------|
| Project Summarizer | `google/gemini-3-pro-preview` | Quality summary, cached once per project |
| User Scanner | `google/gemini-3-flash-preview` | Simple detection, high throughput |
| Memory Extractor | `google/gemini-3-pro-preview` | Complex reasoning, quality matters |
| Doc Summarizer | `google/gemini-3-flash-preview` | Short summaries, called per doc |

**Configuration:** Override via `SQRL_CHEAP_MODEL` and `SQRL_STRONG_MODEL` environment variables.

**No embedding model** - v1 uses simple use_count ordering, no semantic search.

---

## Extraction Flow

```
Session ends (idle timeout or explicit flush)
    ↓
Stage 0 (Pro, cached): Summarize project from README
    → Returns: short project description for context
    ↓
Stage 1 (Flash): Scan user messages only
    "Which messages contain behavioral patterns worth extracting?"
    → Returns: indices of flagged messages
    ↓
Stage 2 (Pro): Only flagged messages + AI context + project summary
    "What should AI do differently?"
    → Input: flagged_msg + 3 AI turns before + project context
    → Output: memories with confidence > 0.8
```

**Key Design Decisions:**
- **Session-end processing**: Full context available, reduces mid-conversation noise
- **Two-stage pipeline**: Flash filters cheaply, Pro only sees relevant slices
- **Confidence threshold**: Only store memories with confidence > 0.8 (no user review in v1)

---

## PROMPT-001: User Scanner

**Model:** `google/gemini-3-flash-preview` (configurable via `SQRL_CHEAP_MODEL`)

**ID:** PROMPT-001-USER-SCANNER

**Purpose:** Scan user messages from a completed session to identify which messages contain behavioral patterns worth extracting.

**Timing:** Called at session-end (idle timeout or explicit flush), not per-message.

**Core Insight:** Only process user messages first (minimal tokens). If patterns detected, pull AI context later for Stage 2.

**Input Variables:**

| Variable | Type | Description |
|----------|------|-------------|
| user_messages | array | All user messages from the session (no AI responses) |

**System Prompt:**
```
You scan user messages from a coding session to identify behavioral patterns.

Look for messages where:
- User corrects AI behavior ("no, use X instead", "don't do Y")
- User expresses preferences ("I prefer...", "always...", "never...")
- User shows frustration with AI's approach
- User redirects AI's direction

Skip messages that are:
- Just acknowledgments ("ok", "sure", "continue", "looks good")
- Questions or requests without correction intent
- One-off task instructions unrelated to preferences

Output JSON only:
{"has_patterns": true, "indices": [1, 5, 12]}
or
{"has_patterns": false, "indices": []}
```

**User Prompt Template:**
```
USER MESSAGES FROM SESSION:
{user_messages}

Which messages (if any) contain behavioral patterns worth extracting? Return JSON with indices.
```

---

## PROMPT-002: Memory Extractor

**Model:** `google/gemini-3-pro-preview` (configurable via `SQRL_STRONG_MODEL`)

**ID:** PROMPT-002-MEMORY-EXTRACTOR

**Purpose:** Extract actionable memories from flagged messages. Focus on "what should AI do differently in future sessions?"

**Timing:** Called only for messages flagged by Stage 1.

**Core Insight:** We're not capturing "corrections" - we're capturing **behavioral adjustments** that help future AI sessions.

**Input Variables:**

| Variable | Type | Description |
|----------|------|-------------|
| trigger_message | string | The user message flagged by Stage 1 |
| ai_context | string | The 3 AI turns before the trigger message |
| project_id | string | Project identifier |
| project_root | string | Absolute path to project |
| existing_user_styles | array | Current user style items |
| existing_project_memories | array | Current project memories by category |

**AI Turn Definition:** One AI turn = all content between two user messages (AI responses + tool calls + tool results).

**System Prompt:**
```
Extract behavioral adjustments from user feedback. Output JSON only.

Your task: What should AI do DIFFERENTLY in future sessions?

User Style = global preference (applies to ALL projects)
Project Memory = specific to THIS project only

Example output:
{
  "user_styles": [
    {"op": "ADD", "text": "never use emoji", "confidence": 0.9}
  ],
  "project_memories": []
}

Another example:
{
  "user_styles": [],
  "project_memories": [
    {"op": "ADD", "category": "backend", "text": "use httpx not requests", "confidence": 0.85}
  ]
}

Rules:
- op: "ADD", "UPDATE", or "DELETE"
- category (project only): "frontend", "backend", "docs_test", "other"
- confidence: 0.0-1.0 (only memories with confidence > 0.8 will be stored)
- Empty arrays if nothing worth remembering

Do NOT extract:
- One-off tasks ("make a video about X")
- Temporary debugging steps
- Universal best practices everyone knows
- Things that only apply to this exact moment
- Creative/content specifications (video scripts, character designs, marketing copy)
- Project statistics or data points for specific contexts
- Tasks unrelated to the core project (see PROJECT CONTEXT below)
```

**User Prompt Template:**
```
PROJECT CONTEXT:
{project_summary}

Only extract memories relevant to this project. Skip unrelated side tasks.

EXISTING USER STYLES: {existing_user_styles}
EXISTING PROJECT MEMORIES: {existing_project_memories}

AI CONTEXT (what AI did before user's message):
{ai_context}

USER MESSAGE (the feedback):
{trigger_message}

What behavioral adjustments should be remembered for future sessions?
```

---

## PROMPT-003: Project Summarizer

**Model:** `google/gemini-3-pro-preview` (configurable via `SQRL_STRONG_MODEL`)

**ID:** PROMPT-003-PROJECT-SUMMARIZER

**Purpose:** Generate a short summary of what a project does from its README. Provides context to the Memory Extractor to filter out unrelated tasks.

**Timing:** Called once per project, cached for reuse.

**Core Insight:** With project context, the extractor can distinguish "core project work" from "side tasks" (e.g., marketing videos unrelated to the codebase).

**Input Variables:**

| Variable | Type | Description |
|----------|------|-------------|
| readme_content | string | Contents of project README.md (truncated to 4000 chars) |

**System Prompt:**
```
Summarize this project in 2-3 sentences. Include:
- What the project does (its purpose)
- Main tech stack
- Key domain concepts

Be concise. Output plain text only, no markdown or formatting.
```

**User Prompt Template:**
```
{readme_content}
```

---

## PROMPT-004: Doc Summarizer

**Model:** `google/gemini-3-flash-preview` (configurable via `SQRL_CHEAP_MODEL`)

**ID:** PROMPT-004-DOC-SUMMARIZER

**Purpose:** Generate a brief summary of a documentation file for the doc tree display.

**Timing:** Called when a doc file is created, modified, or renamed (detected by file watcher).

**Core Insight:** Short, informative summaries help AI tools understand project structure at a glance.

**Input Variables:**

| Variable | Type | Description |
|----------|------|-------------|
| file_path | string | Relative path to the doc file |
| content | string | Full content of the doc file (truncated to 8000 chars) |

**System Prompt:**
```
Summarize this documentation file in 1-2 sentences.

Focus on:
- What the document describes or defines
- Key topics covered
- Purpose within the project

Be concise and specific. Output plain text only, no markdown.
Do not start with "This document" or similar phrases.
```

**User Prompt Template:**
```
File: {file_path}

Content:
{content}
```

**Example Outputs:**

| File | Summary |
|------|---------|
| specs/ARCHITECTURE.md | System boundaries, Rust daemon vs Python service split, data flow diagrams |
| specs/SCHEMAS.md | SQLite table definitions for memories, user styles, and doc tracking |
| .claude/CLAUDE.md | Project rules and AI workflow for Claude Code, spec-driven development guidelines |

---

## Token Budgets

| Prompt ID | Max Input | Max Output |
|-----------|-----------|------------|
| PROMPT-001 (User Scanner) | 4000 | 200 |
| PROMPT-002 (Memory Extractor) | 8000 | 2000 |
| PROMPT-003 (Project Summarizer) | 4000 | 1000 |
| PROMPT-004 (Doc Summarizer) | 8000 | 200 |

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

| Old Prompt | Status |
|------------|--------|
| PROMPT-001-LOG-CLEANER | Replaced by PROMPT-001-USER-SCANNER |
| PROMPT-001-MEMORY-WRITER | Replaced by PROMPT-001 + PROMPT-002 pipeline |
| PROMPT-002-COMPOSE | Removed |
| PROMPT-003-CONFLICT | Removed |
| PROMPT-004-CLI | Removed |
| PROMPT-005-PREFERENCE | Removed |
