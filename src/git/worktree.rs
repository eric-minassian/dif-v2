use std::path::{Path, PathBuf};
use std::process::Command;

use super::repo::home_dir;

pub fn create_worktree(
    repo_root: &Path,
    project_name: &str,
    session_id: &str,
) -> Result<PathBuf, String> {
    let home = home_dir().ok_or("could not determine home directory")?;
    let dif_dir = home.join(".dif");

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    let sanitized_name = project_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>();
    let worktree_name = format!("{sanitized_name}-session-{session_id}-{timestamp}");
    let worktree_path = dif_dir.join(&worktree_name);
    let branch_name = format!("dif/{worktree_name}");

    std::fs::create_dir_all(&dif_dir)
        .map_err(|e| format!("failed to create ~/.dif: {e}"))?;

    // Detect the default branch from origin (main, master, etc.)
    let default_branch = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| "origin/main".to_string());

    // Fetch latest so the worktree isn't based on a stale local ref
    let fetch_branch = default_branch.strip_prefix("origin/").unwrap_or(&default_branch);
    let _ = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["fetch", "origin", fetch_branch])
        .output();

    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["worktree", "add", "-b", &branch_name])
        .arg(&worktree_path)
        .arg(&default_branch)
        .output()
        .map_err(|e| format!("failed to run git worktree add: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(worktree_path)
}

pub fn remove_worktree(repo_root: &Path, worktree_path: &Path) {
    let _ = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .output();
}
