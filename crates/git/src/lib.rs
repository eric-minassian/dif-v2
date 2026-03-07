mod commands;
pub mod conventional;
pub mod diff;
mod repo;
pub(crate) mod status;
pub(crate) mod syntax;
mod types;
mod worktree;

#[cfg(test)]
mod tests;

pub use types::*;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

/// Resolve the full path to a CLI tool.
///
/// macOS GUI apps don't inherit the shell's PATH, so binaries installed via
/// Homebrew (e.g. `/opt/homebrew/bin/gh`) aren't found by a bare
/// `Command::new("gh")`. This checks common locations.
fn resolve_bin(name: &str) -> PathBuf {
    static EXTRA_PATHS: &[&str] = &["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"];

    for dir in EXTRA_PATHS {
        let p = Path::new(dir).join(name);
        if p.exists() {
            return p;
        }
    }

    PathBuf::from(name)
}

fn git() -> &'static Path {
    static GIT: OnceLock<PathBuf> = OnceLock::new();
    GIT.get_or_init(|| resolve_bin("git"))
}

fn gh() -> &'static Path {
    static GH: OnceLock<PathBuf> = OnceLock::new();
    GH.get_or_init(|| resolve_bin("gh"))
}

// ── Command helpers ─────────────────────────────────────────────────────────

/// Run a git command in `dir` and return the raw `Output` on success.
pub(crate) fn run_git_raw(dir: &Path, args: &[&str]) -> Result<Output, String> {
    let label = args.first().copied().unwrap_or("git");
    let output = Command::new(git())
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git {label}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {label}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output)
}

/// Run a git command in `dir` and return trimmed stdout.
pub(crate) fn run_git(dir: &Path, args: &[&str]) -> Result<String, String> {
    run_git_raw(dir, args).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Run a git command, returning `None` on any failure.
pub(crate) fn try_run_git(dir: &Path, args: &[&str]) -> Option<Output> {
    Command::new(git())
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
}

/// Run a `gh` CLI command and return trimmed stdout.
pub(crate) fn run_gh(dir: &Path, args: &[&str]) -> Result<String, String> {
    let label = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
    let output = Command::new(gh())
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("gh {label}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "gh {label}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run a `gh` CLI command, returning `None` on any failure.
pub(crate) fn try_run_gh(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new(gh())
        .current_dir(dir)
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Detect the default branch from the remote (e.g. `origin/main`).
pub(crate) fn default_branch(dir: &Path) -> String {
    run_git(
        dir,
        &["symbolic-ref", "refs/remotes/origin/HEAD", "--short"],
    )
    .ok()
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "origin/main".to_string())
}

pub use commands::{
    amend_selected, commit_selected, create_pr, disable_auto_merge, enable_auto_merge, force_push,
    merge_pr_rebase, push,
};
pub use diff::compute_file_diff;
pub use repo::{get_branch_name, is_valid_repo, normalize_repo_path};
pub use status::{check_repo_capabilities, collect_branch_status, collect_changes};
pub use worktree::{create_worktree, remove_worktree};
