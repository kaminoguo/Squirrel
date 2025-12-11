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
| specs/KEYS.md | Key conventions (non-binding suggestions) |
| specs/POLICY.md | Memory policy configuration |
| specs/PROMPTS.md | LLM prompts with model tiers (PROMPT-*) |
| specs/DECISIONS.md | Architecture decision records (ADR-*) |

**Rules:**
1. Read specs before implementing
2. Never implement behavior not defined in specs
3. Update specs before or with code, never after
4. Reference spec IDs in commits (e.g., "implements SCHEMA-001")

## AI Workflow

Follow these phases in order. Never skip phases. Never jump to code without a spec.

| Phase | Action | Output |
|-------|--------|--------|
| 1. **Specify** | Define WHAT and WHY. No tech stack yet. | `specs/*.md` updated |
| 2. **Clarify** | Ask questions. Mark unclear areas as `[NEEDS CLARIFICATION: reason]` | Ambiguities resolved |
| 3. **Plan** | Define HOW (tech stack, architecture, file structure) | Implementation plan |
| 4. **Tasks** | Break into ordered, independently testable steps | Task list |
| 5. **Implement** | Execute one task at a time. Test after each. | Working code |

### Before Every Commit (MANDATORY)

A pre-commit hook (`.githooks/pre-commit`) will show you which files changed.

**Your job:** Review the list and decide which docs need updates.

| Files Changed | Check These Docs |
|---------------|------------------|
| `*.rs` (Rust) | `specs/ARCHITECTURE.md`, `specs/INTERFACES.md`, `specs/SCHEMAS.md` |
| `*.py` (Python) | `specs/ARCHITECTURE.md`, `specs/PROMPTS.md` |
| `specs/*.md` | Related code that implements the spec |
| `*.toml`, `*.nix` | `specs/DECISIONS.md` (if config change is significant) |

**Checklist:**

| Check | Action if Yes |
|-------|---------------|
| Code changed? | Update related spec |
| Spec changed? | Update related code |
| New key/schema/interface? | Add to registry with ID |
| Unclear requirement? | Mark `[NEEDS CLARIFICATION]`, ask user |

## Project Rules

See `.cursor/rules/*.mdc` for context-specific rules:
- `general.mdc` - Overall development rules
- `rust-daemon.mdc` - Rust daemon boundaries (ARCH-001)
- `python-agent.mdc` - Python agent boundaries (ARCH-002)
- `specs.mdc` - Specification maintenance
- `testing.mdc` - Testing requirements (DR4)

## Architecture

```
Rust Daemon (I/O, storage, MCP) <--IPC--> Python Memory Service (LLM operations)
```

| Component | Responsibility | Never Does |
|-----------|----------------|------------|
| Rust Daemon | Log watching, MCP server, CLI, SQLite, guard interception | LLM calls |
| Python Memory Service | Memory Writer, embeddings, CR-Memory evaluation | File watching, direct DB access |

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
- Today's date: 2025 Dec 10

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
