use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui::Entity;

use terminal::view::TerminalView;

use crate::config::AppConfig;
use crate::ui_state::{ResizingSidebar, UpdateStatus};
use git::{BranchStatus, DiffData, GitSnapshot, RepoCapabilities};

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
            cached_branch_status: None,
            cached_repo_capabilities: None,
        }
    }
}

#[derive(Default)]
pub struct ProjectRuntime {
    pub session_runtimes: HashMap<String, SessionRuntime>,
    pub git_snapshot: GitSnapshot,
    pub staged_files: HashSet<String>,
    pub branch_status: BranchStatus,
    pub repo_capabilities: RepoCapabilities,
    pub action_phase: ActionPhase,
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
    pub viewing_help: bool,
}
