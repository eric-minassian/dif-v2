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

pub fn slugify_message(message: &str) -> String {
    let slug: String = message
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse consecutive hyphens and trim leading/trailing hyphens
    let mut result = String::new();
    let mut prev_hyphen = true; // treat start as hyphen to trim leading
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    // Trim trailing hyphen
    if result.ends_with('-') {
        result.pop();
    }
    // Truncate to 50 chars on a clean boundary
    if result.len() > 50 {
        result.truncate(50);
        if result.ends_with('-') {
            result.pop();
        }
    }
    result
}

pub fn create_worktree(
    repo_root: &Path,
    commit_message: &str,
) -> Result<PathBuf, String> {
    let home = home_dir().ok_or("could not determine home directory")?;
    let dif_dir = home.join(".dif");

    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");
    let sanitized_name = repo_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>();

    let project_dir = dif_dir.join(&sanitized_name);
    std::fs::create_dir_all(&project_dir)
        .map_err(|e| format!("failed to create ~/.dif/{sanitized_name}: {e}"))?;

    let short_id = generate_short_id();
    let slug = slugify_message(commit_message);
    let worktree_path = project_dir.join(&short_id);
    let branch_name = format!("dif/{slug}-{short_id}");

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
