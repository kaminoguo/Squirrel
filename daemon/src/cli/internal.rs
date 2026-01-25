//! Hidden internal commands for git hooks.

use crate::error::Error;

/// Record doc debt after a commit (called by post-commit hook).
pub fn docguard_record() -> Result<(), Error> {
    // TODO: Implement in Phase 9
    // 1. Get list of changed files from git
    // 2. Apply doc rules to detect expected doc updates
    // 3. Store doc debt in SCHEMA-006

    // For now, just acknowledge the call
    Ok(())
}

/// Check doc debt before push (called by pre-push hook).
pub fn docguard_check() -> Result<bool, Error> {
    // TODO: Implement in Phase 10
    // 1. Query unresolved doc debt from SCHEMA-006
    // 2. If any exists, print warning and return false
    // 3. If none, return true

    // For now, always pass
    Ok(true)
}
