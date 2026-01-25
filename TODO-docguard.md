# Docguard Feature TODO

Based on discussion about adding doc awareness to Squirrel.

## Summary of New Feature

Squirrel gains doc awareness:
1. **Doc Indexing**: Watch doc files, summarize with LLM, expose tree via MCP
2. **Doc Debt Detection**: Analyze git commits, detect when docs should have been updated
3. **Auto Hook Install**: Daemon detects `.git/` creation, auto-installs hooks
4. **Silent Init**: `sqrl init` creates defaults, user configures in dashboard

---

## Spec Changes Required

All spec changes completed on 2025-01-26.

### specs/ARCHITECTURE.md ✅

| Section | Change | Status |
|---------|--------|--------|
| Design Principles | Added P5: Doc Aware | ✅ |
| System Overview diagram | Add Doc Watcher + Git Watcher + Dashboard | ✅ |
| ARCH-001 (Rust Daemon) | Add: Doc watcher, Git watcher, Hook installer, Dashboard | ✅ |
| ARCH-002 (Python Service) | Add: Doc Summarizer | ✅ |
| ARCH-003 (Storage) | Add: docs_index, doc_debt, config.yaml | ✅ |
| FLOW-001 | Update: 10 min idle timeout | ✅ |
| New FLOW-004 | Add: Doc Indexing flow | ✅ |
| New FLOW-005 | Add: Git Detection and Hook Installation | ✅ |
| New FLOW-006 | Add: Doc Debt Detection flow | ✅ |
| CLI Commands table | Update: silent init, add status, add hidden commands | ✅ |
| Dashboard features | Add: tool config, doc patterns, doc debt view | ✅ |

### specs/INTERFACES.md ✅

| Section | Change | Status |
|---------|--------|--------|
| CLI-002 (sqrl init) | Update: Silent init, creates config.yaml | ✅ |
| New IPC-003 | Add: `summarize_doc` | ✅ |
| New MCP-002 | Add: `squirrel_get_docs_tree` | ✅ |
| New MCP-003 | Add: `squirrel_get_doc_debt` | ✅ |
| Dashboard Pages (UI-002) | Add: Docs page | ✅ |
| Dashboard Endpoints (UI-003) | Add: /api/docs/tree, /api/docs/debt, /api/config | ✅ |
| New CONFIG-001 | Add: `.sqrl/config.yaml` full schema | ✅ |

### specs/SCHEMAS.md ✅

| Section | Change | Status |
|---------|--------|--------|
| New SCHEMA-005 | Add: `docs_index` table | ✅ |
| New SCHEMA-006 | Add: `doc_debt` table with detection rules | ✅ |

### specs/DECISIONS.md ✅

| Section | Change | Status |
|---------|--------|--------|
| New ADR-017 | Doc Awareness Feature | ✅ |
| New ADR-018 | Silent Init | ✅ |
| New ADR-019 | Auto Git Hook Installation | ✅ |

### specs/PROMPTS.md ✅

| Section | Change | Status |
|---------|--------|--------|
| Model Configuration | Add Doc Summarizer row | ✅ |
| New PROMPT-004 | Doc Summarizer prompt | ✅ |
| Token Budgets | Add PROMPT-004 row | ✅ |

### specs/CONSTITUTION.md ✅

| Section | Change | Status |
|---------|--------|--------|
| Core Principles | Add P5: Doc Aware | ✅ |

### .claude/CLAUDE.md ✅

| Section | Change | Status |
|---------|--------|--------|
| Pre-commit hook section | Update: auto-installed hooks, doc debt detection | ✅ |

---

## Implementation Order

| Phase | Task | Depends On | Status |
|-------|------|------------|--------|
| 1 | Update specs with new feature definitions | - | ✅ |
| 2 | Add config schema to Rust daemon (.sqrl/config.yaml) | Phase 1 | ✅ |
| 3 | Add doc file watcher to daemon | Phase 2 | |
| 4 | Add doc summarizer to Python service (PROMPT-004) | Phase 1 | |
| 5 | Add IPC method for doc summarization | Phase 3, 4 | |
| 6 | Add docs_index storage (SCHEMA-005) | Phase 2 | |
| 7 | Add MCP tool: squirrel_get_docs_tree | Phase 5, 6 | |
| 8 | Add git detection + auto hook install | Phase 2 | ✅ |
| 9 | Add post-commit hook logic (internal command) | Phase 8 | |
| 10 | Add doc debt tracking (SCHEMA-006) | Phase 9 | |
| 11 | Add MCP tool: squirrel_get_doc_debt | Phase 10 | |
| 12 | Update dashboard: tools config page | Phase 2 | |
| 13 | Update dashboard: doc patterns config | Phase 2 | |
| 14 | Update sqrl init to be silent | Phase 2 | ✅ |
| 15 | Update sqrl status to show doc debt | Phase 10 | |

---

## Key Design Decisions (Confirmed)

| Decision | Choice |
|----------|--------|
| Hook command visibility | Hidden internal command, not user-facing |
| Hook installation | Auto by daemon when .git/ detected |
| Doc scanning | Config-based: extensions + include/exclude paths |
| Change detection | File watcher (notify) + content hash |
| LLM for docs | Yes, summarize each doc for tree display |
| sqrl init behavior | Silent, create defaults, no questions |
| Tool configuration | User sets in dashboard, not during init |
| Blank project support | Yes, sqrl init works without git/AI tools |

---

## Open Questions

| Question | Status |
|----------|--------|
| Default doc extensions | Proposed: md, mdc, txt, rst |
| Default include paths | Proposed: specs/, docs/, .claude/, .cursor/, root *.md |
| Default exclude paths | Proposed: node_modules/, target/, .git/, vendor/ |
| Doc summary length | Proposed: 1-2 sentences |
| Doc debt detection rules | Need to define: how to map code→docs |

---

## Files to Create/Modify

### New Files
- `daemon/src/watcher/doc_watcher.rs` - Watch doc files for changes
- `daemon/src/storage/docs_index.rs` - Store doc index
- `daemon/src/storage/doc_debt.rs` - Store doc debt
- `daemon/src/cli/hooks.rs` - Internal hook logic
- `src/sqrl/agents/doc_summarizer.py` - LLM doc summarization

### Modified Files
- `daemon/src/main.rs` - Add internal hook command
- `daemon/src/cli/init.rs` - Make silent
- `daemon/src/cli/status.rs` - Show doc debt
- `daemon/src/cli/watch.rs` - Add git detection, doc watching
- `daemon/src/mcp/mod.rs` - Add new MCP tools
- `daemon/src/ipc/types.rs` - Add summarize_doc request/response
- `daemon/src/dashboard/api.rs` - Add config endpoints
- `daemon/src/dashboard/static/index.html` - Add tools/docs config pages
- `src/sqrl/ipc/server.py` - Add summarize_doc handler

---

## Priority

1. **High**: Spec updates (must be done first per project rules)
2. **Medium**: Core feature implementation (doc indexing, MCP exposure)
3. **Lower**: Doc debt detection (can be added after core works)
4. **Lowest**: CI integration (GitHub Action for required checks)
