use std::path::{Path, PathBuf};

use super::run_git;

pub fn normalize_repo_path(path: &Path) -> Result<PathBuf, String> {
    let expanded = expand_tilde(path);
    let canonical = expanded
        .canonicalize()
        .map_err(|error| format!("failed to access {}: {error}", expanded.display()))?;

    let toplevel = run_git(&canonical, &["rev-parse", "--show-toplevel"])
        .map_err(|_| "that folder is not inside a Git worktree".to_string())?;

    PathBuf::from(&toplevel)
        .canonicalize()
        .map_err(|error| format!("failed to normalize {toplevel}: {error}"))
}

pub fn is_valid_repo(path: &Path) -> bool {
    path.exists() && normalize_repo_path(path).is_ok()
}

pub fn get_branch_name(worktree: &Path) -> Result<String, String> {
    run_git(worktree, &["rev-parse", "--abbrev-ref", "HEAD"])
}

pub(crate) fn expand_tilde(path: &Path) -> PathBuf {
    let Some(raw) = path.to_str() else {
        return path.to_path_buf();
    };

    if raw == "~" {
        return home_dir().unwrap_or_else(|| path.to_path_buf());
    }

    if let Some(stripped) = raw.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(stripped);
    }

    path.to_path_buf()
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
