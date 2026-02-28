use std::path::{Path, PathBuf};
use std::process::Command;

use super::git;
use super::repo::home_dir;

fn generate_short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    // Mix bits for better distribution
    let mixed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let chars: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    (0..5)
        .map(|i| {
            let idx = ((mixed >> (i * 6)) & 0x1F) as usize % chars.len();
            chars[idx] as char
        })
        .collect()
}

pub fn create_worktree(
    repo_root: &Path,
    project_name: &str,
    _session_id: &str,
) -> Result<PathBuf, String> {
    let home = home_dir().ok_or("could not determine home directory")?;
    let dif_dir = home.join(".dif");

    let sanitized_name = project_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>();

    let project_dir = dif_dir.join(&sanitized_name);
    std::fs::create_dir_all(&project_dir)
        .map_err(|e| format!("failed to create ~/.dif/{sanitized_name}: {e}"))?;

    let short_id = generate_short_id();
    let worktree_path = project_dir.join(&short_id);
    let branch_name = format!("dif/{sanitized_name}-{short_id}");

    // Detect the default branch from origin (main, master, etc.)
    let default_branch = Command::new(git())
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
    let _ = Command::new(git())
        .arg("-C")
        .arg(repo_root)
        .args(["fetch", "origin", fetch_branch])
        .output();

    let output = Command::new(git())
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
    let _ = Command::new(git())
        .arg("-C")
        .arg(repo_root)
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .output();
}
