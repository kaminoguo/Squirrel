# Squirrel Constitution

Project governance and principles for AI agents working on this codebase.

## Project Identity

- **Name**: Squirrel
- **Purpose**: Local-first memory system for AI coding tools
- **License**: AGPL-3.0

## Core Principles

### P1: Local-First
All user data stays on their machine by default. No cloud dependency for core functionality. Privacy is non-negotiable.

### P2: Passive Learning
100% invisible during coding sessions. No prompts, no confirmations, no interruptions. Watch logs silently, learn passively.

### P3: Spec-Driven Development
Specs are source of truth. Code is generated output. Never introduce behavior not defined in specs. Update specs before or with code, never after.

### P4: Cross-Platform
Support Mac, Linux, Windows from v1. No OS-specific hacks in code. Platform differences documented in specs.

### P5: AI-Friendly Documentation
All specs written for AI consumption: tables over prose, explicit schemas, stable IDs, no ambiguity.

## Architecture Boundaries

| Component | Language | Responsibility | Boundary |
|-----------|----------|----------------|----------|
| Rust Daemon | Rust | Log watching, storage, MCP server, CLI shell | Never contains LLM logic |
| Python Agent | Python | All LLM operations, memory extraction, retrieval | Never does file watching |
| IPC | JSON-RPC 2.0 | Communication between daemon and agent | Unix socket only |

## Technology Constraints

| Category | Choice | Locked? |
|----------|--------|---------|
| Storage | SQLite + sqlite-vec | Yes (v1) |
| MCP SDK | rmcp (Rust) | Yes (v1) |
| Agent Framework | PydanticAI | Yes (v1) |
| Embeddings | API-based (OpenAI default) | Provider swappable |
| LLM | 2-tier (strong + fast) | Provider swappable |

## Development Rules

### DR1: Spec IDs Required
Every schema, interface, prompt, and key must have a stable ID (e.g., `SCHEMA-001`, `IPC-001`). PRs must reference spec IDs.

### DR2: No Implicit Behavior
If behavior isn't in specs, it doesn't exist. No "obvious" defaults. Document everything.

### DR3: Environment via devenv
All tools managed by devenv.nix. No global installs. `devenv shell` is the only setup command.

### DR4: Test Before Merge
All code changes require passing tests. No exceptions.

### DR5: Minimal Changes
Only change what's necessary. No drive-by refactoring. No "while I'm here" improvements.

## Decision Authority

| Decision Type | Authority |
|---------------|-----------|
| Spec changes | Team consensus (PR approval) |
| Architecture changes | Documented in DECISIONS.md first |
| Dependency additions | Must justify in PR |
| Breaking changes | Major version bump required |

## Communication Style

- English only in code, comments, commits, specs
- No emojis in documentation
- Brief, direct language
- Tables over paragraphs
