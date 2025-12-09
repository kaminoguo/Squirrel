# Squirrel Project

Local-first memory system for AI coding tools.

## AI-First Development

**This project is 98% AI-coded.** You are the primary developer.

All documentation, specs, and structures are designed for AI comprehension. Use declarative thinking: specs define WHAT, you implement HOW.

| Principle | Meaning |
|-----------|---------|
| Specs are source of truth | Never implement undefined behavior |
| Declarative over imperative | Define outcomes, not steps |
| Tables over prose | Structured data > paragraphs |
| Stable IDs everywhere | SCHEMA-001, IPC-001, ADR-001 |
| Update specs first | Specs change before code changes |

## Spec-Driven Development

**Specs are the source of truth. Code is compiled output.**

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
1. Read specs before implementing
2. Never implement behavior not defined in specs
3. Update specs before or with code, never after
4. Reference spec IDs in commits (e.g., "implements SCHEMA-001")

## Project Rules

See `project-rules/*.mdc` for context-specific rules:
- `general.mdc` - Overall development rules
- `rust-daemon.mdc` - Rust daemon boundaries (ARCH-001)
- `python-agent.mdc` - Python agent boundaries (ARCH-002)
- `specs.mdc` - Specification maintenance
- `testing.mdc` - Testing requirements (DR4)

## Architecture

```
Rust Daemon (I/O, storage, MCP) <--IPC--> Python Agent (LLM operations)
```

| Component | Responsibility | Never Does |
|-----------|----------------|------------|
| Rust Daemon | Log watching, MCP server, CLI, SQLite | LLM calls |
| Python Agent | Memory extraction, context composition | File watching |

## Development Environment

Uses Nix via devenv (ADR-006). Single command:

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
- Today's date: 2025 Dec 9

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
