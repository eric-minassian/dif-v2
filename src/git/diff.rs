use std::path::Path;

use similar::TextDiff;

use crate::state::{DiffData, SplitLine, SplitLineKind};

use super::try_run_git;

/// Get the committed (HEAD) version of a file from git.
fn get_base_content(repo_root: &Path, file_path: &str) -> Option<String> {
    let output = try_run_git(repo_root, &["show", &format!("HEAD:{file_path}")])?;
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
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
pub(crate) fn build_split_diff(file_path: &str, old: &str, new: &str) -> DiffData {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    let mut additions: u32 = 0;
    let mut deletions: u32 = 0;
    let mut old_lineno: u32 = 1;
    let mut new_lineno: u32 = 1;

    for op in diff.ops() {
        match op {
            similar::DiffOp::Equal {
                old_index, len, ..
            } => {
                for &text in &diff.old_slices()[*old_index..*old_index + *len] {
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
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
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
                new_index, new_len, ..
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
