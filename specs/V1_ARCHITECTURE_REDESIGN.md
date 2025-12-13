# Squirrel Memory v1/v2 Architecture Redesign

This document captures the canonical v1 design for Squirrel's memory system, plus v2 extension plan.

**Status:** This is the new design. The old lesson/fact/profile + PROMPT-001-A/B + importance/support_count design is deprecated.

---

## 0. Design Principles

### P1: AI-Primary

- One Memory Writer prompt per episode
- No second-stage "manager" prompt
- LLM decides what to remember, where, and at what level (kind/tier/scope/TTL)

### P2: Future-Impact (Immediate + Long-term Benefit)

- When an episode ends, newly written memories must be immediately usable for the next task
- Long-term retention is based on **future opportunities/uses/regret** observed in real coding work
- NOT based on heuristics like importance/support_count

### P3: Declarative

Humans declare:
- **Objectives:** reduce repeated debugging/explanations/rediscovering invariants
- **Constraints:** no secrets/raw logs; some guards/invariants must not auto-disappear
- **Minimal schema + promotion/decay policy**

Memory Writer + CR-Memory implement the behavior inside these constraints.

---

## 1. Data Model (Local SQLite)

### 1.1 memories - Core memory table

```sql
CREATE TABLE memories (
  id          TEXT PRIMARY KEY,
  project_id  TEXT,                     -- repo/project identifier
  scope       TEXT NOT NULL,            -- 'global' | 'project' | 'repo_path'

  owner_type  TEXT NOT NULL,            -- 'user' | 'team' | 'org'
  owner_id    TEXT NOT NULL,            -- user_id / team_id / org_id

  kind        TEXT NOT NULL,            -- 'preference' | 'invariant' | 'pattern' | 'guard' | 'note'
  tier        TEXT NOT NULL,            -- 'short_term' | 'long_term' | 'emergency'
  key         TEXT,                     -- optional: 'project.http.client', 'user.pref.ask_when_uncertain'
  text        TEXT NOT NULL,            -- 1-2 sentence human-readable memory

  status      TEXT NOT NULL,            -- 'provisional' | 'active' | 'deprecated'
  confidence  REAL,                     -- Memory Writer's confidence (0.0-1.0)
  expires_at  TEXT,                     -- nullable
  embedding   BLOB,                     -- sqlite-vec (embedding vector)

  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
```

**Semantics:**

| Field | Values | Meaning |
|-------|--------|---------|
| **kind** | `preference` | User style/interaction preferences |
| | `invariant` | Stable project facts / architectural decisions |
| | `pattern` | Reusable debugging or design patterns |
| | `guard` | Risk rules ("don't do X", "ask before Y") |
| | `note` | Lightweight notes / ideas / hypotheses |
| **tier** | `short_term` | Hot cache, trial memories (short horizon) |
| | `long_term` | Promoted, validated memories (project/user "personality") |
| | `emergency` | High-severity guards/patterns that may affect tool execution |
| **status** | `provisional` | Newly written, not yet validated long-term |
| | `active` | Accepted long-term memory |
| | `deprecated` | No longer used (ignored in retrieval, kept for history) |
| **scope** | `global` | Cross-project, user-level |
| | `project` | Applies to the whole project/repo |
| | `repo_path` | Applies only to a sub-tree (monorepo use case) |

**What we no longer have:**
- No `memory_type` = lesson/fact/profile
- No `outcome`, `importance`, `evidence_source`, `fact_type`
- No `support_count`, `valid_from`, `valid_to`, `superseded_by`

### 1.2 evidence - Where memories came from

```sql
CREATE TABLE evidence (
  id           TEXT PRIMARY KEY,
  memory_id    TEXT NOT NULL,
  episode_id   TEXT NOT NULL,
  source       TEXT,      -- 'failure_then_success' | 'user_correction' | 'explicit_statement'
  frustration  TEXT,      -- 'none' | 'mild' | 'moderate' | 'severe'
  created_at   TEXT NOT NULL,
  FOREIGN KEY (memory_id) REFERENCES memories(id)
);
```

Episode-level signals (success/failure/frustration) stored here, NOT on memories table.

### 1.3 memory_metrics - For CR-Memory

```sql
CREATE TABLE memory_metrics (
  memory_id              TEXT PRIMARY KEY,
  use_count              INTEGER DEFAULT 0,
  opportunities          INTEGER DEFAULT 0,
  suspected_regret_hits  INTEGER DEFAULT 0,
  estimated_regret_saved REAL DEFAULT 0.0,
  last_used_at           TEXT,
  last_evaluated_at      TEXT,
  FOREIGN KEY (memory_id) REFERENCES memories(id)
);
```

| Metric | Meaning |
|--------|---------|
| `use_count` | How many times this memory was actually chosen/injected |
| `opportunities` | How many tasks could reasonably have used this memory |
| `suspected_regret_hits` | How many times it appears to have prevented repeated errors/explanations |
| `estimated_regret_saved` | Continuous metric (v1 keeps this simple) |

### 1.4 episodes - Log timeline

```sql
CREATE TABLE episodes (
  id          TEXT PRIMARY KEY,
  project_id  TEXT NOT NULL,
  start_ts    TEXT NOT NULL,
  end_ts      TEXT NOT NULL,
  events_json TEXT NOT NULL,
  processed   INTEGER DEFAULT 0
);
```

**events_json structure (compressed, NOT raw logs):**

```json
{
  "events": [
    {
      "ts": "2024-12-10T10:00:00Z",
      "role": "user | assistant | tool",
      "kind": "message | tool_call | tool_result",
      "tool_name": "Bash | Edit | Http | null",
      "file": "stripe_client.py | null",
      "summary": "user asked to call Stripe API",
      "raw_snippet": "optional short snippet or truncated content"
    }
  ],
  "stats": {
    "error_count": 3,
    "retry_loops": 2,
    "tests_final_status": "passed | failed | not_run",
    "user_frustration": "none | mild | moderate | severe"
  }
}
```

---

## 2. Online Pipeline - Immediate Benefit

### 2.1 Episode Boundaries & Ingest

Episode flush triggers (NOT just "5-minute idle"):
1. **Task boundary detected:** New user query starts a different task
2. **High-impact event resolved:** Severe frustration + eventual success
3. **User explicit flush:** `sqrl flush`

On flush:
1. Daemon writes `episodes` row
2. Calls Memory Writer (Python agent) with:
   - episode_id + compressed episode content
   - small set of relevant existing memories
   - basic episode features (errors, retries, success, frustration)
   - owner info (user/team/org)

### 2.2 Memory Writer (Single Strong-Model Call)

**The only LLM-based memory decision step.**

**Inputs:**
- Episode summary
- Relevant existing memories (small subset)
- Episode features (error/retry/frustration)
- Policy/constraints (as system prompt text)

**Responsibilities:**
- Decide whether to create any memories at all
- For each memory, decide: scope, owner_type, owner_id, kind, tier, key, text, ttl_days, confidence

**Output JSON:**

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
      "key": null,
      "text": "In this project, requests often hits SSL certificate errors; use httpx as the default HTTP client instead.",
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
      "key": "project.http.client",
      "text": "The standard HTTP client for this project is httpx.",
      "ttl_days": null,
      "confidence": 0.9
    },
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
    }
  ],
  "episode_evidence": {
    "episode_id": "ep-2024-12-10-001",
    "source": "failure_then_success",
    "frustration": "severe"
  }
}
```

**There is NO separate PROMPT-001-B / "Stage 2 manager".**

### 2.3 Commit Layer (Thin CRUD, No LLM)

Pure code:

| Op | Action |
|----|--------|
| **ADD** | Insert into memories with status='provisional', tier as given, expires_at = now + ttl_days. Compute embedding. Insert evidence row. Insert memory_metrics row with zeros. |
| **UPDATE** | Mark previous memory (by key or id) as status='deprecated'. Then ADD the new memory. |
| **DEPRECATE** | Mark target memory as status='deprecated'. |

**Guarantee:** Commit is synchronous. Next `get_task_context` sees new memories immediately.

---

## 3. Retrieval & Output

### 3.1 get_task_context

**Filter:**
- project_id + scope + owner_type/owner_id
- status IN ('provisional', 'active')

**Retrieval (v1):**

| Priority | Method |
|----------|--------|
| Primary | Key-based lookup: `key LIKE 'project.%'`, `key LIKE 'user.%'` |
| Secondary | Embedding search via sqlite-vec (cosine similarity, top-K) |

**Ranking factors:**
- Semantic similarity
- Status: active > provisional
- Tier: emergency/long_term > short_term
- Kind: invariant/preference > pattern > note/guard
- log(1 + use_count) as small boost

**On retrieval:**
- memory_metrics.use_count++
- memory_metrics.opportunities++ (for injected memories)

### 3.2 Guards and Context Injection

Memories with `kind='guard'` and `tier='emergency'` are special:

1. Appear in context to influence assistant reasoning
2. Have `polarity=-1` (anti-patterns, "don't do X" knowledge)

**Important:** Squirrel is passive (log watching). We do NOT intercept tool execution.

**Soft enforcement model:**

| Aspect | Behavior |
|--------|----------|
| How guards work | Injected into context with high priority |
| Who enforces | The AI assistant decides whether to obey |
| What we can't do | Block tool calls, require confirmation |
| What we can do | Strongly inform AI of risks/anti-patterns |

Guards are valuable because they represent learned "don't do X" knowledge. The AI, seeing a guard in context, should avoid the anti-pattern. But we cannot force compliance - we inform, AI decides.

---

## 4. CR-Memory v1 - Opportunity-based Long-term Management

Background job (cron or after N episodes) that:
- **Promotes** useful memories: provisional → active (and tier='long_term')
- **Deprecates** useless ones: provisional/active → deprecated
- **Adjusts** TTLs (optional)

### 4.1 Core Idea

| Phase | Goal |
|-------|------|
| **Short-term** | Memories born from episodes, goal is immediate help |
| **Long-term** | Memory earns long-term status by consistently reducing repeated errors/explanations/rediscovery |

### 4.2 estimated_regret_saved Calculation (v1 Heuristic)

Per project, maintain baseline stats:
- `avg_errors_per_opportunity`
- `avg_retries_per_opportunity`

When memory m is used in an episode:
```
delta_err   = max(0, avg_errors_per_opportunity  - err_actual)
delta_retry = max(0, avg_retries_per_opportunity - retry_actual)

estimated_regret_saved += α * delta_err + β * delta_retry
```

Weights (α=1.0, β=0.5) defined in `memory_policy.toml`.

### 4.3 Policy (Declarative)

`memory_policy.toml`:

```toml
[promotion.default]
min_opportunities = 5
min_use_ratio     = 0.6
min_regret_hits   = 2

[deprecation.default]
min_opportunities = 10
max_use_ratio     = 0.1

[decay.guard]
max_inactive_days = 90

[decay.gotcha]
max_inactive_days = 180

[regret_weights]
alpha_errors  = 1.0
beta_retries  = 0.5
```

### 4.4 CR-Memory v1 Algorithm

```python
for each memory m with status='provisional':
    opp  = metrics.opportunities
    uses = metrics.use_count
    hits = metrics.suspected_regret_hits

    if opp < promotion.min_opportunities:
        continue  # not enough evidence yet

    use_ratio = uses / opp

    if use_ratio >= promotion.min_use_ratio and hits >= promotion.min_regret_hits:
        # Promote to long-term
        status = 'active'
        if kind in ['invariant', 'preference', 'pattern']:
            tier = 'long_term'
        maybe_extend_expires_at(m)

    elif use_ratio <= deprecation.max_use_ratio and opp >= deprecation.min_opportunities:
        # Lots of chances, rarely or never used => discard
        status = 'deprecated'
```

**Important:**
- Time (days) is secondary factor
- Main drivers are opportunities/use_ratio/regret_hits
- CR-Memory v1 uses **heuristics only** (no additional LLM calls)

---

## 5. Model Roles

| Role | Model | When |
|------|-------|------|
| Memory Writer | Strong (cloud) | One call per episode boundary |
| Episode pre-filter | Cheap/rules | Decide if episode deserves Memory Writer call |
| Frustration detection | Cheap/rules | Light sentiment analysis |
| Context composing | Fast/templates | Small reranking (optional) |

**There is no second-stage "manager" prompt.**

---

## 6. v2 Extension Plan

v2 does NOT change v1's memory paradigm. It adds:

### 6.1 Cloud Team/Org Memory Service

- For `owner_type='team'/'org'`, memories managed by cloud service
- Multi-tenant DB for team/org-scoped memories
- APIs: get_task_context (team-level), ingest_episode, CRUD for admin/editor UI
- Local daemon continues to manage user-level memories locally

### 6.2 Multi-CLI / Multi-IDE (Beyond MCP)

Same memory engine, multiple clients:
- MCP adapter for Claude Code (existing)
- VS Code / Cursor / Windsurf / Codex CLI integrations
- Each integration translates its logs into episodes format, calls unified local protocol

---

## Appendix: Scope='repo_path' Clarification

| Field | Example | Meaning |
|-------|---------|---------|
| project_id | `offer-i-monorepo` | Logical project/repo |
| scope | `repo_path` | Memory applies only to sub-tree |
| (metadata) | path_prefix: `services/payment-api` | Sub-tree path |

Use when:
- Monorepo with clearly separated submodules
- Memory applies only to one sub-tree (e.g., payment services, not analytics)

For v1: Default to 'project' unless clear submodule use case detected.
