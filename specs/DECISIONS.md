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
Split into Rust daemon (I/O, storage, MCP) and Python agent (LLM operations). Communicate via JSON-RPC over Unix socket.

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

## ADR-004: 2-Tier LLM Strategy

**Status:** accepted
**Date:** 2024-11-23

**Context:**
Different operations have different accuracy/speed requirements:
- Memory extraction: Needs quality, can be slow
- Context composition: Needs speed, simpler task

**Decision:**
Use two model tiers:
- `strong_model`: Complex extraction (default: Claude Sonnet)
- `fast_model`: Quick operations (default: Claude Haiku)

**Consequences:**
- (+) Cost optimization
- (+) Latency optimization for simple tasks
- (+) Provider flexibility
- (-) Configuration complexity
- (-) Need to maintain two sets of prompts

---

## ADR-005: Declarative Keys for Facts

**Status:** accepted
**Date:** 2024-11-23

**Context:**
Facts like "project uses PostgreSQL" can change. Need conflict detection:
- Pure semantic: LLM compares all facts, expensive
- Pure rule-based: Miss semantic conflicts
- Hybrid: Declarative keys for common facts, LLM for rest

**Decision:**
Use declarative keys (e.g., `project.db.engine`) for deterministic conflict resolution. Fall back to LLM for unkeyed facts.

**Consequences:**
- (+) Fast conflict detection for common facts
- (+) No LLM call for key-value changes
- (+) Predictable behavior
- (-) Need to maintain key registry
- (-) Some facts don't fit key-value model

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

## ADR-008: Frustration Detection for Memory Importance

**Status:** accepted
**Date:** 2024-11-23

**Context:**
Some memories are more important than others. User frustration signals high-value learning:
- Swearing after bug fix: Important lesson
- "Finally!" after struggle: Key breakthrough
- Neutral tone: Standard importance

**Decision:**
Detect frustration signals in episodes and boost memory importance accordingly.

| Frustration Level | Signals | Importance Boost |
|-------------------|---------|------------------|
| severe | swearing, rage | critical |
| moderate | "finally", "ugh", 3+ retries | high |
| mild | sigh, minor complaint | medium |
| none | neutral | based on content |

**Consequences:**
- (+) Better prioritization of valuable memories
- (+) Learns from user pain points
- (+) Passive, no user action needed
- (-) May misinterpret sarcasm/humor
- (-) Cultural differences in expression

---

## ADR-009: Unix Socket for IPC

**Status:** accepted
**Date:** 2024-11-20

**Context:**
Daemon and agent need to communicate. Options:
- HTTP: Works but overhead for local
- gRPC: Complex setup for simple RPC
- Unix socket: Fast, secure, local-only

**Decision:**
Use Unix socket at `/tmp/sqrl_agent.sock` with JSON-RPC 2.0 protocol. Windows uses named pipes.

**Consequences:**
- (+) No network exposure
- (+) Low latency (<1ms)
- (+) Simple protocol
- (-) Platform-specific paths
- (-) Need to handle socket cleanup

---

## Pending Decisions

| Topic | Options | Blocking |
|-------|---------|----------|
| Team sync backend | Supabase / Custom / None | v2 |
| Local LLM support | Ollama / llama.cpp / None | v2 |
| Web UI | None / Tauri / Electron | v2 |
