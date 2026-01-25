# Squirrel Decisions

Architecture Decision Records (ADR) for significant choices.

## ADR Format

Each decision follows:
- **ID**: ADR-NNN
- **Status**: proposed | accepted | deprecated | superseded
- **Date**: YYYY-MM-DD
- **Context**: Why this decision was needed
- **Decision**: What was decided
- **Consequences**: Trade-offs accepted

---

## ADR-001: Rust + Python Split Architecture

**Status:** accepted
**Date:** 2024-11-20

**Context:**
Need a system that watches files, serves MCP, and runs LLM operations. Single language options:
- Pure Rust: LLM libraries immature, PydanticAI not available
- Pure Python: File watching unreliable, MCP SDK less mature

**Decision:**
Split into Rust daemon (I/O, storage, MCP) and Python Memory Service (LLM operations). Communicate via JSON-RPC over Unix socket.

**Consequences:**
- (+) Best libraries for each domain
- (+) Clear separation of concerns
- (+) Daemon can run without Python for basic ops
- (-) IPC overhead (~1ms per call)
- (-) Two deployment artifacts
- (-) More complex build process

---

## ADR-002: SQLite with sqlite-vec

**Status:** accepted
**Date:** 2024-11-20

**Context:**
Need vector storage for embeddings. Options:
- PostgreSQL + pgvector: Requires server, overkill for local
- Pinecone/Weaviate: Cloud dependency, violates local-first
- SQLite + sqlite-vec: Local, single file, good enough performance
- ChromaDB: Python-only, can't use from Rust daemon

**Decision:**
Use SQLite with sqlite-vec extension for all storage including vectors.

**Consequences:**
- (+) Single file database, easy backup
- (+) No server process needed
- (+) Works from both Rust and Python
- (+) sqlite-vec handles cosine similarity efficiently
- (-) Limited to ~100k vectors before slowdown
- (-) No built-in sharding

---

## ADR-003: PydanticAI for Agent Framework

**Status:** accepted
**Date:** 2024-11-23

**Context:**
Need structured LLM outputs for memory extraction. Options:
- Raw API calls: No validation, manual JSON parsing
- LangChain: Heavy, over-engineered for our needs
- PydanticAI: Lightweight, Pydantic validation, good typing

**Decision:**
Use PydanticAI for all agent implementations.

**Consequences:**
- (+) Structured outputs with validation
- (+) Type safety
- (+) Clean agent patterns
- (+) Active development
- (-) Newer library, less ecosystem
- (-) Anthropic-focused (but supports OpenAI)

---

## ADR-004: 2-Tier Model Pipeline

**Status:** accepted
**Date:** 2024-11-23
**Updated:** 2025-01-20

**Context:**
Different operations have different accuracy/cost requirements:
- Log cleaning: Simple task, needs to be cheap
- Memory extraction: Core intelligence, needs quality

**Decision:**
Use two model tiers:
- `cheap_model`: Log Cleaner (default: Claude Haiku)
- `strong_model`: Memory Extractor (default: Claude Sonnet)

**Consequences:**
- (+) Cost optimization (cheap model for filtering)
- (+) Quality where it matters (strong model for extraction)
- (+) Provider flexibility via LiteLLM
- (-) Need to maintain two prompts
- (-) Configuration complexity

---

## ADR-006: Nix/devenv for Development Environment

**Status:** accepted
**Date:** 2024-11-23

**Context:**
Team uses Windows/WSL2/Mac. Need reproducible dev environment:
- Docker: Heavy, doesn't integrate well with IDEs
- Manual setup: Drift, "works on my machine"
- Nix/devenv: Declarative, reproducible, integrates with shells

**Decision:**
Use devenv.nix for development environment. All tools (Rust, Python, SQLite) managed by Nix.

**Consequences:**
- (+) Reproducible across all platforms (via WSL2 on Windows)
- (+) Single `devenv shell` command for setup
- (+) Declarative, matches spec-driven approach
- (+) No global installs needed
- (-) Nix learning curve
- (-) Windows requires WSL2

---

## ADR-007: Spec-Driven Development

**Status:** accepted
**Date:** 2024-11-23

**Context:**
Project is 98% AI-coded. Need to ensure consistency and quality:
- Ad-hoc development: AI generates inconsistent code
- Heavy process: Slows down iteration
- Spec-driven: Specs are source of truth, AI follows specs

**Decision:**
Adopt spec-driven development. All behavior defined in specs/ before implementation. Code is "compiled output" of specs.

**Consequences:**
- (+) AI has clear instructions
- (+) Consistent implementation
- (+) Documentation always current
- (+) Easy to review (review specs, not code)
- (-) More upfront work on specs
- (-) Need discipline to update specs first

---

## ADR-009: Unix Socket for IPC

**Status:** accepted
**Date:** 2024-11-20

**Context:**
Daemon and Memory Service need to communicate. Options:
- HTTP: Works but overhead for local
- gRPC: Complex setup for simple RPC
- Unix socket: Fast, secure, local-only

**Decision:**
Use Unix socket at `/tmp/sqrl.sock` with JSON-RPC 2.0 protocol. Windows uses named pipes.

**Consequences:**
- (+) No network exposure
- (+) Low latency (<1ms)
- (+) Simple protocol
- (-) Platform-specific paths
- (-) Need to handle socket cleanup

---

## ADR-012: Simplified Memory Architecture (v1 Redesign)

**Status:** accepted
**Date:** 2025-01-20

**Context:**
The original memory architecture was too complex:
- CR-Memory with opportunities, regret_hits, promotion/deprecation logic
- 5 memory kinds (preference, invariant, pattern, guard, note)
- 3 tiers (short_term, long_term, emergency)
- Guard interception for tool blocking
- Complex status lifecycle (provisional → active → deprecated)
- Declarative keys for conflict resolution

This complexity was premature. We needed to simplify for v1.

**Decision:**
Adopt simplified architecture with two memory types:

| Type | Storage | Access | Purpose |
|------|---------|--------|---------|
| User Style | `~/.sqrl/user_style.db` | Synced to agent.md files | Personal development preferences |
| Project Memory | `<repo>/.sqrl/memory.db` | MCP tool | Project-specific knowledge |

Key simplifications:

| Old | New |
|-----|-----|
| CR-Memory evaluation | Simple use_count ordering |
| 5 kinds, 3 tiers | No kinds/tiers, just text |
| Guard interception | Removed (v2 maybe) |
| Declarative keys | Removed |
| Complex status lifecycle | No status field |
| Context composition | Removed (return all memories) |

**Memory retrieval:**
- MCP tool returns ALL project memories grouped by category
- Ordered by use_count DESC within each category
- No semantic search, no filtering

**Model pipeline:**
1. Log Cleaner (cheap model) - compress, decide if worth processing
2. Memory Extractor (strong model) - extract user styles + project memories

**Consequences:**
- (+) Much simpler to implement and maintain
- (+) Easier to understand and debug
- (+) Less API cost (no complex evaluation)
- (+) Faster cold start (no bootstrapping period)
- (-) No intelligent context selection
- (-) May include irrelevant memories
- (-) No guard protection against dangerous actions

**Supersedes:**
- ADR-005: Declarative Keys (removed)
- ADR-008: Frustration Detection (removed)
- ADR-010: AI-Primary Memory Architecture (replaced by this simpler version)
- ADR-011: Historical Timeline (no longer needed without CR-Memory)

---

## ADR-013: B2B Focus with B2D Open Source

**Status:** accepted
**Date:** 2025-01-20

**Context:**
Need to determine business model and feature prioritization.

**Decision:**
B2B (Business to Business) is the main focus. B2D (Business to Developer) is the open source layer.

| Tier | Features | Monetization |
|------|----------|--------------|
| Free (B2D) | Local memory extraction, MCP access, Dashboard | Open source |
| Team (B2B) | Team style sync, shared project memory, analytics | Cloud subscription |

Team features require cloud service for sync and management.

**Consequences:**
- (+) Clear monetization path
- (+) Open source builds community
- (+) Team features justify cloud service
- (-) Need to build and maintain cloud infrastructure
- (-) Must ensure free tier is genuinely useful

---

## ADR-014: Minimal CLI Commands

**Status:** accepted
**Date:** 2025-01-20
**Updated:** 2025-01-25

**Context:**
Previous design had many CLI commands (search, forget, export, flush, policy). With Dashboard for editing, most CLI commands are unnecessary.

**Decision:**
Reduce CLI to essential commands:

| Command | Purpose |
|---------|---------|
| `sqrl` | Show help |
| `sqrl init [--no-history]` | Initialize project (30 days history by default) |
| `sqrl on` | Enable watcher daemon |
| `sqrl off` | Disable watcher daemon |
| `sqrl goaway [-f]` | Remove all Squirrel data |
| `sqrl config` | Open Dashboard in browser |

All other operations (edit, delete, search) happen in Dashboard.

**Consequences:**
- (+) Simpler CLI, less code to maintain
- (+) Dashboard provides better UX for editing
- (+) Consistent experience across operations
- (+) Clear daemon control via on/off
- (-) No quick command-line memory search
- (-) Requires browser for management

---

## ADR-015: Category-Based Project Memory Organization

**Status:** accepted
**Date:** 2025-01-20

**Context:**
Project memories need organization. Options:
- Flat list: Simple but hard to navigate
- By role (engineering, product, design): Overlapping concerns
- By project composition (frontend, backend, etc.): Natural fit

**Decision:**
Organize project memories by 4 default categories:

| Category | Description |
|----------|-------------|
| frontend | UI framework, components, styling |
| backend | API, database, services |
| docs_test | Documentation, testing |
| other | Everything else |

Users can add custom subcategories via Dashboard.

**Consequences:**
- (+) Natural organization for most projects
- (+) Simple to understand and use
- (+) MCP can return all grouped by category
- (-) May not fit all project types
- (-) Need subcategories for complex projects

---

## ADR-016: System Service for Daemon

**Status:** accepted
**Date:** 2025-01-25

**Context:**
The watcher daemon needs to run persistently in the background. Options:
- Foreground process: User must keep terminal open
- PID file management: Manual start/stop, doesn't survive reboot
- System service: Managed by OS, survives reboot, auto-restart

**Decision:**
Use platform-native system services:

| Platform | Service |
|----------|---------|
| Linux | systemd user service (`~/.config/systemd/user/dev.sqrl.daemon.service`) |
| macOS | launchd agent (`~/Library/LaunchAgents/dev.sqrl.daemon.plist`) |
| Windows | Task Scheduler (runs at logon) |

Hidden command `sqrl watch-daemon` runs the actual watcher loop. System service calls this command.

**Consequences:**
- (+) Survives reboots and terminal closures
- (+) Auto-restart on failure
- (+) Platform-native management (systemctl, launchctl)
- (+) No manual daemon management for users
- (-) Platform-specific code paths
- (-) Requires service installation on first init

---

## ADR-017: Doc Awareness Feature

**Status:** accepted
**Date:** 2025-01-26

**Context:**
AI tools often forget to update documentation when code changes. Projects have many docs (specs, README, etc.) that drift from reality. Need a way to:
1. Make docs discoverable to AI tools
2. Detect when docs are stale
3. Remind AI to update docs

**Decision:**
Add doc awareness to Squirrel:

| Component | Purpose |
|-----------|---------|
| Doc Watcher | Watch doc files for changes (create/modify/rename) |
| Doc Indexer | Store doc tree with LLM summaries |
| Doc Debt Tracker | Detect when code changes but related docs don't |
| MCP Tools | Expose doc tree and debt status to AI tools |

Detection rules (priority order):
1. User config mappings (highest priority, eliminates false positives)
2. Reference-based (code contains SCHEMA-001 → SCHEMAS.md)
3. Pattern-based (*.rs → ARCHITECTURE.md)

**Consequences:**
- (+) AI tools can see project doc structure
- (+) Doc staleness is tracked and visible
- (+) Deterministic detection (no LLM for debt detection)
- (+) User can configure mappings to reduce false positives
- (-) Adds complexity to daemon
- (-) Requires LLM calls for doc summarization

---

## ADR-018: Silent Init

**Status:** accepted
**Date:** 2025-01-26

**Context:**
Previous design had `sqrl init` ask questions about which tools to use. This adds friction and confusion.

**Decision:**
Make `sqrl init` completely silent:
- Creates `.sqrl/` with default config
- No prompts, no questions
- User configures tools and patterns in dashboard (`sqrl config`)

**Consequences:**
- (+) Zero friction init
- (+) Works for any project (even blank)
- (+) Configuration is visual and discoverable in dashboard
- (-) User must open dashboard to configure

---

## ADR-019: Auto Git Hook Installation

**Status:** accepted
**Date:** 2025-01-26

**Context:**
Git hooks are needed for doc debt detection, but:
- Manual hook installation is friction
- Users forget to install hooks
- Different git paths (IDEs, wrappers) may not trigger hooks

**Decision:**
Daemon auto-installs hooks when `.git/` detected:

```
Daemon running
    ↓ watches project directory
.git/ created (user runs git init)
    ↓ daemon detects
Auto-install hooks to .git/hooks/
    ↓
post-commit: calls sqrl _internal docguard-record
pre-push: calls sqrl _internal docguard-check (optional block)
```

Hooks are hidden internal commands, not user-facing.

**Consequences:**
- (+) Zero friction hook setup
- (+) Works even if user inits git after sqrl
- (+) No manual hook management
- (-) Hooks can still be bypassed (--no-verify)
- (-) True enforcement requires CI (GitHub required checks)

---

## Deprecated ADRs

| ADR | Status | Reason |
|-----|--------|--------|
| ADR-005 | superseded | Declarative keys removed in ADR-012 |
| ADR-008 | superseded | Frustration detection removed in ADR-012 |
| ADR-010 | superseded | Replaced by simpler ADR-012 |
| ADR-011 | superseded | No longer needed without CR-Memory |

---

## Pending Decisions

| Topic | Options | Blocking |
|-------|---------|----------|
| Team sync backend | Supabase / Custom / None | v2 |
| Local LLM support | Ollama / llama.cpp / None | v2 |
| Dashboard hosting | Local / Cloud / Hybrid | v1 |
