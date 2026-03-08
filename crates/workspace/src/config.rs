use std::path::PathBuf;

pub const DEFAULT_LEFT_SIDEBAR_WIDTH: f32 = 240.0;
pub const DEFAULT_RIGHT_SIDEBAR_WIDTH: f32 = 320.0;
pub const MIN_SIDEBAR_WIDTH: f32 = 140.0;
pub const MAX_SIDEBAR_WIDTH: f32 = 600.0;

pub const DEFAULT_BOTTOM_PANEL_HEIGHT: f32 = 250.0;
pub const MIN_BOTTOM_PANEL_HEIGHT: f32 = 100.0;
pub const MAX_BOTTOM_PANEL_HEIGHT: f32 = 600.0;

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
    #[serde(default)]
    pub bottom_panel_height: Option<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn from_repo_root_derives_display_name() {
        let project = SavedProject::from_repo_root(PathBuf::from("/home/user/my-project"));
        assert_eq!(project.display_name, "my-project");
        assert_eq!(project.repo_root, PathBuf::from("/home/user/my-project"));
        assert!(project.last_known_valid);
        assert!(project.sessions.is_empty());
        assert_eq!(project.last_selected_session, None);
    }

    #[test]
    fn from_repo_root_handles_root_path() {
        let project = SavedProject::from_repo_root(PathBuf::from("/"));
        assert_eq!(project.display_name, "repo");
    }

    #[test]
    fn next_session_id_empty() {
        let project = SavedProject::from_repo_root(PathBuf::from("/tmp/repo"));
        assert_eq!(project.next_session_id(), "1");
    }

    #[test]
    fn next_session_id_increments() {
        let mut project = SavedProject::from_repo_root(PathBuf::from("/tmp/repo"));
        project.sessions = vec![
            SavedSession {
                id: "1".to_string(),
                name: "first".to_string(),
                worktree_path: None,
            },
            SavedSession {
                id: "3".to_string(),
                name: "third".to_string(),
                worktree_path: None,
            },
        ];
        assert_eq!(project.next_session_id(), "4");
    }

    #[test]
    fn next_session_id_skips_non_numeric() {
        let mut project = SavedProject::from_repo_root(PathBuf::from("/tmp/repo"));
        project.sessions = vec![
            SavedSession {
                id: "abc".to_string(),
                name: "alpha".to_string(),
                worktree_path: None,
            },
            SavedSession {
                id: "2".to_string(),
                name: "second".to_string(),
                worktree_path: None,
            },
        ];
        assert_eq!(project.next_session_id(), "3");
    }

    #[test]
    fn app_config_serialization_roundtrip() {
        let config = AppConfig {
            projects: vec![SavedProject {
                repo_root: PathBuf::from("/tmp/my-repo"),
                display_name: "my-repo".to_string(),
                last_known_valid: true,
                sessions: vec![SavedSession {
                    id: "1".to_string(),
                    name: "main".to_string(),
                    worktree_path: Some(PathBuf::from("/tmp/worktree")),
                }],
                last_selected_session: Some("1".to_string()),
                settings: ProjectSettings {
                    workspace_init_commands: vec!["echo hi".to_string()],
                    enforce_conventional_commits: true,
                },
            }],
            last_selected_repo: Some(PathBuf::from("/tmp/my-repo")),
            left_sidebar_width: Some(250.0),
            right_sidebar_width: Some(300.0),
            bottom_panel_height: Some(200.0),
        };

        let json = serde_json::to_string_pretty(&config).unwrap();
        let restored: AppConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.projects.len(), 1);
        assert_eq!(restored.projects[0].repo_root, config.projects[0].repo_root);
        assert_eq!(
            restored.projects[0].display_name,
            config.projects[0].display_name
        );
        assert_eq!(
            restored.projects[0].settings.enforce_conventional_commits,
            true
        );
        assert_eq!(
            restored.projects[0].settings.workspace_init_commands,
            vec!["echo hi"]
        );
        assert_eq!(restored.left_sidebar_width, Some(250.0));
        assert_eq!(restored.right_sidebar_width, Some(300.0));
        assert_eq!(restored.bottom_panel_height, Some(200.0));
    }

    #[test]
    fn project_settings_defaults() {
        let settings = ProjectSettings::default();
        assert!(settings.workspace_init_commands.is_empty());
        assert!(!settings.enforce_conventional_commits);
    }

    #[test]
    fn saved_session_with_worktree() {
        let session = SavedSession {
            id: "1".to_string(),
            name: "feature".to_string(),
            worktree_path: Some(PathBuf::from("/tmp/wt")),
        };
        let json = serde_json::to_string(&session).unwrap();
        let restored: SavedSession = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.worktree_path, Some(PathBuf::from("/tmp/wt")));
    }

    #[test]
    fn saved_session_without_worktree_omits_field() {
        let session = SavedSession {
            id: "1".to_string(),
            name: "main".to_string(),
            worktree_path: None,
        };
        let json = serde_json::to_string(&session).unwrap();
        assert!(!json.contains("worktree_path"));
    }

    #[test]
    fn config_constants_are_reasonable() {
        assert!(MIN_SIDEBAR_WIDTH < DEFAULT_LEFT_SIDEBAR_WIDTH);
        assert!(DEFAULT_LEFT_SIDEBAR_WIDTH < MAX_SIDEBAR_WIDTH);
        assert!(MIN_SIDEBAR_WIDTH < DEFAULT_RIGHT_SIDEBAR_WIDTH);
        assert!(DEFAULT_RIGHT_SIDEBAR_WIDTH < MAX_SIDEBAR_WIDTH);
        assert!(MIN_BOTTOM_PANEL_HEIGHT < DEFAULT_BOTTOM_PANEL_HEIGHT);
        assert!(DEFAULT_BOTTOM_PANEL_HEIGHT < MAX_BOTTOM_PANEL_HEIGHT);
    }
}
