use std::path::{Path, PathBuf};
use std::process::Command;

pub fn normalize_repo_path(path: &Path) -> Result<PathBuf, String> {
    let expanded = expand_tilde(path);
    let canonical = expanded
        .canonicalize()
        .map_err(|error| format!("failed to access {}: {error}", expanded.display()))?;

    let output = Command::new("git")
        .arg("-C")
        .arg(&canonical)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|error| format!("failed to run git: {error}"))?;

    if !output.status.success() {
        return Err("that folder is not inside a Git worktree".to_string());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let repo_root = PathBuf::from(stdout.trim());
    repo_root
        .canonicalize()
        .map_err(|error| format!("failed to normalize {}: {error}", repo_root.display()))
}

pub fn is_valid_repo(path: &Path) -> bool {
    path.exists() && normalize_repo_path(path).is_ok()
}

pub fn get_branch_name(worktree: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .map_err(|e| format!("failed to get branch name: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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
