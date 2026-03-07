use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use gpui::Entity;

use terminal::view::TerminalView;

use crate::config::AppConfig;
use crate::pane_group::PaneGroup;
use crate::ui_state::{ResizingSidebar, UpdateStatus};
use git::{BranchStatus, DiffData, GitSnapshot, RepoCapabilities};

#[derive(Clone, Debug, Default)]
pub enum ActionPhase {
    #[default]
    Idle,
    Working(String),
    Error(String),
}

/// A single tab in the bottom panel. Each tab has its own split layout.
pub struct TerminalTab {
    pub pane_group: PaneGroup,
    /// The currently focused terminal within this tab's split layout.
    pub active_pane: Option<Entity<TerminalView>>,
    /// When zoomed, we stash the full pane group and show only the zoomed pane.
    pub zoomed_pane_group: Option<PaneGroup>,
}

pub struct SessionRuntime {
    pub main_terminal: Option<Entity<TerminalView>>,
    pub main_terminal_error: Option<String>,
    /// The terminal tabs in the bottom panel. Each tab has its own split layout.
    pub tabs: Vec<TerminalTab>,
    /// The currently active tab index.
    pub active_tab_index: usize,
    pub cached_branch_status: Option<BranchStatus>,
    pub cached_repo_capabilities: Option<RepoCapabilities>,
}

impl SessionRuntime {
    /// The currently active terminal tab.
    pub fn active_tab(&self) -> Option<&TerminalTab> {
        self.tabs.get(self.active_tab_index)
    }

    /// The currently active terminal tab (mutable).
    pub fn active_tab_mut(&mut self) -> Option<&mut TerminalTab> {
        self.tabs.get_mut(self.active_tab_index)
    }
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self {
            main_terminal: None,
            main_terminal_error: None,
            tabs: Vec::new(),
            active_tab_index: 0,
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
    pub bottom_panel_collapsed: bool,
    pub left_sidebar_width: f32,
    pub right_sidebar_width: f32,
    pub bottom_panel_height: f32,
    pub collapsed_projects: HashSet<PathBuf>,
    pub resizing_sidebar: Option<ResizingSidebar>,
    pub update_status: UpdateStatus,
    pub checks_popover_open: bool,
    pub viewing_help: bool,
}
