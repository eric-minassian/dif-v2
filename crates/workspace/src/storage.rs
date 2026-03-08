use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::config::{AppConfig, ProjectSettings, SavedProject, SavedSession};

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct RawSavedSession {
    id: Option<String>,
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worktree_path: Option<PathBuf>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct RawProjectSettings {
    #[serde(default)]
    workspace_init_commands: Option<Vec<String>>,
    #[serde(default)]
    enforce_conventional_commits: Option<bool>,
    #[serde(default)]
    auto_merge: Option<bool>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct RawSavedProject {
    repo_root: Option<PathBuf>,
    display_name: Option<String>,
    last_known_valid: Option<bool>,
    #[serde(default)]
    sessions: Vec<RawSavedSession>,
    last_selected_session: Option<String>,
    #[serde(default)]
    settings: Option<RawProjectSettings>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct RawAppConfig {
    projects: Vec<RawSavedProject>,
    last_selected_repo: Option<PathBuf>,
    #[serde(default)]
    left_sidebar_width: Option<f32>,
    #[serde(default)]
    right_sidebar_width: Option<f32>,
    #[serde(default)]
    bottom_panel_height: Option<f32>,
}

pub fn load_config() -> Result<AppConfig> {
    let path = config_path()?;
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(AppConfig::default());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };

    let raw: RawAppConfig = serde_json::from_str(&contents).unwrap_or_default();

    let projects = raw
        .projects
        .into_iter()
        .filter_map(|item| {
            let repo_root = item.repo_root?;
            let display_name = item.display_name.unwrap_or_else(|| {
                repo_root
                    .file_name()
                    .and_then(|value| value.to_str())
                    .filter(|value| !value.is_empty())
                    .unwrap_or("repo")
                    .to_string()
            });

            let sessions: Vec<SavedSession> = item
                .sessions
                .into_iter()
                .filter_map(|s| {
                    Some(SavedSession {
                        id: s.id?,
                        name: s.name.unwrap_or_else(|| "Session".to_string()),
                        worktree_path: s.worktree_path,
                    })
                })
                .collect();

            let last_selected_session = item.last_selected_session;

            let settings = item
                .settings
                .map(|raw| ProjectSettings {
                    workspace_init_commands: raw.workspace_init_commands.unwrap_or_default(),
                    enforce_conventional_commits: raw.enforce_conventional_commits.unwrap_or(false),
                })
                .unwrap_or_default();

            Some(SavedProject {
                repo_root,
                display_name,
                last_known_valid: item.last_known_valid.unwrap_or(true),
                sessions,
                last_selected_session,
                settings,
            })
        })
        .collect();

    Ok(AppConfig {
        projects,
        last_selected_repo: raw.last_selected_repo,
        left_sidebar_width: raw.left_sidebar_width,
        right_sidebar_width: raw.right_sidebar_width,
        bottom_panel_height: raw.bottom_panel_height,
    })
}

pub fn save_config(config: &AppConfig) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let contents = serde_json::to_string_pretty(config)?;
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn config_path() -> Result<PathBuf> {
    let dirs =
        ProjectDirs::from("", "", "dif").context("failed to resolve app support directory")?;
    Ok(dirs.config_dir().join("config.json"))
}

pub fn keybindings_path() -> Result<PathBuf> {
    let dirs =
        ProjectDirs::from("", "", "dif").context("failed to resolve app support directory")?;
    Ok(dirs.config_dir().join("keybindings.json"))
}

pub fn load_keybindings() -> Result<Vec<crate::keybindings::KeybindingEntry>> {
    let path = keybindings_path()?;
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(crate::keybindings::default_keybindings());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", path.display()));
        }
    };

    let entries: Vec<crate::keybindings::KeybindingEntry> = serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(entries)
}

pub fn ensure_keybindings_file() -> Result<PathBuf> {
    let path = keybindings_path()?;
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let defaults = crate::keybindings::default_keybindings();
        let contents = serde_json::to_string_pretty(&defaults)?;
        fs::write(&path, contents)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, ProjectSettings, SavedProject, SavedSession};
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    #[test]
    fn skips_invalid_saved_projects() {
        let raw = RawAppConfig {
            projects: vec![
                RawSavedProject {
                    repo_root: Some(PathBuf::from("/tmp/one")),
                    display_name: Some("one".into()),
                    last_known_valid: Some(true),
                    sessions: vec![RawSavedSession {
                        id: Some("1".into()),
                        name: Some("Session 1".into()),
                        worktree_path: Some(PathBuf::from("/tmp/worktree-1")),
                    }],
                    last_selected_session: Some("1".into()),
                    ..Default::default()
                },
                RawSavedProject {
                    repo_root: None,
                    display_name: Some("broken".into()),
                    last_known_valid: Some(false),
                    sessions: vec![],
                    last_selected_session: None,
                    ..Default::default()
                },
            ],
            last_selected_repo: Some(PathBuf::from("/tmp/one")),
            ..Default::default()
        };

        let json = serde_json::to_string(&raw).unwrap();
        let parsed: RawAppConfig = serde_json::from_str(&json).unwrap();
        let config = AppConfig {
            projects: parsed
                .projects
                .into_iter()
                .filter_map(|item| {
                    let sessions: Vec<SavedSession> = item
                        .sessions
                        .into_iter()
                        .filter_map(|s| {
                            Some(SavedSession {
                                id: s.id?,
                                name: s.name.unwrap_or_default(),
                                worktree_path: s.worktree_path,
                            })
                        })
                        .collect();

                    let last_selected_session = item.last_selected_session;

                    Some(SavedProject {
                        repo_root: item.repo_root?,
                        display_name: item.display_name.unwrap_or_default(),
                        last_known_valid: item.last_known_valid.unwrap_or(true),
                        sessions,
                        last_selected_session,
                        settings: ProjectSettings::default(),
                    })
                })
                .collect(),
            last_selected_repo: parsed.last_selected_repo,
            ..Default::default()
        };

        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].display_name, "one");
        assert_eq!(config.projects[0].sessions.len(), 1);
        assert_eq!(config.projects[0].sessions[0].name, "Session 1");
        assert_eq!(
            config.projects[0].sessions[0].worktree_path,
            Some(PathBuf::from("/tmp/worktree-1"))
        );
    }

    #[test]
    fn keeps_empty_sessions_for_old_config() {
        let raw = RawAppConfig {
            projects: vec![RawSavedProject {
                repo_root: Some(PathBuf::from("/tmp/old-project")),
                display_name: Some("old-project".into()),
                last_known_valid: Some(true),
                sessions: vec![],
                last_selected_session: None,
                ..Default::default()
            }],
            last_selected_repo: None,
            ..Default::default()
        };

        let json = serde_json::to_string(&raw).unwrap();
        let parsed: RawAppConfig = serde_json::from_str(&json).unwrap();
        let config = AppConfig {
            projects: parsed
                .projects
                .into_iter()
                .filter_map(|item| {
                    let sessions: Vec<SavedSession> = item
                        .sessions
                        .into_iter()
                        .filter_map(|s| {
                            Some(SavedSession {
                                id: s.id?,
                                name: s.name.unwrap_or_default(),
                                worktree_path: s.worktree_path,
                            })
                        })
                        .collect();

                    let last_selected_session = item.last_selected_session;

                    Some(SavedProject {
                        repo_root: item.repo_root?,
                        display_name: item.display_name.unwrap_or_default(),
                        last_known_valid: item.last_known_valid.unwrap_or(true),
                        sessions,
                        last_selected_session,
                        settings: ProjectSettings::default(),
                    })
                })
                .collect(),
            last_selected_repo: parsed.last_selected_repo,
            ..Default::default()
        };

        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].sessions.len(), 0);
        assert_eq!(config.projects[0].last_selected_session, None);
    }

    #[test]
    fn lenient_deserialization_handles_extra_fields() {
        let json = r#"{
            "projects": [{
                "repo_root": "/tmp/repo",
                "display_name": "repo",
                "last_known_valid": true,
                "sessions": [],
                "unknown_future_field": "should be ignored"
            }],
            "last_selected_repo": null,
            "some_new_setting": 42
        }"#;

        // RawAppConfig should tolerate unknown fields (serde default behavior)
        let raw: RawAppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(raw.projects.len(), 1);
        assert_eq!(raw.projects[0].repo_root, Some(PathBuf::from("/tmp/repo")));
    }

    #[test]
    fn lenient_deserialization_handles_missing_optional_fields() {
        let json = r#"{
            "projects": [{
                "repo_root": "/tmp/repo"
            }]
        }"#;

        let raw: RawAppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(raw.projects.len(), 1);
        assert_eq!(raw.projects[0].display_name, None);
        assert_eq!(raw.projects[0].last_known_valid, None);
        assert!(raw.projects[0].sessions.is_empty());
    }

    #[test]
    fn sessions_without_id_are_filtered_out() {
        let raw = RawAppConfig {
            projects: vec![RawSavedProject {
                repo_root: Some(PathBuf::from("/tmp/repo")),
                display_name: Some("repo".into()),
                last_known_valid: Some(true),
                sessions: vec![
                    RawSavedSession {
                        id: None, // missing id → should be filtered
                        name: Some("bad session".into()),
                        worktree_path: None,
                    },
                    RawSavedSession {
                        id: Some("1".into()),
                        name: Some("good session".into()),
                        worktree_path: None,
                    },
                ],
                last_selected_session: None,
                ..Default::default()
            }],
            last_selected_repo: None,
            ..Default::default()
        };

        let json = serde_json::to_string(&raw).unwrap();
        let parsed: RawAppConfig = serde_json::from_str(&json).unwrap();

        let sessions: Vec<SavedSession> = parsed.projects[0]
            .sessions
            .iter()
            .filter_map(|s| {
                Some(SavedSession {
                    id: s.id.clone()?,
                    name: s.name.clone().unwrap_or_default(),
                    worktree_path: s.worktree_path.clone(),
                })
            })
            .collect();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name, "good session");
    }

    #[test]
    fn session_name_defaults_to_session() {
        let raw_session = RawSavedSession {
            id: Some("1".into()),
            name: None,
            worktree_path: None,
        };

        let session = SavedSession {
            id: raw_session.id.unwrap(),
            name: raw_session.name.unwrap_or_else(|| "Session".to_string()),
            worktree_path: raw_session.worktree_path,
        };

        assert_eq!(session.name, "Session");
    }

    #[test]
    fn save_and_load_roundtrip_via_tempfile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        let config = AppConfig {
            projects: vec![SavedProject {
                repo_root: PathBuf::from("/tmp/test-repo"),
                display_name: "test-repo".to_string(),
                last_known_valid: true,
                sessions: vec![SavedSession {
                    id: "1".to_string(),
                    name: "dev".to_string(),
                    worktree_path: None,
                }],
                last_selected_session: Some("1".to_string()),
                settings: ProjectSettings {
                    workspace_init_commands: vec!["npm install".to_string()],
                    enforce_conventional_commits: true,
                },
            }],
            last_selected_repo: Some(PathBuf::from("/tmp/test-repo")),
            left_sidebar_width: Some(280.0),
            right_sidebar_width: None,
            bottom_panel_height: Some(180.0),
        };

        let contents = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(&path, &contents).unwrap();

        let loaded_contents = std::fs::read_to_string(&path).unwrap();
        let raw: RawAppConfig = serde_json::from_str(&loaded_contents).unwrap();

        assert_eq!(raw.projects.len(), 1);
        assert_eq!(
            raw.projects[0].repo_root,
            Some(PathBuf::from("/tmp/test-repo"))
        );
        assert_eq!(raw.left_sidebar_width, Some(280.0));
        assert_eq!(raw.right_sidebar_width, None);
        assert_eq!(raw.bottom_panel_height, Some(180.0));

        let settings = raw.projects[0].settings.as_ref().unwrap();
        assert_eq!(settings.enforce_conventional_commits, Some(true));
        assert_eq!(
            settings.workspace_init_commands,
            Some(vec!["npm install".to_string()])
        );
    }

    #[test]
    fn corrupt_json_falls_back_to_default() {
        let raw: RawAppConfig = serde_json::from_str("not json at all").unwrap_or_default();
        assert!(raw.projects.is_empty());
        assert_eq!(raw.last_selected_repo, None);
    }
}
