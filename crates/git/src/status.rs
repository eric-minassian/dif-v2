use std::collections::HashMap;
use std::path::Path;

use super::types::{BranchStatus, CheckBucket, CiCheck, GitChange, RepoCapabilities};

use super::{default_branch, run_git, run_git_raw, try_run_gh, try_run_git};

pub fn collect_changes(repo_root: &Path) -> Result<Vec<GitChange>, String> {
    let (status_result, numstat) = std::thread::scope(|s| {
        let t1 = s.spawn(|| {
            run_git_raw(
                repo_root,
                &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            )
        });
        let t2 = s.spawn(|| collect_numstat(repo_root));
        (t1.join().unwrap(), t2.join().unwrap())
    });

    let output = status_result?;
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
    let (unstaged, staged) = std::thread::scope(|s| {
        let t1 = s.spawn(|| try_run_git(repo_root, &["diff", "--numstat", "-z"]));
        let t2 = s.spawn(|| try_run_git(repo_root, &["diff", "--cached", "--numstat", "-z"]));
        (t1.join().unwrap(), t2.join().unwrap())
    });

    let mut map = HashMap::new();
    if let Some(output) = unstaged {
        parse_numstat_output(&output.stdout, &mut map);
    }
    if let Some(output) = staged {
        parse_numstat_output(&output.stdout, &mut map);
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

fn commits_ahead_of_main(worktree: &Path) -> u32 {
    let branch = default_branch(worktree);
    run_git(
        worktree,
        &["rev-list", "--count", &format!("{branch}..HEAD")],
    )
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(0)
}

struct PrInfo {
    url: String,
    merged: bool,
    number: Option<u32>,
    state: Option<String>,
    auto_merge_enabled: bool,
}

fn check_pr_status(worktree: &Path) -> Option<PrInfo> {
    let text = try_run_gh(
        worktree,
        &["pr", "view", "--json", "url,state,number,autoMergeRequest"],
    )?;
    let parsed: serde_json::Value = serde_json::from_str(&text).ok()?;

    let url = parsed["url"]
        .as_str()
        .filter(|s| !s.is_empty())?
        .to_string();
    let state = parsed["state"].as_str().unwrap_or_default().to_string();

    Some(PrInfo {
        merged: state == "MERGED",
        number: parsed["number"].as_u64().map(|n| n as u32),
        auto_merge_enabled: !parsed["autoMergeRequest"].is_null(),
        state: Some(state),
        url,
    })
}

pub fn check_repo_capabilities(worktree: &Path) -> RepoCapabilities {
    try_run_gh(
        worktree,
        &[
            "api",
            "repos/{owner}/{repo}",
            "--jq",
            "{allow_auto_merge,allow_rebase_merge}",
        ],
    )
    .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
    .map(|parsed| RepoCapabilities {
        auto_merge_allowed: parsed["allow_auto_merge"].as_bool().unwrap_or(false),
        rebase_merge_allowed: parsed["allow_rebase_merge"].as_bool().unwrap_or(true),
    })
    .unwrap_or_default()
}

fn collect_pr_checks(worktree: &Path) -> Vec<CiCheck> {
    let entries: Vec<serde_json::Value> = try_run_gh(
        worktree,
        &["pr", "checks", "--json", "name,bucket,workflow,link"],
    )
    .and_then(|text| serde_json::from_str(&text).ok())
    .unwrap_or_default();

    entries
        .into_iter()
        .map(|entry| {
            let name = entry["name"].as_str().unwrap_or_default().to_string();
            let bucket = match entry["bucket"].as_str().unwrap_or_default() {
                "pass" => CheckBucket::Pass,
                "fail" => CheckBucket::Fail,
                "pending" => CheckBucket::Pending,
                "skipping" => CheckBucket::Skipping,
                _ => CheckBucket::Cancel,
            };
            let workflow = entry["workflow"].as_str().unwrap_or_default().to_string();
            let link = entry["link"]
                .as_str()
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty());

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
    let (commits_ahead, branch_name, pr_info) = std::thread::scope(|s| {
        let t1 = s.spawn(|| commits_ahead_of_main(worktree));
        let t2 = s.spawn(|| super::get_branch_name(worktree).ok());
        let t3 = s.spawn(|| check_pr_status(worktree));
        (t1.join().unwrap(), t2.join().unwrap(), t3.join().unwrap())
    });

    let (pr_url, pr_merged, pr_number, pr_state, auto_merge_enabled) = match pr_info {
        Some(info) => (
            Some(info.url),
            info.merged,
            info.number,
            info.state,
            info.auto_merge_enabled,
        ),
        None => (None, false, None, None, false),
    };

    // collect_pr_checks depends on pr_url, must run after
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
        branch_name,
    }
}
