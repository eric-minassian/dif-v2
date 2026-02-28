use std::path::{Path, PathBuf};
use std::process::Command;

use crate::state::GitChange;

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

pub fn collect_changes(repo_root: &Path) -> Result<Vec<GitChange>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .output()
        .map_err(|error| format!("failed to run git status: {error}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut changes = parse_status_entries(&output.stdout);
    changes.sort_by(|left, right| left.sort_key.cmp(&right.sort_key));
    Ok(changes)
}

pub fn parse_status_entries(bytes: &[u8]) -> Vec<GitChange> {
    let mut changes = Vec::new();
    let mut fields = bytes
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());

    while let Some(entry) = fields.next() {
        if entry.len() < 4 {
            continue;
        }

        let status_code = String::from_utf8_lossy(&entry[..2]).into_owned();
        let path = String::from_utf8_lossy(&entry[3..]).into_owned();
        let record_type = status_code.as_bytes().first().copied().unwrap_or_default();

        let (display_path, sort_key) = if matches!(record_type, b'R' | b'C') {
            let new_path = fields
                .next()
                .map(|field| String::from_utf8_lossy(field).into_owned())
                .unwrap_or_default();
            (format!("{path} -> {new_path}"), new_path.to_lowercase())
        } else {
            let sort_key = path.to_lowercase();
            (path, sort_key)
        };

        changes.push(GitChange {
            path: display_path,
            status_code,
            sort_key,
        });
    }

    changes
}

fn expand_tilde(path: &Path) -> PathBuf {
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

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::parse_status_entries;

    #[test]
    fn parses_standard_status_rows() {
        let bytes = b" M src/main.rs\0A  src/lib.rs\0?? Cargo.lock\0";
        let changes = parse_status_entries(bytes);

        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].status_code, " M");
        assert_eq!(changes[0].path, "src/main.rs");
        assert_eq!(changes[2].status_code, "??");
        assert_eq!(changes[2].path, "Cargo.lock");
    }

    #[test]
    fn parses_rename_rows() {
        let bytes = b"R  old.rs\0new.rs\0";
        let changes = parse_status_entries(bytes);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "old.rs -> new.rs");
        assert_eq!(changes[0].sort_key, "new.rs");
    }
}
