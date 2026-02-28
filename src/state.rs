use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use crate::terminal_view::view::TerminalView;
use gpui::Entity;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SavedSession {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ProjectSettings {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workspace_init_commands: Vec<String>,
    #[serde(default)]
    pub enforce_conventional_commits: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SavedProject {
    pub repo_root: PathBuf,
    pub display_name: String,
    pub last_known_valid: bool,
    pub sessions: Vec<SavedSession>,
    pub last_selected_session: Option<String>,
    #[serde(default)]
    pub settings: ProjectSettings,
}

impl SavedProject {
    pub fn from_repo_root(repo_root: PathBuf) -> Self {
        let display_name = repo_root
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("repo")
            .to_string();

        Self {
            repo_root,
            display_name,
            last_known_valid: true,
            sessions: vec![],
            last_selected_session: None,
            settings: ProjectSettings::default(),
        }
    }

    pub fn next_session_id(&self) -> String {
        let max_id = self
            .sessions
            .iter()
            .filter_map(|s| s.id.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        (max_id + 1).to_string()
    }

    pub fn next_session_name(&self) -> String {
        let max_num = self
            .sessions
            .iter()
            .filter_map(|s| {
                s.name
                    .strip_prefix("Session ")
                    .and_then(|n| n.parse::<u64>().ok())
            })
            .max()
            .unwrap_or(0);
        format!("Session {}", max_num + 1)
    }
}

pub const DEFAULT_LEFT_SIDEBAR_WIDTH: f32 = 240.0;
pub const DEFAULT_RIGHT_SIDEBAR_WIDTH: f32 = 320.0;
pub const MIN_SIDEBAR_WIDTH: f32 = 140.0;
pub const MAX_SIDEBAR_WIDTH: f32 = 600.0;

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct AppConfig {
    pub projects: Vec<SavedProject>,
    pub last_selected_repo: Option<PathBuf>,
    #[serde(default)]
    pub left_sidebar_width: Option<f32>,
    #[serde(default)]
    pub right_sidebar_width: Option<f32>,
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
    pub lines: Vec<SplitLine>,
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
    pub pr_url: Option<String>,
    pub pr_merged: bool,
    pub pr_number: Option<u32>,
    pub pr_state: Option<String>,
    pub checks: Vec<CiCheck>,
    pub auto_merge_enabled: bool,
}

#[derive(Clone, Debug, Default)]
pub enum ActionPhase {
    #[default]
    Idle,
    Working(String),
    Error(String),
}

pub struct TerminalTab {
    pub id: String,
    pub view: Entity<TerminalView>,
}

pub struct SessionRuntime {
    pub main_terminal: Option<Entity<TerminalView>>,
    pub main_terminal_error: Option<String>,
    pub side_tabs: Vec<TerminalTab>,
    pub selected_side_tab: Option<String>,
    pub next_tab_id: u64,
    pub commit_message: String,
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self {
            main_terminal: None,
            main_terminal_error: None,
            side_tabs: Vec::new(),
            selected_side_tab: None,
            next_tab_id: 1,
            commit_message: String::new(),
        }
    }
}

pub struct ProjectRuntime {
    pub session_runtimes: HashMap<String, SessionRuntime>,
    pub git_snapshot: GitSnapshot,
    pub staged_files: HashSet<String>,
    pub branch_status: BranchStatus,
    pub repo_capabilities: RepoCapabilities,
    pub action_phase: ActionPhase,
}

impl Default for ProjectRuntime {
    fn default() -> Self {
        Self {
            session_runtimes: HashMap::new(),
            git_snapshot: GitSnapshot::default(),
            staged_files: HashSet::new(),
            branch_status: BranchStatus::default(),
            repo_capabilities: RepoCapabilities::default(),
            action_phase: ActionPhase::default(),
        }
    }
}

#[derive(Default)]
pub struct AppState {
    pub config: AppConfig,
    pub selected_repo: Option<PathBuf>,
    pub selected_session: Option<String>,
    pub runtimes: HashMap<PathBuf, ProjectRuntime>,
    pub flash_error: Option<String>,
    pub git_poll_generation: u64,
    pub viewing_diff: Option<DiffData>,
    pub viewing_settings: bool,
    pub left_sidebar_collapsed: bool,
    pub right_sidebar_collapsed: bool,
    pub left_sidebar_width: f32,
    pub right_sidebar_width: f32,
    pub collapsed_projects: HashSet<PathBuf>,
    pub resizing_sidebar: Option<ResizingSidebar>,
    pub update_status: UpdateStatus,
    pub checks_popover_open: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum ResizingSidebar {
    Left,
    Right,
}

#[derive(Clone, Debug, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    Available {
        version: String,
        download_url: String,
    },
    Updating,
    Error(String),
}
