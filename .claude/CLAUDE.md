# Squirrel Project

Local-first memory system for AI coding tools.

## Spec-Driven Development

This project uses spec-driven development. **Specs are the source of truth.**

| Spec File | Purpose |
|-----------|---------|
| specs/CONSTITUTION.md | Project governance, core principles |
| specs/ARCHITECTURE.md | System boundaries, data flow |
| specs/SCHEMAS.md | Database schemas (SCHEMA-*) |
| specs/INTERFACES.md | IPC, MCP, CLI contracts (IPC-*, MCP-*, CLI-*) |
| specs/KEYS.md | Declarative key registry (KEY-*) |
| specs/PROMPTS.md | LLM prompts with model tiers (PROMPT-*) |
| specs/DECISIONS.md | Architecture decision records (ADR-*) |

**Rules:**
1. Never implement behavior not defined in specs
2. Update specs before or with code, never after
3. Reference spec IDs in commits (e.g., "implements SCHEMA-001")

## Project Rules

See `project-rules/*.mdc` for context-specific rules:
- `general.mdc` - Overall development rules
- `rust-daemon.mdc` - Rust daemon boundaries
- `python-agent.mdc` - Python agent boundaries
- `specs.mdc` - Specification maintenance
- `testing.mdc` - Testing requirements

## Architecture

```
Rust Daemon (I/O, storage, MCP) <--IPC--> Python Agent (LLM operations)
```

| Component | Responsibility |
|-----------|----------------|
| Rust Daemon | Log watching, MCP server, CLI, SQLite storage |
| Python Agent | Memory extraction, context composition, conflict detection |

See specs/ARCHITECTURE.md for details.

## Development Environment

Uses Nix via devenv. Single command setup:

```bash
devenv shell
```

Available commands:
- `test-all` - Run all tests
- `dev-daemon` - Start daemon in dev mode
- `fmt` - Format all code
- `lint` - Lint all code

## Team Standards

### Communication
- No unnecessary emojis
- Documentation written for AI comprehension
- English only in code, comments, commits
- Brief, direct language

### Git Workflow

Branch: `yourname/type-description`
- `feat`, `fix`, `refactor`, `docs`, `test`, `chore`

Commit: `type(scope): brief description`
- Reference spec IDs when applicable

### Code Quality
- Write tests for new features (DR4)
- Keep files under 200 lines
- Only change what's necessary (DR5)
- No drive-by refactoring

### Security
- Never commit secrets
- Validate user input
- Review AI-generated code
