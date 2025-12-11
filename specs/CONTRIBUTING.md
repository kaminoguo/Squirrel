# Squirrel Contributing Guide

Collaboration standards for contributors. This document complements CONSTITUTION.md with practical workflow details.

## Contributor Workflow

### Branch Strategy

| Branch | Purpose | Protection |
|--------|---------|------------|
| `main` | Stable release-ready code | Protected, requires PR |
| `<name>/<type>-<desc>` | Feature/fix branches | None |

**Branch naming examples:**
- `adrian/feat-log-watcher`
- `lyrica/fix-ipc-timeout`
- `adrian/docs-contributing`

### Pull Request Process

#### PR-001: Standard PR Flow

```
1. Create branch from main
2. Make changes (follow spec-driven development)
3. Run local checks: `fmt` + `lint` + `test-all`
4. Push branch
5. Create PR with template
6. CI runs automatically
7. Request review
8. Address feedback
9. Squash merge to main
```

#### PR-002: PR Template

All PRs must include:

```markdown
## Summary
Brief description of changes (1-2 sentences)

## Spec References
- Implements: SCHEMA-001, IPC-002 (if applicable)
- Updates: specs/ARCHITECTURE.md (if spec changed)

## Type
- [ ] feat: New feature
- [ ] fix: Bug fix
- [ ] docs: Documentation only
- [ ] refactor: Code refactoring
- [ ] test: Test additions
- [ ] chore: Build/tooling changes

## Checklist
- [ ] Specs updated (if behavior changed)
- [ ] Tests added/updated
- [ ] `fmt` passes
- [ ] `lint` passes
- [ ] `test-all` passes

## Test Plan
How to verify this change works.
```

#### PR-003: Review Requirements

| Change Type | Required Reviewers | Auto-merge |
|-------------|-------------------|------------|
| docs only | 1 | Yes (after CI) |
| code (non-breaking) | 1 | No |
| spec changes | 2 | No |
| breaking changes | 2 + explicit approval | No |

### Commit Standards

#### COMMIT-001: Message Format

```
<type>(<scope>): <description>

[optional body]

[optional footer: refs #issue, implements SPEC-ID]
```

**Types:**
| Type | Description |
|------|-------------|
| feat | New feature |
| fix | Bug fix |
| docs | Documentation |
| refactor | Code refactoring (no behavior change) |
| test | Test additions/fixes |
| chore | Build, CI, tooling |
| perf | Performance improvement |

**Scopes:**
| Scope | Description |
|-------|-------------|
| daemon | Rust daemon code |
| agent | Python agent code |
| cli | CLI interface |
| mcp | MCP server |
| ipc | IPC protocol |
| specs | Specification files |
| ci | CI/CD configuration |

**Examples:**
```
feat(daemon): implement Claude Code log watcher

Implements FLOW-001 passive ingestion for Claude Code JSONL files.
Uses notify crate for cross-platform file watching.

Implements: ARCH-001
Refs: #12
```

```
fix(agent): handle empty episode gracefully

Return early with empty result instead of raising exception
when episode has no events.

Fixes: #34
```

#### COMMIT-002: Commit Hygiene

| Rule | Description |
|------|-------------|
| Atomic commits | One logical change per commit |
| Passing state | Each commit should pass CI |
| No WIP commits | Squash before PR, no "WIP", "fixup" |
| Reference specs | Include spec IDs when implementing specs |

### Code Review Guidelines

#### REVIEW-001: Reviewer Checklist

| Check | Question |
|-------|----------|
| Spec alignment | Does code match spec? If no spec, should there be one? |
| Boundary respect | Rust doing I/O only? Python doing LLM only? |
| Error handling | Graceful degradation? No panics in daemon? |
| Tests | New behavior covered? Edge cases? |
| Security | No secrets? Input validated? |
| Performance | Acceptable latency? No unnecessary allocations? |

#### REVIEW-002: Review Etiquette

| Do | Don't |
|----|-------|
| Be specific and actionable | Vague criticism |
| Suggest alternatives | Just say "this is wrong" |
| Approve when ready | Block on nitpicks |
| Use "nit:" prefix for optional | Demand perfection |

### Issue Management

#### ISSUE-001: Issue Labels

| Label | Description | Color |
|-------|-------------|-------|
| `bug` | Something broken | Red |
| `feat` | Feature request | Green |
| `docs` | Documentation | Blue |
| `good-first-issue` | Beginner friendly | Purple |
| `help-wanted` | Needs contributor | Yellow |
| `blocked` | Waiting on something | Orange |
| `wontfix` | Not planned | Gray |

#### ISSUE-002: Issue Template

```markdown
## Description
Clear description of the issue or feature request.

## Current Behavior (for bugs)
What happens now.

## Expected Behavior
What should happen.

## Reproduction Steps (for bugs)
1. Step one
2. Step two

## Environment
- OS:
- Squirrel version:
- CLI being used:

## Spec Reference (if applicable)
Related spec IDs: SCHEMA-001, IPC-002
```

## Security

### SEC-001: Secrets Handling

| Rule | Enforcement |
|------|-------------|
| No secrets in code | Pre-commit hook check |
| No secrets in commits | CI secret scanning |
| API keys via env vars | Documented in README |
| `.env` files gitignored | In .gitignore |

**Forbidden patterns in commits:**
- API keys (sk-*, anthropic-*, etc.)
- Private keys
- Passwords
- Connection strings with credentials

### SEC-002: Dependency Security

| Check | Frequency | Tool |
|-------|-----------|------|
| Rust advisories | Every CI run | `cargo audit` |
| Python vulnerabilities | Every CI run | `pip-audit` |
| Dependabot alerts | Automated | GitHub Dependabot |

## Release Process

### RELEASE-001: Versioning

Follow [Semantic Versioning](https://semver.org/):

```
MAJOR.MINOR.PATCH

MAJOR: Breaking changes
MINOR: New features (backward compatible)
PATCH: Bug fixes (backward compatible)
```

### RELEASE-002: Release Checklist

```
1. [ ] All CI passing on main
2. [ ] CHANGELOG.md updated
3. [ ] Version bumped in Cargo.toml and pyproject.toml
4. [ ] Tag created: v{version}
5. [ ] GitHub Release created with notes
6. [ ] Binaries built and attached (via CI)
7. [ ] Homebrew formula updated (if applicable)
```

## Communication

### COMM-001: Channels

| Channel | Purpose |
|---------|---------|
| GitHub Issues | Bugs, feature requests |
| GitHub Discussions | Questions, ideas, RFC |
| Pull Requests | Code review |

### COMM-002: Response Times

| Type | Target Response |
|------|-----------------|
| Security issues | 24 hours |
| Bug reports | 48 hours |
| Feature requests | 1 week |
| PR reviews | 48 hours |
