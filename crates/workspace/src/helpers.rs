use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui::CursorStyle;

use crate::config::{AppConfig, SavedProject};
use crate::runtime::ProjectRuntime;
use crate::ui_state::ResizingSidebar;
use git::GitChange;
use ui::prelude::*;

use crate::WorkspaceView;

impl WorkspaceView {
    /// Get the session name (used as commit message) for the currently selected session.
    pub(crate) fn session_name(&self, repo: &Path) -> String {
        let session_id = self.state.selected_session.as_deref().unwrap_or_default();
        self.state
            .config
            .projects
            .iter()
            .find(|p| p.repo_root.as_path() == repo)
            .and_then(|p| p.sessions.iter().find(|s| s.id == session_id))
            .map(|s| s.name.clone())
            .unwrap_or_default()
    }

    /// Check whether the project at `repo` enforces conventional commit format.
    pub(crate) fn enforces_conventional_commits(&self, repo: &Path) -> bool {
        self.state
            .config
            .projects
            .iter()
            .find(|p| p.repo_root.as_path() == repo)
            .is_some_and(|p| p.settings.enforce_conventional_commits)
    }
}

pub(crate) fn resize_handle(
    id: &'static str,
    cx: &mut Context<WorkspaceView>,
    side: ResizingSidebar,
) -> impl IntoElement {
    let t = theme();
    div()
        .id(id)
        .w(px(2.))
        .h_full()
        .flex_shrink_0()
        .bg(t.border_subtle)
        .cursor(CursorStyle::ResizeLeftRight)
        .hover(|style| style.bg(t.accent_blue))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _window, cx| {
                this.state.resizing_sidebar = Some(side);
                cx.notify();
            }),
        )
}

pub(crate) fn resize_handle_horizontal(
    id: &'static str,
    cx: &mut Context<WorkspaceView>,
) -> impl IntoElement {
    let t = theme();
    div()
        .id(id)
        .w_full()
        .h(px(2.))
        .flex_shrink_0()
        .bg(t.border_subtle)
        .cursor(CursorStyle::ResizeUpDown)
        .hover(|style| style.bg(t.accent_blue))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _window, cx| {
                this.state.resizing_sidebar = Some(ResizingSidebar::Bottom);
                cx.notify();
            }),
        )
}

pub(crate) fn refresh_project_validity(projects: &mut [SavedProject]) {
    for project in projects {
        project.last_known_valid = git::is_valid_repo(&project.repo_root);
    }
}

/// Returns `true` if the snapshot actually changed (so callers know whether to re-render).
pub(crate) fn apply_git_snapshot(
    runtime: &mut ProjectRuntime,
    snapshot: &Result<Vec<GitChange>, String>,
) -> bool {
    runtime.git_snapshot.last_refresh = Some(Instant::now());

    match snapshot {
        Ok(changes) => {
            let changed = runtime.git_snapshot.changes != *changes
                || runtime.git_snapshot.last_error.is_some();

            // Collect current file paths from the new snapshot (borrow, don't clone)
            let current_paths: std::collections::HashSet<&str> =
                changes.iter().map(|c| c.path.as_str()).collect();

            // Auto-stage new files that weren't in the previous snapshot
            for change in changes {
                if !runtime
                    .git_snapshot
                    .changes
                    .iter()
                    .any(|old| old.path == change.path)
                {
                    runtime.staged_files.insert(change.path.clone());
                }
            }

            // Remove staged paths that no longer appear in changes
            runtime
                .staged_files
                .retain(|p| current_paths.contains(p.as_str()));

            runtime.git_snapshot.changes = changes.clone();
            runtime.git_snapshot.last_error = None;
            changed
        }
        Err(error) => {
            let changed = runtime.git_snapshot.last_error.as_ref() != Some(error);
            runtime.git_snapshot.last_error = Some(error.clone());
            changed
        }
    }
}

pub(crate) fn pick_initial_selection(config: &AppConfig) -> Option<PathBuf> {
    if let Some(last) = &config.last_selected_repo
        && config
            .projects
            .iter()
            .any(|project| project.repo_root == *last && project.last_known_valid)
    {
        return Some(last.clone());
    }

    config
        .projects
        .iter()
        .find(|project| project.last_known_valid)
        .map(|project| project.repo_root.clone())
}

pub(crate) fn pick_initial_session(config: &AppConfig, repo: &Path) -> Option<String> {
    let project = config.projects.iter().find(|p| p.repo_root == repo)?;
    if let Some(last) = &project.last_selected_session
        && project.sessions.iter().any(|s| s.id == *last)
    {
        return Some(last.clone());
    }
    project.sessions.first().map(|s| s.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ProjectSettings, SavedSession};
    use crate::runtime::ProjectRuntime;
    use git::GitChange;
    use pretty_assertions::assert_eq;

    fn make_project(repo_root: &str, valid: bool) -> SavedProject {
        SavedProject {
            repo_root: PathBuf::from(repo_root),
            display_name: repo_root.to_string(),
            last_known_valid: valid,
            sessions: vec![],
            last_selected_session: None,
            settings: ProjectSettings::default(),
        }
    }

    fn make_project_with_sessions(repo_root: &str, session_ids: &[&str]) -> SavedProject {
        let mut p = make_project(repo_root, true);
        p.sessions = session_ids
            .iter()
            .map(|id| SavedSession {
                id: id.to_string(),
                name: format!("Session {id}"),
                worktree_path: None,
            })
            .collect();
        p
    }

    fn make_change(path: &str) -> GitChange {
        GitChange {
            path: path.to_string(),
            status_code: " M".to_string(),
            sort_key: path.to_string(),
            additions: Some(1),
            deletions: Some(0),
        }
    }

    // ── pick_initial_selection ──────────────────────────────────────────

    #[test]
    fn pick_initial_selection_returns_none_for_empty_config() {
        let config = AppConfig::default();
        assert_eq!(pick_initial_selection(&config), None);
    }

    #[test]
    fn pick_initial_selection_returns_last_selected_if_valid() {
        let config = AppConfig {
            projects: vec![make_project("/repo/a", true), make_project("/repo/b", true)],
            last_selected_repo: Some(PathBuf::from("/repo/b")),
            ..Default::default()
        };
        assert_eq!(
            pick_initial_selection(&config),
            Some(PathBuf::from("/repo/b"))
        );
    }

    #[test]
    fn pick_initial_selection_skips_invalid_last_selected() {
        let config = AppConfig {
            projects: vec![
                make_project("/repo/a", true),
                make_project("/repo/b", false), // invalid
            ],
            last_selected_repo: Some(PathBuf::from("/repo/b")),
            ..Default::default()
        };
        // Falls back to first valid project
        assert_eq!(
            pick_initial_selection(&config),
            Some(PathBuf::from("/repo/a"))
        );
    }

    #[test]
    fn pick_initial_selection_falls_back_to_first_valid() {
        let config = AppConfig {
            projects: vec![
                make_project("/repo/a", false),
                make_project("/repo/b", true),
            ],
            last_selected_repo: None,
            ..Default::default()
        };
        assert_eq!(
            pick_initial_selection(&config),
            Some(PathBuf::from("/repo/b"))
        );
    }

    #[test]
    fn pick_initial_selection_returns_none_if_all_invalid() {
        let config = AppConfig {
            projects: vec![
                make_project("/repo/a", false),
                make_project("/repo/b", false),
            ],
            last_selected_repo: None,
            ..Default::default()
        };
        assert_eq!(pick_initial_selection(&config), None);
    }

    // ── pick_initial_session ───────────────────────────────────────────

    #[test]
    fn pick_initial_session_returns_none_for_missing_repo() {
        let config = AppConfig {
            projects: vec![make_project_with_sessions("/repo/a", &["1", "2"])],
            ..Default::default()
        };
        assert_eq!(
            pick_initial_session(&config, &PathBuf::from("/repo/missing")),
            None
        );
    }

    #[test]
    fn pick_initial_session_returns_last_selected() {
        let mut project = make_project_with_sessions("/repo/a", &["1", "2", "3"]);
        project.last_selected_session = Some("2".to_string());
        let config = AppConfig {
            projects: vec![project],
            ..Default::default()
        };
        assert_eq!(
            pick_initial_session(&config, &PathBuf::from("/repo/a")),
            Some("2".to_string())
        );
    }

    #[test]
    fn pick_initial_session_falls_back_to_first() {
        let config = AppConfig {
            projects: vec![make_project_with_sessions("/repo/a", &["5", "10"])],
            ..Default::default()
        };
        assert_eq!(
            pick_initial_session(&config, &PathBuf::from("/repo/a")),
            Some("5".to_string())
        );
    }

    #[test]
    fn pick_initial_session_returns_none_for_no_sessions() {
        let config = AppConfig {
            projects: vec![make_project("/repo/a", true)],
            ..Default::default()
        };
        assert_eq!(
            pick_initial_session(&config, &PathBuf::from("/repo/a")),
            None
        );
    }

    #[test]
    fn pick_initial_session_skips_stale_last_selected() {
        let mut project = make_project_with_sessions("/repo/a", &["1", "2"]);
        project.last_selected_session = Some("deleted".to_string()); // stale
        let config = AppConfig {
            projects: vec![project],
            ..Default::default()
        };
        // Falls back to first session
        assert_eq!(
            pick_initial_session(&config, &PathBuf::from("/repo/a")),
            Some("1".to_string())
        );
    }

    // ── apply_git_snapshot ─────────────────────────────────────────────

    #[test]
    fn apply_snapshot_auto_stages_new_files() {
        let mut runtime = ProjectRuntime::default();

        let changes = vec![make_change("file_a.rs"), make_change("file_b.rs")];
        let changed = apply_git_snapshot(&mut runtime, &Ok(changes));

        assert!(changed);
        assert!(runtime.staged_files.contains("file_a.rs"));
        assert!(runtime.staged_files.contains("file_b.rs"));
        assert_eq!(runtime.staged_files.len(), 2);
    }

    #[test]
    fn apply_snapshot_does_not_re_stage_existing_files() {
        let mut runtime = ProjectRuntime::default();

        // First snapshot: auto-stages both files
        let changes = vec![make_change("file_a.rs"), make_change("file_b.rs")];
        apply_git_snapshot(&mut runtime, &Ok(changes.clone()));
        assert_eq!(runtime.staged_files.len(), 2);

        // Manually unstage one
        runtime.staged_files.remove("file_a.rs");
        assert_eq!(runtime.staged_files.len(), 1);

        // Second snapshot with same files: should NOT re-stage file_a.rs
        let changed = apply_git_snapshot(&mut runtime, &Ok(changes));
        assert!(!changed); // same changes, no error change
        assert!(!runtime.staged_files.contains("file_a.rs"));
        assert!(runtime.staged_files.contains("file_b.rs"));
    }

    #[test]
    fn apply_snapshot_removes_staged_files_no_longer_present() {
        let mut runtime = ProjectRuntime::default();

        let changes = vec![make_change("file_a.rs"), make_change("file_b.rs")];
        apply_git_snapshot(&mut runtime, &Ok(changes));
        assert_eq!(runtime.staged_files.len(), 2);

        // New snapshot without file_b.rs
        let changes = vec![make_change("file_a.rs")];
        apply_git_snapshot(&mut runtime, &Ok(changes));
        assert!(runtime.staged_files.contains("file_a.rs"));
        assert!(!runtime.staged_files.contains("file_b.rs"));
        assert_eq!(runtime.staged_files.len(), 1);
    }

    #[test]
    fn apply_snapshot_returns_false_when_unchanged() {
        let mut runtime = ProjectRuntime::default();

        let changes = vec![make_change("file.rs")];
        assert!(apply_git_snapshot(&mut runtime, &Ok(changes.clone())));
        assert!(!apply_git_snapshot(&mut runtime, &Ok(changes)));
    }

    #[test]
    fn apply_snapshot_error_stores_error_message() {
        let mut runtime = ProjectRuntime::default();

        let changed = apply_git_snapshot(&mut runtime, &Err("git status failed".to_string()));
        assert!(changed);
        assert_eq!(
            runtime.git_snapshot.last_error,
            Some("git status failed".to_string())
        );
    }

    #[test]
    fn apply_snapshot_error_returns_false_for_same_error() {
        let mut runtime = ProjectRuntime::default();

        let err = Err("same error".to_string());
        assert!(apply_git_snapshot(&mut runtime, &err));
        assert!(!apply_git_snapshot(&mut runtime, &err));
    }

    #[test]
    fn apply_snapshot_success_after_error_clears_error() {
        let mut runtime = ProjectRuntime::default();

        apply_git_snapshot(&mut runtime, &Err("error".to_string()));
        assert!(runtime.git_snapshot.last_error.is_some());

        let changed = apply_git_snapshot(&mut runtime, &Ok(vec![]));
        assert!(changed); // error cleared counts as change
        assert!(runtime.git_snapshot.last_error.is_none());
    }

    #[test]
    fn apply_snapshot_empty_to_empty_is_not_changed() {
        let mut runtime = ProjectRuntime::default();
        // Default changes is [] and snapshot is Ok([]) → no change
        assert!(!apply_git_snapshot(&mut runtime, &Ok(vec![])));
    }
}
