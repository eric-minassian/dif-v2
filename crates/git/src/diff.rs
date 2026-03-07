use std::collections::HashSet;
use std::mem;
use std::path::Path;
use std::sync::Arc;

use similar::TextDiff;

use super::types::{DiffData, DiffDisplayRow, SplitLine, SplitLineKind};

use super::syntax::highlight_lines;
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
pub fn build_split_diff(file_path: &str, old: &str, new: &str) -> DiffData {
    let diff = TextDiff::from_lines(old, new);
    let mut lines = Vec::new();
    let mut additions: u32 = 0;
    let mut deletions: u32 = 0;
    let mut old_lineno: u32 = 1;
    let mut new_lineno: u32 = 1;

    // Highlight both sides
    let mut old_highlights = highlight_lines(old, file_path);
    let mut new_highlights = highlight_lines(new, file_path);

    for op in diff.ops() {
        match op {
            similar::DiffOp::Equal {
                old_index, len, ..
            } => {
                for i in 0..*len {
                    let text = strip_newline(diff.old_slices()[*old_index + i]);
                    let old_runs = old_highlights
                        .get_mut((old_lineno - 1) as usize)
                        .map(mem::take)
                        .unwrap_or_default();
                    let new_runs = new_highlights
                        .get_mut((new_lineno - 1) as usize)
                        .map(mem::take)
                        .unwrap_or_default();
                    lines.push(SplitLine {
                        old_lineno: Some(old_lineno),
                        new_lineno: Some(new_lineno),
                        old_text: text.to_string(),
                        new_text: text.to_string(),
                        kind: SplitLineKind::Equal,
                        old_syntax_runs: old_runs,
                        new_syntax_runs: new_runs,
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
                    let old_runs = old_highlights
                        .get_mut((old_lineno - 1) as usize)
                        .map(mem::take)
                        .unwrap_or_default();
                    lines.push(SplitLine {
                        old_lineno: Some(old_lineno),
                        new_lineno: None,
                        old_text: strip_newline(text).to_string(),
                        new_text: String::new(),
                        kind: SplitLineKind::Delete,
                        old_syntax_runs: old_runs,
                        new_syntax_runs: Vec::new(),
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
                    let new_runs = new_highlights
                        .get_mut((new_lineno - 1) as usize)
                        .map(mem::take)
                        .unwrap_or_default();
                    lines.push(SplitLine {
                        old_lineno: None,
                        new_lineno: Some(new_lineno),
                        old_text: String::new(),
                        new_text: strip_newline(text).to_string(),
                        kind: SplitLineKind::Insert,
                        old_syntax_runs: Vec::new(),
                        new_syntax_runs: new_runs,
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
                        strip_newline(diff.old_slices()[*old_index + i]).to_string()
                    } else {
                        String::new()
                    };
                    let new_text = if has_new {
                        strip_newline(diff.new_slices()[*new_index + i]).to_string()
                    } else {
                        String::new()
                    };
                    let old_runs = if has_old {
                        old_highlights
                            .get_mut((old_lineno - 1) as usize)
                            .map(mem::take)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let new_runs = if has_new {
                        new_highlights
                            .get_mut((new_lineno - 1) as usize)
                            .map(mem::take)
                            .unwrap_or_default()
                    } else {
                        Vec::new()
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
                        old_syntax_runs: old_runs,
                        new_syntax_runs: new_runs,
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

    let display_rows = build_display_rows(&lines, &HashSet::new());

    DiffData {
        file_path: file_path.to_string(),
        lines: Arc::new(lines),
        display_rows: Arc::new(display_rows),
        expanded_sections: HashSet::new(),
        additions,
        deletions,
    }
}

const CONTEXT_LINES: usize = 3;

/// Build display rows from diff lines, collapsing unchanged regions.
///
/// Changed lines and their surrounding context (CONTEXT_LINES before/after)
/// are visible. Consecutive unchanged lines outside context are collapsed
/// into a single `Collapsed` row.
pub fn build_display_rows(
    lines: &[SplitLine],
    expanded_sections: &HashSet<usize>,
) -> Vec<DiffDisplayRow> {
    let len = lines.len();
    if len == 0 {
        return Vec::new();
    }

    // Mark which lines are "visible" (changed or within context of a change)
    let mut visible = vec![false; len];
    for (i, line) in lines.iter().enumerate() {
        if line.kind != SplitLineKind::Equal {
            let start = i.saturating_sub(CONTEXT_LINES);
            let end = (i + CONTEXT_LINES + 1).min(len);
            for v in &mut visible[start..end] {
                *v = true;
            }
        }
    }

    let mut rows = Vec::new();
    let mut i = 0;
    while i < len {
        if visible[i] {
            rows.push(DiffDisplayRow::Line(i));
            i += 1;
        } else {
            // Start of a collapsed section
            let start = i;
            while i < len && !visible[i] {
                i += 1;
            }
            let hidden_count = i - start;
            if expanded_sections.contains(&start) {
                // This section has been expanded - show header + all lines
                rows.push(DiffDisplayRow::ExpandedHeader {
                    hidden_count,
                    start_index: start,
                });
                for j in start..i {
                    rows.push(DiffDisplayRow::Line(j));
                }
            } else {
                rows.push(DiffDisplayRow::Collapsed {
                    hidden_count,
                    start_index: start,
                });
            }
        }
    }

    rows
}

fn strip_newline(s: &str) -> &str {
    s.strip_suffix('\n')
        .or_else(|| s.strip_suffix("\r\n"))
        .unwrap_or(s)
}
