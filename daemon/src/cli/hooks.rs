//! Git hook installation and management.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use tracing::info;

use crate::error::Error;

/// Post-commit hook script content.
const POST_COMMIT_HOOK: &str = r#"#!/bin/sh
# Squirrel doc debt tracking (auto-installed)
# Records which files changed for doc debt detection

sqrl _internal docguard-record "$@" 2>/dev/null || true
"#;

/// Pre-push hook script content.
const PRE_PUSH_HOOK: &str = r#"#!/bin/sh
# Squirrel doc debt check (auto-installed)
# Warns if there's unresolved doc debt before push

sqrl _internal docguard-check "$@" 2>/dev/null || true
"#;

/// Pre-push hook with blocking enabled.
const PRE_PUSH_HOOK_BLOCKING: &str = r#"#!/bin/sh
# Squirrel doc debt check (auto-installed, blocking mode)
# Blocks push if there's unresolved doc debt

if ! sqrl _internal docguard-check "$@" 2>/dev/null; then
    echo "Push blocked: unresolved doc debt. Run 'sqrl status' to see details."
    exit 1
fi
"#;

/// Check if git is initialized in the project.
pub fn has_git(project_root: &Path) -> bool {
    project_root.join(".git").exists()
}

/// Check if Squirrel hooks are already installed.
pub fn hooks_installed(project_root: &Path) -> bool {
    let hooks_dir = project_root.join(".git").join("hooks");
    let post_commit = hooks_dir.join("post-commit");

    if !post_commit.exists() {
        return false;
    }

    // Check if it's our hook (contains "Squirrel")
    fs::read_to_string(&post_commit)
        .map(|content| content.contains("Squirrel"))
        .unwrap_or(false)
}

/// Install Squirrel git hooks.
pub fn install_hooks(project_root: &Path, pre_push_block: bool) -> Result<(), Error> {
    let git_dir = project_root.join(".git");
    if !git_dir.exists() {
        return Ok(()); // No git, nothing to do
    }

    let hooks_dir = git_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)?;

    // Install post-commit hook
    let post_commit_path = hooks_dir.join("post-commit");
    install_hook(&post_commit_path, POST_COMMIT_HOOK)?;
    info!("Installed post-commit hook");

    // Install pre-push hook
    let pre_push_path = hooks_dir.join("pre-push");
    let pre_push_content = if pre_push_block {
        PRE_PUSH_HOOK_BLOCKING
    } else {
        PRE_PUSH_HOOK
    };
    install_hook(&pre_push_path, pre_push_content)?;
    info!("Installed pre-push hook");

    Ok(())
}

/// Install a single hook, preserving existing hooks.
fn install_hook(path: &Path, content: &str) -> Result<(), Error> {
    let final_content = if path.exists() {
        let existing = fs::read_to_string(path)?;

        // Already has our hook
        if existing.contains("Squirrel") {
            return Ok(());
        }

        // Append to existing hook
        format!("{}\n\n{}", existing.trim(), content)
    } else {
        content.to_string()
    };

    fs::write(path, &final_content)?;

    // Make executable
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms)?;

    Ok(())
}

/// Uninstall Squirrel git hooks.
pub fn uninstall_hooks(project_root: &Path) -> Result<(), Error> {
    let hooks_dir = project_root.join(".git").join("hooks");
    if !hooks_dir.exists() {
        return Ok(());
    }

    for hook_name in &["post-commit", "pre-push"] {
        let hook_path = hooks_dir.join(hook_name);
        if hook_path.exists() {
            let content = fs::read_to_string(&hook_path)?;
            if content.contains("Squirrel") {
                // Remove our section or the entire file
                let cleaned = remove_squirrel_section(&content);
                if cleaned.trim().is_empty() || cleaned.trim() == "#!/bin/sh" {
                    fs::remove_file(&hook_path)?;
                } else {
                    fs::write(&hook_path, cleaned)?;
                }
                info!(hook = hook_name, "Removed Squirrel hook");
            }
        }
    }

    Ok(())
}

/// Remove Squirrel section from hook content.
fn remove_squirrel_section(content: &str) -> String {
    content
        .lines()
        .filter(|line| !line.contains("Squirrel") && !line.contains("sqrl _internal"))
        .collect::<Vec<_>>()
        .join("\n")
}
