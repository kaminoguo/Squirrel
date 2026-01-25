# Current TODO (Pre-Docguard)

Existing bugs and incomplete features from previous session.

---

## Bugs

### BUG-001: Deduplication Not Working - ✅ FIXED

**Location:** `daemon/src/cli/watch.rs`

**Fix:** Added `fetch_existing_user_styles()` and `fetch_existing_project_memories()` functions that query SQLite storage before sending to Python service.

---

### BUG-002: Pre-commit Hook Not Installed - ✅ ADDRESSED

Now using auto-installed hooks in `.git/hooks/` instead of `.githooks/`.
Hooks are installed automatically by `sqrl init` when git is detected.

---

## Incomplete Features

### INCOMPLETE-001: Dashboard Storage Operations - ✅ FIXED

Added to `storage/mod.rs`:
- `add_user_style(text)`
- `delete_user_style(id)`
- `add_project_memory(project_root, category, subcategory, text)`
- `delete_project_memory(project_root, id)`

Wired to API handlers.

---

### INCOMPLETE-002: Project Listing - ✅ FIXED

Added `discover_projects()` function that scans:
- Current directory
- ~/projects, ~/dev, ~/code, ~/src, ~/repos, ~/workspace

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
