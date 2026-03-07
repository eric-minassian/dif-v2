use std::path::{Path, PathBuf};
use std::time::Instant;

use gpui::CursorStyle;

use crate::git;
use crate::prelude::*;
use crate::state::{
    AppConfig, GitChange, ProjectRuntime, ResizingSidebar, SavedProject,
};

use super::WorkspaceView;

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
                if !runtime.git_snapshot.changes.iter().any(|old| old.path == change.path) {
                    runtime.staged_files.insert(change.path.clone());
                }
            }

            // Remove staged paths that no longer appear in changes
            runtime.staged_files.retain(|p| current_paths.contains(p.as_str()));

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
    if let Some(last) = &project.last_selected_session {
        if project.sessions.iter().any(|s| s.id == *last) {
            return Some(last.clone());
        }
    }
    project.sessions.first().map(|s| s.id.clone())
}
