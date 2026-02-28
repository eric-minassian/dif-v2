mod commands;
pub(crate) mod diff;
mod repo;
pub(crate) mod status;
mod worktree;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Resolve the full path to a CLI tool.
///
/// macOS GUI apps don't inherit the shell's PATH, so binaries installed via
/// Homebrew (e.g. `/opt/homebrew/bin/gh`) aren't found by a bare
/// `Command::new("gh")`. This checks common locations.
fn resolve_bin(name: &str) -> PathBuf {
    static EXTRA_PATHS: &[&str] = &[
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/bin",
        "/bin",
    ];

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

pub use commands::{
    amend_selected, commit_selected, create_pr, disable_auto_merge, enable_auto_merge, force_push,
    merge_pr_rebase, push,
};
pub use diff::compute_file_diff;
pub use repo::{get_branch_name, is_valid_repo, normalize_repo_path};
pub use status::{check_repo_capabilities, collect_branch_status, collect_changes};
pub use worktree::{create_worktree, remove_worktree};
