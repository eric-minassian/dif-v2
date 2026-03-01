use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui::Entity;

use crate::terminal_view::view::TerminalView;

use super::config::AppConfig;
use super::git::{BranchStatus, DiffData, GitSnapshot, RepoCapabilities};
use super::ui::{ResizingSidebar, UpdateStatus};

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
    pub cached_branch_status: Option<BranchStatus>,
    pub cached_repo_capabilities: Option<RepoCapabilities>,
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
            cached_branch_status: None,
            cached_repo_capabilities: None,
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
    pub cmd_held: bool,
    pub viewing_help: bool,
}
