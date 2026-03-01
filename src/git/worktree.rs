use std::path::{Path, PathBuf};

use super::{default_branch, run_git};
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

    let branch = default_branch(repo_root);

    // Fetch latest so the worktree isn't based on a stale local ref
    let fetch_branch = branch.strip_prefix("origin/").unwrap_or(&branch);
    let _ = run_git(repo_root, &["fetch", "origin", fetch_branch]);

    let worktree_str = worktree_path.to_string_lossy();
    run_git(
        repo_root,
        &["worktree", "add", "-b", &branch_name, &worktree_str, &branch],
    )?;

    Ok(worktree_path)
}

pub fn remove_worktree(repo_root: &Path, worktree_path: &Path) {
    let path_str = worktree_path.to_string_lossy();
    let _ = run_git(repo_root, &["worktree", "remove", "--force", &path_str]);
}
