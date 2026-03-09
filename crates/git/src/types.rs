use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Debug)]
pub struct SyntaxRun {
    pub len: usize,
    pub color: gpui::Hsla,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Clone, Debug)]
pub enum DiffDisplayRow {
    Line(usize),
    Collapsed {
        hidden_count: usize,
        start_index: usize,
    },
    ExpandedHeader {
        hidden_count: usize,
        start_index: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GitChange {
    pub path: String,
    pub status_code: String,
    pub sort_key: String,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct SplitLine {
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub old_text: String,
    pub new_text: String,
    pub kind: SplitLineKind,
    pub old_syntax_runs: Vec<SyntaxRun>,
    pub new_syntax_runs: Vec<SyntaxRun>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SplitLineKind {
    Equal,
    Delete,
    Insert,
    Replace,
}

#[derive(Clone, Debug)]
pub struct DiffData {
    pub file_path: String,
    pub lines: Arc<Vec<SplitLine>>,
    pub display_rows: Arc<Vec<DiffDisplayRow>>,
    pub expanded_sections: HashSet<usize>,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Clone, Debug, Default)]
pub struct GitSnapshot {
    pub changes: Vec<GitChange>,
    pub last_refresh: Option<Instant>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CheckBucket {
    Pass,
    Fail,
    Pending,
    Skipping,
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CiCheck {
    pub name: String,
    pub bucket: CheckBucket,
    pub workflow: String,
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RepoCapabilities {
    pub auto_merge_allowed: bool,
    pub rebase_merge_allowed: bool,
}

impl Default for RepoCapabilities {
    fn default() -> Self {
        Self {
            auto_merge_allowed: false,
            rebase_merge_allowed: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BranchStatus {
    pub commits_ahead: u32,
    pub commits_behind: u32,
    pub pr_url: Option<String>,
    pub pr_merged: bool,
    pub pr_number: Option<u32>,
    pub pr_state: Option<String>,
    pub checks: Vec<CiCheck>,
    pub auto_merge_enabled: bool,
    pub branch_name: Option<String>,
}
