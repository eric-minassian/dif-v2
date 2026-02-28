use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::state::{AppConfig, SavedProject, SavedSession};

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct RawSavedSession {
    id: Option<String>,
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worktree_path: Option<PathBuf>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct RawSavedProject {
    repo_root: Option<PathBuf>,
    display_name: Option<String>,
    last_known_valid: Option<bool>,
    #[serde(default)]
    sessions: Vec<RawSavedSession>,
    last_selected_session: Option<String>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct RawAppConfig {
    projects: Vec<RawSavedProject>,
    last_selected_repo: Option<PathBuf>,
    #[serde(default)]
    left_sidebar_width: Option<f32>,
    #[serde(default)]
    right_sidebar_width: Option<f32>,
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

            let (sessions, last_selected_session) = if sessions.is_empty() {
                (
                    vec![SavedSession {
                        id: "1".to_string(),
                        name: "Session 1".to_string(),
                        worktree_path: None,
                    }],
                    Some("1".to_string()),
                )
            } else {
                (sessions, item.last_selected_session)
            };

            Some(SavedProject {
                repo_root,
                display_name,
                last_known_valid: item.last_known_valid.unwrap_or(true),
                sessions,
                last_selected_session,
            })
        })
        .collect();

    Ok(AppConfig {
        projects,
        last_selected_repo: raw.last_selected_repo,
        left_sidebar_width: raw.left_sidebar_width,
        right_sidebar_width: raw.right_sidebar_width,
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

#[cfg(test)]
mod tests {
    use super::{RawAppConfig, RawSavedProject, RawSavedSession};
    use crate::state::{AppConfig, SavedProject, SavedSession};
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
                },
                RawSavedProject {
                    repo_root: None,
                    display_name: Some("broken".into()),
                    last_known_valid: Some(false),
                    sessions: vec![],
                    last_selected_session: None,
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

                    let (sessions, last_selected_session) = if sessions.is_empty() {
                        (
                            vec![SavedSession {
                                id: "1".to_string(),
                                name: "Session 1".to_string(),
                                worktree_path: None,
                            }],
                            Some("1".to_string()),
                        )
                    } else {
                        (sessions, item.last_selected_session)
                    };

                    Some(SavedProject {
                        repo_root: item.repo_root?,
                        display_name: item.display_name.unwrap_or_default(),
                        last_known_valid: item.last_known_valid.unwrap_or(true),
                        sessions,
                        last_selected_session,
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
    fn creates_default_session_for_old_config() {
        let raw = RawAppConfig {
            projects: vec![RawSavedProject {
                repo_root: Some(PathBuf::from("/tmp/old-project")),
                display_name: Some("old-project".into()),
                last_known_valid: Some(true),
                sessions: vec![],
                last_selected_session: None,
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

                    let (sessions, last_selected_session) = if sessions.is_empty() {
                        (
                            vec![SavedSession {
                                id: "1".to_string(),
                                name: "Session 1".to_string(),
                                worktree_path: None,
                            }],
                            Some("1".to_string()),
                        )
                    } else {
                        (sessions, item.last_selected_session)
                    };

                    Some(SavedProject {
                        repo_root: item.repo_root?,
                        display_name: item.display_name.unwrap_or_default(),
                        last_known_valid: item.last_known_valid.unwrap_or(true),
                        sessions,
                        last_selected_session,
                    })
                })
                .collect(),
            last_selected_repo: parsed.last_selected_repo,
            ..Default::default()
        };

        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].sessions.len(), 1);
        assert_eq!(config.projects[0].sessions[0].id, "1");
        assert_eq!(config.projects[0].sessions[0].name, "Session 1");
        assert_eq!(
            config.projects[0].last_selected_session,
            Some("1".to_string())
        );
    }
}
