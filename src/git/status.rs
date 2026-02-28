use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::state::{BranchStatus, GitChange};

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

pub fn parse_numstat_output(bytes: &[u8], map: &mut HashMap<String, (Option<u32>, Option<u32>)>) {
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
