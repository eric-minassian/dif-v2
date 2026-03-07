use std::path::PathBuf;

pub const DEFAULT_LEFT_SIDEBAR_WIDTH: f32 = 240.0;
pub const DEFAULT_RIGHT_SIDEBAR_WIDTH: f32 = 320.0;
pub const MIN_SIDEBAR_WIDTH: f32 = 140.0;
pub const MAX_SIDEBAR_WIDTH: f32 = 600.0;

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
}

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct AppConfig {
    pub projects: Vec<SavedProject>,
    pub last_selected_repo: Option<PathBuf>,
    #[serde(default)]
    pub left_sidebar_width: Option<f32>,
    #[serde(default)]
    pub right_sidebar_width: Option<f32>,
}
