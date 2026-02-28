use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use similar::TextDiff;

use crate::state::{BranchStatus, DiffData, GitChange, SplitLine, SplitLineKind};

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

    let numstat = collect_numstat(repo_root);
    let mut changes = parse_status_entries(&output.stdout);

    for change in &mut changes {
        let lookup_path = if change.path.contains(" -> ") {
            change.path.split(" -> ").last().unwrap_or(&change.path)
        } else {
            &change.path
        };

        if change.status_code == "??" {
            let lines = count_file_lines(repo_root, lookup_path);
            change.additions = lines;
            change.deletions = Some(0);
        } else if let Some(&(adds, dels)) = numstat.get(lookup_path) {
            change.additions = adds;
            change.deletions = dels;
        }
    }

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
            additions: None,
            deletions: None,
        });
    }

    changes
}

fn collect_numstat(repo_root: &Path) -> HashMap<String, (Option<u32>, Option<u32>)> {
    let mut map = HashMap::new();

    for extra_args in [&["diff", "--numstat", "-z"][..], &["diff", "--cached", "--numstat", "-z"]] {
        if let Ok(output) = Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(extra_args)
            .output()
        {
            if output.status.success() {
                parse_numstat_output(&output.stdout, &mut map);
            }
        }
    }

    map
}

fn parse_numstat_output(bytes: &[u8], map: &mut HashMap<String, (Option<u32>, Option<u32>)>) {
    let text = String::from_utf8_lossy(bytes);
    // With -z, paths are NUL-separated but the stats line uses \t.
    // Format: "adds\tdels\tpath\0" or for renames "adds\tdels\t\0old\0new\0"
    let mut parts = text.split('\0');
    while let Some(stat_line) = parts.next() {
        let stat_line = stat_line.trim();
        if stat_line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = stat_line.splitn(3, '\t').collect();
        if fields.len() < 3 {
            continue;
        }
        let adds = fields[0].parse::<u32>().ok();
        let dels = fields[1].parse::<u32>().ok();
        let path_field = fields[2];

        let file_path = if path_field.is_empty() {
            // Rename: next two NUL-separated fields are old and new paths
            let _old = parts.next().unwrap_or_default();
            let new = parts.next().unwrap_or_default();
            new.to_string()
        } else {
            path_field.to_string()
        };

        if !file_path.is_empty() {
            let entry = map.entry(file_path).or_insert((Some(0), Some(0)));
            if let (Some(a), Some(ea)) = (adds, &mut entry.0) {
                *ea += a;
            } else {
                entry.0 = None;
            }
            if let (Some(d), Some(ed)) = (dels, &mut entry.1) {
                *ed += d;
            } else {
                entry.1 = None;
            }
        }
    }
}

fn count_file_lines(repo_root: &Path, relative_path: &str) -> Option<u32> {
    let full_path = repo_root.join(relative_path);
    let content = std::fs::read_to_string(&full_path).ok()?;
    Some(content.lines().count() as u32)
}

/// Get the committed (HEAD) version of a file from git.
fn get_base_content(repo_root: &Path, file_path: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["show", &format!("HEAD:{file_path}")])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        None
    }
}

/// Compute a split (side-by-side) diff between the committed and working-tree
/// versions of a file. Uses the `similar` crate for correct diff computation.
pub fn compute_file_diff(
    repo_root: &Path,
    file_path: &str,
    status_code: &str,
) -> Result<DiffData, String> {
    let (old_path, new_path) = if file_path.contains(" -> ") {
        let parts: Vec<&str> = file_path.splitn(2, " -> ").collect();
        (parts[0], parts[1])
    } else {
        (file_path, file_path)
    };

    let old_content = if status_code.trim() == "??" {
        String::new()
    } else {
        get_base_content(repo_root, old_path).unwrap_or_default()
    };

    let new_content = if status_code.contains('D') {
        String::new()
    } else {
        let full_path = repo_root.join(new_path);
        std::fs::read_to_string(&full_path)
            .map_err(|e| format!("failed to read {}: {e}", full_path.display()))?
    };

    Ok(build_split_diff(file_path, &old_content, &new_content))
}

/// Build split-view diff lines from old/new content using `similar`.
fn build_split_diff(file_path: &str, old: &str, new: &str) -> DiffData {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    let mut additions: u32 = 0;
    let mut deletions: u32 = 0;
    let mut old_lineno: u32 = 1;
    let mut new_lineno: u32 = 1;

    for op in diff.ops() {
        match op {
            similar::DiffOp::Equal {
                old_index,
                new_index,
                len,
            } => {
                let old_lines = diff.old_slices()[*old_index..*old_index + *len].to_vec();
                for text in old_lines {
                    lines.push(SplitLine {
                        old_lineno: Some(old_lineno),
                        new_lineno: Some(new_lineno),
                        old_text: strip_newline(text),
                        new_text: strip_newline(text),
                        kind: SplitLineKind::Equal,
                    });
                    old_lineno += 1;
                    new_lineno += 1;
                }
                let _ = new_index; // used implicitly
            }
            similar::DiffOp::Delete {
                old_index,
                old_len,
                new_index: _,
            } => {
                for i in 0..*old_len {
                    let text = diff.old_slices()[*old_index + i];
                    lines.push(SplitLine {
                        old_lineno: Some(old_lineno),
                        new_lineno: None,
                        old_text: strip_newline(text),
                        new_text: String::new(),
                        kind: SplitLineKind::Delete,
                    });
                    old_lineno += 1;
                    deletions += 1;
                }
            }
            similar::DiffOp::Insert {
                old_index: _,
                new_index,
                new_len,
            } => {
                for i in 0..*new_len {
                    let text = diff.new_slices()[*new_index + i];
                    lines.push(SplitLine {
                        old_lineno: None,
                        new_lineno: Some(new_lineno),
                        old_text: String::new(),
                        new_text: strip_newline(text),
                        kind: SplitLineKind::Insert,
                    });
                    new_lineno += 1;
                    additions += 1;
                }
            }
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                let max = (*old_len).max(*new_len);
                for i in 0..max {
                    let has_old = i < *old_len;
                    let has_new = i < *new_len;
                    let old_text = if has_old {
                        strip_newline(diff.old_slices()[*old_index + i])
                    } else {
                        String::new()
                    };
                    let new_text = if has_new {
                        strip_newline(diff.new_slices()[*new_index + i])
                    } else {
                        String::new()
                    };
                    lines.push(SplitLine {
                        old_lineno: if has_old {
                            let n = old_lineno;
                            old_lineno += 1;
                            Some(n)
                        } else {
                            None
                        },
                        new_lineno: if has_new {
                            let n = new_lineno;
                            new_lineno += 1;
                            Some(n)
                        } else {
                            None
                        },
                        old_text,
                        new_text,
                        kind: SplitLineKind::Replace,
                    });
                    if has_old {
                        deletions += 1;
                    }
                    if has_new {
                        additions += 1;
                    }
                }
            }
        }
    }

    DiffData {
        file_path: file_path.to_string(),
        lines,
        additions,
        deletions,
    }
}

fn strip_newline(s: &str) -> String {
    s.strip_suffix('\n')
        .or_else(|| s.strip_suffix("\r\n"))
        .unwrap_or(s)
        .to_string()
}

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

pub fn commit_all(worktree: &Path, message: &str) -> Result<(), String> {
    // Stage everything
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["add", "-A"])
        .output()
        .map_err(|e| format!("git add failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    // Commit
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["commit", "-m", message])
        .output()
        .map_err(|e| format!("git commit failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn push(worktree: &Path) -> Result<(), String> {
    let branch = get_branch_name(worktree)?;

    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["push", "-u", "origin", &branch])
        .output()
        .map_err(|e| format!("git push failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git push failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn create_pr(worktree: &Path, title: &str) -> Result<String, String> {
    let output = Command::new("gh")
        .current_dir(worktree)
        .args(["pr", "create", "--title", title, "--body", "", "--fill"])
        .output()
        .map_err(|e| format!("gh pr create failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "gh pr create failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn merge_pr_rebase(worktree: &Path) -> Result<(), String> {
    let output = Command::new("gh")
        .current_dir(worktree)
        .args(["pr", "merge", "--rebase"])
        .output()
        .map_err(|e| format!("gh pr merge failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "gh pr merge failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    // Delete the remote branch separately — we skip local branch deletion
    // because the branch is checked out in this worktree and `main` is
    // checked out in the main worktree, so `gh --delete-branch` would fail.
    let branch = get_branch_name(worktree).unwrap_or_default();
    if !branch.is_empty() {
        let _ = Command::new("git")
            .arg("-C")
            .arg(worktree)
            .args(["push", "origin", "--delete", &branch])
            .output();
    }

    Ok(())
}

pub fn commits_ahead_of_main(worktree: &Path) -> Result<u32, String> {
    // Detect default branch
    let default_branch = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| "origin/main".to_string());

    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["rev-list", "--count", &format!("{default_branch}..HEAD")])
        .output()
        .map_err(|e| format!("git rev-list failed: {e}"))?;

    if !output.status.success() {
        return Ok(0);
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .unwrap_or(0))
}

pub fn check_pr_status(worktree: &Path) -> Result<Option<(String, bool)>, String> {
    let output = Command::new("gh")
        .current_dir(worktree)
        .args(["pr", "view", "--json", "url,state"])
        .output()
        .map_err(|e| format!("gh pr view failed: {e}"))?;

    if !output.status.success() {
        // No PR exists for this branch
        return Ok(None);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(|e| format!("failed to parse gh output: {e}"))?;

    let url = parsed["url"].as_str().unwrap_or_default().to_string();
    let state = parsed["state"].as_str().unwrap_or_default();
    let merged = state == "MERGED";

    if url.is_empty() {
        return Ok(None);
    }

    Ok(Some((url, merged)))
}

pub fn collect_branch_status(worktree: &Path) -> BranchStatus {
    let commits_ahead = commits_ahead_of_main(worktree).unwrap_or(0);
    let (pr_url, pr_merged) = match check_pr_status(worktree) {
        Ok(Some((url, merged))) => (Some(url), merged),
        _ => (None, false),
    };
    BranchStatus {
        commits_ahead,
        pr_url,
        pr_merged,
    }
}

pub fn commit_selected(worktree: &Path, files: &[String], message: &str) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(worktree).arg("add").arg("--");
    for file in files {
        cmd.arg(file);
    }
    let output = cmd.output().map_err(|e| format!("git add failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["commit", "-m", message])
        .output()
        .map_err(|e| format!("git commit failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn amend_selected(worktree: &Path, files: &[String]) -> Result<(), String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(worktree).arg("add").arg("--");
    for file in files {
        cmd.arg(file);
    }
    let output = cmd.output().map_err(|e| format!("git add failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["commit", "--amend", "--no-edit"])
        .output()
        .map_err(|e| format!("git commit --amend failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git commit --amend failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn force_push(worktree: &Path) -> Result<(), String> {
    let branch = get_branch_name(worktree)?;

    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["push", "--force-with-lease", "-u", "origin", &branch])
        .output()
        .map_err(|e| format!("git push failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git push failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn remove_worktree(repo_root: &Path, worktree_path: &Path) {
    let _ = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .output();
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
    use super::*;
    use crate::state::SplitLineKind;

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

    #[test]
    fn parses_numstat_output() {
        let mut map = HashMap::new();
        let bytes = b"10\t5\tsrc/main.rs\0-\t-\tbinary.png\0";
        parse_numstat_output(bytes, &mut map);

        assert_eq!(map.get("src/main.rs"), Some(&(Some(10), Some(5))));
        assert_eq!(map.get("binary.png"), Some(&(None, None)));
    }

    #[test]
    fn split_diff_replace() {
        let old = "line one\nold line\nline three\n";
        let new = "line one\nnew line\nline three\n";
        let data = build_split_diff("file.rs", old, new);

        assert_eq!(data.file_path, "file.rs");
        assert_eq!(data.lines.len(), 3);

        assert_eq!(data.lines[0].kind, SplitLineKind::Equal);
        assert_eq!(data.lines[0].old_text, "line one");
        assert_eq!(data.lines[0].new_text, "line one");

        assert_eq!(data.lines[1].kind, SplitLineKind::Replace);
        assert_eq!(data.lines[1].old_text, "old line");
        assert_eq!(data.lines[1].new_text, "new line");

        assert_eq!(data.lines[2].kind, SplitLineKind::Equal);
    }

    #[test]
    fn split_diff_insert_delete() {
        let old = "context\nremoved\nend\n";
        let new = "context\nadded one\nadded two\nend\n";
        let data = build_split_diff("file.rs", old, new);

        assert!(data.additions >= 2);
        assert!(data.deletions >= 1);

        let has_insert = data.lines.iter().any(|l| {
            l.kind == SplitLineKind::Insert || l.kind == SplitLineKind::Replace
        });
        assert!(has_insert);
    }

    #[test]
    fn split_diff_new_file() {
        let data = build_split_diff("new.rs", "", "hello\nworld\n");

        assert_eq!(data.lines.len(), 2);
        assert!(data.lines.iter().all(|l| l.kind == SplitLineKind::Insert));
        assert_eq!(data.additions, 2);
        assert_eq!(data.deletions, 0);
    }

    #[test]
    fn split_diff_deleted_file() {
        let data = build_split_diff("gone.rs", "goodbye\nworld\n", "");

        assert_eq!(data.lines.len(), 2);
        assert!(data.lines.iter().all(|l| l.kind == SplitLineKind::Delete));
        assert_eq!(data.additions, 0);
        assert_eq!(data.deletions, 2);
    }
}
