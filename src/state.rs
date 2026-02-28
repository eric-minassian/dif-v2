use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use gpui::Entity;
use gpui_terminal::TerminalView;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SavedProject {
    pub repo_root: PathBuf,
    pub display_name: String,
    pub last_known_valid: bool,
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
        }
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
}

#[derive(Clone, Debug, Default)]
pub struct GitSnapshot {
    pub changes: Vec<GitChange>,
    pub last_refresh: Option<Instant>,
    pub last_error: Option<String>,
}

pub struct TerminalPair {
    pub main: Entity<TerminalView>,
    pub side: Entity<TerminalView>,
}

pub struct ProjectRuntime {
    pub terminals: Option<TerminalPair>,
    pub terminal_error: Option<String>,
    pub git_snapshot: GitSnapshot,
}

impl Default for ProjectRuntime {
    fn default() -> Self {
        Self {
            terminals: None,
            terminal_error: None,
            git_snapshot: GitSnapshot::default(),
        }
    }
}

#[derive(Default)]
pub struct AppState {
    pub config: AppConfig,
    pub selected_repo: Option<PathBuf>,
    pub runtimes: HashMap<PathBuf, ProjectRuntime>,
    pub flash_error: Option<String>,
    pub git_poll_generation: u64,
}
