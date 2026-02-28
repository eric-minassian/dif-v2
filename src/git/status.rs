use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use super::{gh, git};
use crate::state::{BranchStatus, CheckBucket, CiCheck, GitChange, RepoCapabilities};

pub fn collect_changes(repo_root: &Path) -> Result<Vec<GitChange>, String> {
    let output = Command::new(git())
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
        if let Ok(output) = Command::new(git())
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
    let default_branch = Command::new(git())
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

    let output = Command::new(git())
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

pub struct PrInfo {
    pub url: String,
    pub merged: bool,
    pub number: Option<u32>,
    pub state: Option<String>,
    pub auto_merge_enabled: bool,
}

pub fn check_pr_status(worktree: &Path) -> Result<Option<PrInfo>, String> {
    let output = Command::new(gh())
        .current_dir(worktree)
        .args(["pr", "view", "--json", "url,state,number,autoMergeRequest"])
        .output()
        .map_err(|e| format!("gh pr view failed: {e}"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(text.trim()).map_err(|e| format!("failed to parse gh output: {e}"))?;

    let url = parsed["url"].as_str().unwrap_or_default().to_string();
    let state = parsed["state"].as_str().unwrap_or_default().to_string();
    let merged = state == "MERGED";
    let number = parsed["number"].as_u64().map(|n| n as u32);
    let auto_merge_enabled = !parsed["autoMergeRequest"].is_null();

    if url.is_empty() {
        return Ok(None);
    }

    Ok(Some(PrInfo {
        url,
        merged,
        number,
        state: Some(state),
        auto_merge_enabled,
    }))
}

pub fn check_repo_capabilities(worktree: &Path) -> RepoCapabilities {
    let output = match Command::new(gh())
        .current_dir(worktree)
        .args([
            "api",
            "repos/{owner}/{repo}",
            "--jq",
            "{allow_auto_merge,allow_rebase_merge}",
        ])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return RepoCapabilities::default(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(_) => return RepoCapabilities::default(),
    };

    RepoCapabilities {
        auto_merge_allowed: parsed["allow_auto_merge"].as_bool().unwrap_or(false),
        rebase_merge_allowed: parsed["allow_rebase_merge"].as_bool().unwrap_or(true),
    }
}

pub fn collect_pr_checks(worktree: &Path) -> Vec<CiCheck> {
    let output = match Command::new(gh())
        .current_dir(worktree)
        .args(["pr", "checks", "--json", "name,bucket,workflow,link"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let parsed: Vec<serde_json::Value> = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    parsed
        .into_iter()
        .map(|entry| {
            let name = entry["name"].as_str().unwrap_or_default().to_string();
            let bucket_str = entry["bucket"].as_str().unwrap_or_default();
            let workflow = entry["workflow"].as_str().unwrap_or_default().to_string();
            let link = entry["link"].as_str().map(|s| s.to_string()).filter(|s| !s.is_empty());

            let bucket = match bucket_str {
                "pass" => CheckBucket::Pass,
                "fail" => CheckBucket::Fail,
                "pending" => CheckBucket::Pending,
                "skipping" => CheckBucket::Skipping,
                _ => CheckBucket::Cancel,
            };

            CiCheck {
                name,
                bucket,
                workflow,
                link,
            }
        })
        .collect()
}

pub fn collect_branch_status(worktree: &Path) -> BranchStatus {
    let commits_ahead = commits_ahead_of_main(worktree).unwrap_or(0);
    let (pr_url, pr_merged, pr_number, pr_state, auto_merge_enabled) =
        match check_pr_status(worktree) {
            Ok(Some(info)) => (
                Some(info.url),
                info.merged,
                info.number,
                info.state,
                info.auto_merge_enabled,
            ),
            _ => (None, false, None, None, false),
        };

    let checks = if pr_url.is_some() {
        collect_pr_checks(worktree)
    } else {
        Vec::new()
    };

    BranchStatus {
        commits_ahead,
        pr_url,
        pr_merged,
        pr_number,
        pr_state,
        checks,
        auto_merge_enabled,
    }
}
