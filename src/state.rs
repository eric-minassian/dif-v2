use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use gpui::Entity;
use gpui_terminal::TerminalView;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SavedSession {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SavedProject {
    pub repo_root: PathBuf,
    pub display_name: String,
    pub last_known_valid: bool,
    pub sessions: Vec<SavedSession>,
    pub last_selected_session: Option<String>,
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
            sessions: vec![SavedSession {
                id: "1".to_string(),
                name: "Session 1".to_string(),
            }],
            last_selected_session: Some("1".to_string()),
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

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct AppConfig {
    pub projects: Vec<SavedProject>,
    pub last_selected_repo: Option<PathBuf>,
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
pub struct DiffLine {
    pub left_number: Option<u32>,
    pub left_text: String,
    pub right_number: Option<u32>,
    pub right_text: String,
    pub kind: DiffLineKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
    Modified,
}

#[derive(Clone, Debug)]
pub struct DiffData {
    pub file_path: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Clone, Debug, Default)]
pub struct GitSnapshot {
    pub changes: Vec<GitChange>,
    pub last_refresh: Option<Instant>,
    pub last_error: Option<String>,
}

pub struct TerminalTab {
    pub id: String,
    pub name: String,
    pub view: Entity<TerminalView>,
}

pub struct SessionRuntime {
    pub main_terminal: Option<Entity<TerminalView>>,
    pub main_terminal_error: Option<String>,
    pub side_tabs: Vec<TerminalTab>,
    pub selected_side_tab: Option<String>,
    pub next_tab_id: u64,
}

impl Default for SessionRuntime {
    fn default() -> Self {
        Self {
            main_terminal: None,
            main_terminal_error: None,
            side_tabs: Vec::new(),
            selected_side_tab: None,
            next_tab_id: 1,
        }
    }
}

pub struct ProjectRuntime {
    pub session_runtimes: HashMap<String, SessionRuntime>,
    pub git_snapshot: GitSnapshot,
}

impl Default for ProjectRuntime {
    fn default() -> Self {
        Self {
            session_runtimes: HashMap::new(),
            git_snapshot: GitSnapshot::default(),
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
}
