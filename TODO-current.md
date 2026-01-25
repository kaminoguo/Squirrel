# Current TODO (Pre-Docguard)

Existing bugs and incomplete features from previous session.

---

## Bugs

### BUG-001: Deduplication Not Working

**Location:** `daemon/src/cli/watch.rs:138-140`

**Problem:** `send_to_service` passes empty arrays for `existing_user_styles` and `existing_project_memories`. Python service cannot deduplicate.

```rust
let request = ProcessEpisodeRequest {
    // ...
    // TODO: Fetch existing styles and memories from storage
    existing_user_styles: vec![],
    existing_project_memories: vec![],
};
```

**Fix:** Query SQLite storage before sending to Python service.

**Priority:** High (causes duplicate memories)

---

### BUG-002: Pre-commit Hook Not Installed

**Location:** `.githooks/pre-commit` exists but `core.hooksPath` = `.git/hooks`

**Problem:** Custom doc sync reminder hook in `.githooks/pre-commit` is never executed. Active hook is pre-commit framework (ruff + rustfmt only).

**Fix:** Either:
1. Set `core.hooksPath` to `.githooks/`
2. Or merge doc sync logic into pre-commit framework

**Priority:** Medium (addressed by docguard feature later)

---

## Incomplete Features

### INCOMPLETE-001: Dashboard Storage Operations

**Location:** `daemon/src/dashboard/api.rs`

**Problem:** API handlers exist but may not have full CRUD for:
- Add user style (needs storage.add_style)
- Delete user style (needs storage.delete_style)
- Add project memory
- Delete project memory

**Fix:** Implement storage operations in `daemon/src/storage/` and wire to API.

---

### INCOMPLETE-002: Project Listing

**Location:** `daemon/src/dashboard/api.rs` - `list_projects` handler

**Problem:** Need to scan for `.sqrl/` directories to list initialized projects.

**Fix:** Implement project discovery logic.

---

### INCOMPLETE-003: Idle Timeout Updated But Not Tested

**Location:** `daemon/src/watcher/session_tracker.rs`

**Change:** Idle timeout changed from 30 min to 10 min.

**Status:** Code updated, test updated, but not verified in real usage.

---

## Order of Priority

| Priority | Item | Reason |
|----------|------|--------|
| 1 | BUG-001 | Critical - causes duplicate memories |
| 2 | INCOMPLETE-001 | Dashboard non-functional without it |
| 3 | INCOMPLETE-002 | Dashboard incomplete without project list |
| 4 | BUG-002 | Will be addressed by docguard feature |

---

## Decision: Fix Current Bugs vs Start Docguard

Options:
1. Fix BUG-001 and INCOMPLETE-001/002 first, then start docguard
2. Start docguard specs first, fix bugs during implementation
3. Ask user preference

Recommendation: Fix BUG-001 first (critical), then proceed to docguard specs.
