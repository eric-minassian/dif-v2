use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::state::{DiffData, DiffLine, DiffLineKind, GitChange};

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

pub fn get_file_diff(repo_root: &Path, file_path: &str, status_code: &str) -> Result<String, String> {
    let (_old_path, new_path) = if file_path.contains(" -> ") {
        let parts: Vec<&str> = file_path.splitn(2, " -> ").collect();
        (parts[0], parts[1])
    } else {
        (file_path, file_path)
    };

    if status_code.trim() == "??" {
        let full_path = repo_root.join(new_path);
        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| format!("failed to read {}: {e}", full_path.display()))?;
        let line_count = content.lines().count();
        let mut diff = format!(
            "--- /dev/null\n+++ b/{new_path}\n@@ -0,0 +1,{line_count} @@\n"
        );
        for line in content.lines() {
            diff.push('+');
            diff.push_str(line);
            diff.push('\n');
        }
        return Ok(diff);
    }

    // Try unstaged then staged
    let unstaged = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["diff", "--", new_path])
        .output()
        .map_err(|e| format!("failed to run git diff: {e}"))?;

    let staged = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["diff", "--cached", "--", new_path])
        .output()
        .map_err(|e| format!("failed to run git diff --cached: {e}"))?;

    let mut combined = String::new();
    if unstaged.status.success() {
        combined.push_str(&String::from_utf8_lossy(&unstaged.stdout));
    }
    if staged.status.success() {
        let staged_str = String::from_utf8_lossy(&staged.stdout);
        if !staged_str.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&staged_str);
        }
    }

    if combined.is_empty() {
        Err(format!("no diff available for {new_path}"))
    } else {
        Ok(combined)
    }
}

pub fn parse_unified_diff(file_path: &str, raw_diff: &str) -> DiffData {
    let mut lines = Vec::new();
    let mut left_num: u32 = 0;
    let mut right_num: u32 = 0;
    let mut pending_deletions: Vec<String> = Vec::new();

    for raw_line in raw_diff.lines() {
        if raw_line.starts_with("diff ")
            || raw_line.starts_with("index ")
            || raw_line.starts_with("--- ")
            || raw_line.starts_with("+++ ")
        {
            continue;
        }

        if raw_line.starts_with("@@ ") {
            flush_pending(&mut lines, &mut pending_deletions, &mut left_num);
            if let Some((l, r)) = parse_hunk_header(raw_line) {
                left_num = l;
                right_num = r;
            }
            lines.push(DiffLine {
                left_number: None,
                left_text: raw_line.to_string(),
                right_number: None,
                right_text: String::new(),
                kind: DiffLineKind::Context,
            });
            continue;
        }

        if let Some(text) = raw_line.strip_prefix('-') {
            pending_deletions.push(text.to_string());
        } else if let Some(text) = raw_line.strip_prefix('+') {
            if let Some(del_text) = pending_deletions.pop() {
                lines.push(DiffLine {
                    left_number: Some(left_num),
                    left_text: del_text,
                    right_number: Some(right_num),
                    right_text: text.to_string(),
                    kind: DiffLineKind::Modified,
                });
                left_num += 1;
                right_num += 1;
            } else {
                lines.push(DiffLine {
                    left_number: None,
                    left_text: String::new(),
                    right_number: Some(right_num),
                    right_text: text.to_string(),
                    kind: DiffLineKind::Addition,
                });
                right_num += 1;
            }
        } else {
            flush_pending(&mut lines, &mut pending_deletions, &mut left_num);
            let text = raw_line.strip_prefix(' ').unwrap_or(raw_line);
            lines.push(DiffLine {
                left_number: Some(left_num),
                left_text: text.to_string(),
                right_number: Some(right_num),
                right_text: text.to_string(),
                kind: DiffLineKind::Context,
            });
            left_num += 1;
            right_num += 1;
        }
    }

    flush_pending(&mut lines, &mut pending_deletions, &mut left_num);

    DiffData {
        file_path: file_path.to_string(),
        lines,
    }
}

fn flush_pending(lines: &mut Vec<DiffLine>, pending: &mut Vec<String>, left_num: &mut u32) {
    for del_text in pending.drain(..) {
        lines.push(DiffLine {
            left_number: Some(*left_num),
            left_text: del_text,
            right_number: None,
            right_text: String::new(),
            kind: DiffLineKind::Deletion,
        });
        *left_num += 1;
    }
}

fn parse_hunk_header(line: &str) -> Option<(u32, u32)> {
    let after_at = line.strip_prefix("@@ ")?;
    let parts: Vec<&str> = after_at.splitn(3, ' ').collect();
    if parts.len() < 2 {
        return None;
    }
    let left_start = parts[0].strip_prefix('-')?.split(',').next()?.parse::<u32>().ok()?;
    let right_start = parts[1].strip_prefix('+')?.split(',').next()?.parse::<u32>().ok()?;
    Some((left_start, right_start))
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
    use crate::state::DiffLineKind;

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
    fn parses_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -10,5 +20,8 @@ fn foo"), Some((10, 20)));
        assert_eq!(parse_hunk_header("@@ -1 +1 @@"), Some((1, 1)));
        assert_eq!(parse_hunk_header("not a header"), None);
    }

    #[test]
    fn parses_unified_diff_side_by_side() {
        let diff = "\
--- a/file.rs
+++ b/file.rs
@@ -1,4 +1,4 @@
 line one
-old line
+new line
 line three
";
        let data = parse_unified_diff("file.rs", diff);
        assert_eq!(data.file_path, "file.rs");

        // Hunk header + context + modified + context = 4 lines
        assert_eq!(data.lines.len(), 4);

        // Context line
        assert_eq!(data.lines[1].kind, DiffLineKind::Context);
        assert_eq!(data.lines[1].left_text, "line one");
        assert_eq!(data.lines[1].right_text, "line one");

        // Modified line (paired deletion + addition)
        assert_eq!(data.lines[2].kind, DiffLineKind::Modified);
        assert_eq!(data.lines[2].left_text, "old line");
        assert_eq!(data.lines[2].right_text, "new line");
    }

    #[test]
    fn parses_additions_and_deletions() {
        let diff = "\
--- a/file.rs
+++ b/file.rs
@@ -1,3 +1,4 @@
 context
-removed
+added one
+added two
 end
";
        let data = parse_unified_diff("file.rs", diff);
        // Hunk header + context + modified(removed/added one) + addition(added two) + context = 5
        assert_eq!(data.lines.len(), 5);
        assert_eq!(data.lines[2].kind, DiffLineKind::Modified);
        assert_eq!(data.lines[3].kind, DiffLineKind::Addition);
        assert_eq!(data.lines[3].right_text, "added two");
    }
}
