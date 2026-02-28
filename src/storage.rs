use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

use crate::state::{AppConfig, SavedProject};

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct RawSavedProject {
    repo_root: Option<PathBuf>,
    display_name: Option<String>,
    last_known_valid: Option<bool>,
}

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct RawAppConfig {
    projects: Vec<RawSavedProject>,
    last_selected_repo: Option<PathBuf>,
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

            Some(SavedProject {
                repo_root,
                display_name,
                last_known_valid: item.last_known_valid.unwrap_or(true),
            })
        })
        .collect();

    Ok(AppConfig {
        projects,
        last_selected_repo: raw.last_selected_repo,
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
    use super::{RawAppConfig, RawSavedProject};
    use crate::state::{AppConfig, SavedProject};
    use std::path::PathBuf;

    #[test]
    fn skips_invalid_saved_projects() {
        let raw = RawAppConfig {
            projects: vec![
                RawSavedProject {
                    repo_root: Some(PathBuf::from("/tmp/one")),
                    display_name: Some("one".into()),
                    last_known_valid: Some(true),
                },
                RawSavedProject {
                    repo_root: None,
                    display_name: Some("broken".into()),
                    last_known_valid: Some(false),
                },
            ],
            last_selected_repo: Some(PathBuf::from("/tmp/one")),
        };

        let json = serde_json::to_string(&raw).unwrap();
        let parsed: RawAppConfig = serde_json::from_str(&json).unwrap();
        let config = AppConfig {
            projects: parsed
                .projects
                .into_iter()
                .filter_map(|item| {
                    Some(SavedProject {
                        repo_root: item.repo_root?,
                        display_name: item.display_name.unwrap_or_default(),
                        last_known_valid: item.last_known_valid.unwrap_or(true),
                    })
                })
                .collect(),
            last_selected_repo: parsed.last_selected_repo,
        };

        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].display_name, "one");
    }
}
