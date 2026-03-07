use std::path::PathBuf;

use git;
use crate::picker;
use ui::prelude::*;
use crate::config::SavedProject;

use crate::helpers::{pick_initial_selection, pick_initial_session};
use crate::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn on_add_project(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = picker::choose_folder() else {
            return;
        };

        self.add_project_from_path(path, window, cx);
    }

    pub(crate) fn on_remove_project(
        &mut self,
        repo_root: PathBuf,
        cx: &mut Context<Self>,
    ) {
        // Collect worktree paths before removing the project
        let worktree_paths: Vec<PathBuf> = self
            .state
            .config
            .projects
            .iter()
            .find(|p| p.repo_root == repo_root)
            .map(|p| {
                p.sessions
                    .iter()
                    .filter_map(|s| s.worktree_path.clone())
                    .collect()
            })
            .unwrap_or_default();

        let removed_selected = self
            .state
            .selected_repo
            .as_ref()
            .is_some_and(|selected| selected == &repo_root);

        self.state
            .config
            .projects
            .retain(|project| project.repo_root != repo_root);
        self.state.runtimes.remove(&repo_root);

        for wt_path in worktree_paths {
            git::remove_worktree(&repo_root, &wt_path);
        }

        if removed_selected {
            self.state.selected_repo = pick_initial_selection(&self.state.config);
            self.state.selected_session = self
                .state
                .selected_repo
                .as_ref()
                .and_then(|repo| pick_initial_session(&self.state.config, repo));
            self.state.git_poll_generation = self.state.git_poll_generation.wrapping_add(1);
        }

        self.persist_config();
        cx.notify();
    }

    pub(crate) fn on_select_project(
        &mut self,
        repo_root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_id = pick_initial_session(&self.state.config, &repo_root);
        if let Some(session_id) = session_id {
            self.activate_session(repo_root, session_id, window, cx);
        } else {
            self.state.git_poll_generation = self.state.git_poll_generation.wrapping_add(1);
            self.state.selected_repo = Some(repo_root.clone());
            self.state.selected_session = None;
            self.state.config.last_selected_repo = Some(repo_root);
            self.persist_config();
            cx.notify();
        }
    }

    pub(crate) fn add_project_from_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match git::normalize_repo_path(&path) {
            Ok(repo_root) => {
                if let Some(existing) = self
                    .state
                    .config
                    .projects
                    .iter()
                    .find(|project| project.repo_root == repo_root)
                    .map(|project| project.repo_root.clone())
                {
                    let session_id = pick_initial_session(&self.state.config, &existing)
                        .unwrap_or_else(|| "1".to_string());
                    self.activate_session(existing, session_id, window, cx);
                    return;
                }

                let project = SavedProject::from_repo_root(repo_root.clone());
                self.state.config.projects.push(project);
                self.state.selected_repo = Some(repo_root.clone());
                self.state.selected_session = None;
                self.state.config.last_selected_repo = Some(repo_root);
                self.persist_config();
                cx.notify();
            }
            Err(error) => {
                self.state.flash_error = Some(error);
                cx.notify();
            }
        }
    }

    pub(crate) fn on_toggle_project_collapse(
        &mut self,
        repo_root: PathBuf,
        cx: &mut Context<Self>,
    ) {
        if self.state.collapsed_projects.contains(&repo_root) {
            self.state.collapsed_projects.remove(&repo_root);
        } else {
            self.state.collapsed_projects.insert(repo_root);
        }
        cx.notify();
    }
}
