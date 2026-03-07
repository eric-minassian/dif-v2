use std::path::PathBuf;

use gpui::{AppContext, Context, Focusable, MouseUpEvent, Window};

use crate::git;
use crate::git::conventional::is_conventional_commit;
use crate::state::SavedSession;
use crate::text_input::{TextInput, TextInputEvent};

use super::helpers::pick_initial_session;
use super::{InlineEdit, SessionCreate, SessionRename, WorkspaceView};

impl WorkspaceView {
    /// "+" button on project row: begin creating a session for that project.
    pub(crate) fn on_add_session(
        &mut self,
        repo_root: PathBuf,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_creating_session(repo_root, window, cx);
    }

    /// Cmd+N: create a new session in the currently selected project.
    pub(crate) fn on_new_session(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo_root) = self.state.selected_repo.clone() else {
            return;
        };
        self.begin_creating_session(repo_root, window, cx);
    }

    /// Show an inline text input in the sidebar for entering the commit message / session name.
    fn begin_creating_session(
        &mut self,
        repo_root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Ensure the project exists and is valid
        let project_exists = self
            .state
            .config
            .projects
            .iter()
            .any(|p| p.repo_root == repo_root && p.last_known_valid);
        if !project_exists {
            return;
        }

        // Expand the project if collapsed
        self.state.collapsed_projects.remove(&repo_root);

        let input = cx.new(|cx| TextInput::new(String::new(), window, cx));
        let event_sub = cx.subscribe_in(&input, window, {
            let repo_root = repo_root.clone();
            move |this, _input, event, window, cx| match event {
                TextInputEvent::Confirm(message) => {
                    this.on_create_session_confirm(
                        repo_root.clone(),
                        message.clone(),
                        window,
                        cx,
                    );
                }
                TextInputEvent::Cancel => {
                    this.creating_session = None;
                    cx.notify();
                }
            }
        });
        let blur_sub = cx.on_blur(
            &input.read(cx).focus_handle(cx),
            window,
            {
                let repo_root = repo_root.clone();
                move |this, window, cx| {
                    if let Some(create) = this.creating_session.take() {
                        let message = create.edit.input.read(cx).text().trim().to_string();
                        if message.is_empty() {
                            cx.notify();
                        } else {
                            this.on_create_session_confirm(
                                repo_root.clone(),
                                message,
                                window,
                                cx,
                            );
                        }
                    }
                }
            },
        );
        self.creating_session = Some(SessionCreate {
            edit: InlineEdit {
                repo_root,
                input,
                _event_sub: event_sub,
                _blur_sub: blur_sub,
            },
            validation_error: None,
        });
        cx.notify();
    }

    /// Called when the user confirms the session name (Enter or blur with text).
    fn on_create_session_confirm(
        &mut self,
        repo_root: PathBuf,
        message: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if message.is_empty() {
            self.creating_session = None;
            cx.notify();
            return;
        }

        if self.enforces_conventional_commits(&repo_root) && !is_conventional_commit(&message) {
            if let Some(create) = &mut self.creating_session {
                create.validation_error = Some(
                    "Must follow Conventional Commits: type[(scope)]: description".into(),
                );
            }
            cx.notify();
            return;
        }

        self.creating_session = None;

        let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        else {
            return;
        };

        let new_id = project.next_session_id();

        project.sessions.push(SavedSession {
            id: new_id.clone(),
            name: message.clone(),
            worktree_path: None,
        });

        match git::create_worktree(&repo_root, &message) {
            Ok(wt_path) => {
                self.run_init_commands(&repo_root, &wt_path);
                if let Some(project) = self
                    .state
                    .config
                    .projects
                    .iter_mut()
                    .find(|p| p.repo_root == repo_root)
                {
                    if let Some(session) = project.sessions.iter_mut().find(|s| s.id == new_id) {
                        session.worktree_path = Some(wt_path);
                    }
                }
            }
            Err(error) => {
                self.state.flash_error = Some(format!("Failed to create worktree: {error}"));
            }
        }

        self.activate_session(repo_root, new_id, window, cx);
    }

    pub(crate) fn on_delete_session(
        &mut self,
        repo_root: PathBuf,
        session_id: String,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Grab worktree path before removing the session
        let worktree_path = self
            .state
            .config
            .projects
            .iter()
            .find(|p| p.repo_root == repo_root)
            .and_then(|p| p.sessions.iter().find(|s| s.id == session_id))
            .and_then(|s| s.worktree_path.clone());

        if let Some(runtime) = self.state.runtimes.get_mut(&repo_root) {
            runtime.session_runtimes.remove(&session_id);
        }

        if let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        {
            project.sessions.retain(|s| s.id != session_id);
        }

        if let Some(wt_path) = worktree_path {
            git::remove_worktree(&repo_root, &wt_path);
        }

        let was_selected = self
            .state
            .selected_repo
            .as_ref()
            .is_some_and(|r| r == &repo_root)
            && self
                .state
                .selected_session
                .as_ref()
                .is_some_and(|s| s == &session_id);

        if was_selected {
            let new_session = pick_initial_session(&self.state.config, &repo_root);
            if let Some(new_session) = new_session {
                self.activate_session(repo_root, new_session, window, cx);
                return;
            } else {
                self.state.selected_session = None;
                // Stop the git poll so it doesn't run against the removed worktree
                self.state.git_poll_generation = self.state.git_poll_generation.wrapping_add(1);
                // Clear stale git data
                if let Some(runtime) = self.state.runtimes.get_mut(&repo_root) {
                    runtime.git_snapshot = Default::default();
                }
            }
        }

        self.persist_config();
        cx.notify();
    }

    pub(crate) fn on_rename_session_start(
        &mut self,
        repo_root: PathBuf,
        session_id: String,
        current_name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| TextInput::new(current_name, window, cx));
        let event_sub = cx.subscribe(&input, {
            let repo_root = repo_root.clone();
            let session_id = session_id.clone();
            move |this, _input, event, cx| match event {
                TextInputEvent::Confirm(new_name) => {
                    this.on_rename_session_commit(
                        repo_root.clone(),
                        session_id.clone(),
                        new_name.clone(),
                        cx,
                    );
                }
                TextInputEvent::Cancel => {
                    this.renaming_session = None;
                    cx.notify();
                }
            }
        });
        let blur_sub = cx.on_blur(
            &input.read(cx).focus_handle(cx),
            window,
            move |this, _window, cx| {
                if let Some(rename) = this.renaming_session.take() {
                    let new_name = rename.edit.input.read(cx).text().trim().to_string();
                    this.on_rename_session_commit(rename.edit.repo_root, rename.session_id, new_name, cx);
                }
            },
        );
        self.renaming_session = Some(SessionRename {
            session_id,
            edit: InlineEdit {
                repo_root,
                input,
                _event_sub: event_sub,
                _blur_sub: blur_sub,
            },
        });
        cx.notify();
    }

    pub(crate) fn on_rename_session_commit(
        &mut self,
        repo_root: PathBuf,
        session_id: String,
        new_name: String,
        cx: &mut Context<Self>,
    ) {
        self.renaming_session = None;
        if new_name.is_empty() {
            cx.notify();
            return;
        }

        if self.enforces_conventional_commits(&repo_root) && !is_conventional_commit(&new_name) {
            self.state.flash_error = Some(
                "Session name must follow Conventional Commits: type[(scope)]: description".into(),
            );
            cx.notify();
            return;
        }

        if let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        {
            if let Some(session) = project.sessions.iter_mut().find(|s| s.id == session_id) {
                session.name = new_name;
            }
        }
        self.persist_config();
        cx.notify();
    }

    pub(crate) fn on_close_session(
        &mut self,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };
        let Some(session_id) = self.state.selected_session.clone() else {
            return;
        };
        // Reuse on_delete_session logic with a synthetic event
        self.on_delete_session(repo, session_id, _event, window, cx);
    }

    pub(crate) fn select_session_by_index(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };
        let Some(project) = self.state.config.projects.iter().find(|p| p.repo_root == repo) else {
            return;
        };
        if let Some(session) = project.sessions.get(index) {
            let session_id = session.id.clone();
            self.activate_session(repo, session_id, window, cx);
        }
    }

    pub(crate) fn activate_session(
        &mut self,
        repo_root: PathBuf,
        session_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Save current branch status / repo capabilities to the outgoing session's cache
        if let (Some(old_repo), Some(old_session)) = (
            self.state.selected_repo.clone(),
            self.state.selected_session.clone(),
        ) {
            if let Some(runtime) = self.state.runtimes.get_mut(&old_repo) {
                if let Some(session_rt) = runtime.session_runtimes.get_mut(&old_session) {
                    session_rt.cached_branch_status = Some(runtime.branch_status.clone());
                    session_rt.cached_repo_capabilities = Some(runtime.repo_capabilities.clone());
                }
            }
        }

        self.state.selected_repo = Some(repo_root.clone());
        self.state.selected_session = Some(session_id.clone());
        self.ensure_session_runtime(&repo_root, &session_id, window, cx);

        // Restore cached branch status / repo capabilities from incoming session
        if let Some(runtime) = self.state.runtimes.get_mut(&repo_root) {
            if let Some(session_rt) = runtime.session_runtimes.get(&session_id) {
                if let Some(cached) = &session_rt.cached_branch_status {
                    runtime.branch_status = cached.clone();
                }
                if let Some(cached) = &session_rt.cached_repo_capabilities {
                    runtime.repo_capabilities = cached.clone();
                }
            }
        }

        self.state.config.last_selected_repo = Some(repo_root.clone());

        if let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        {
            project.last_selected_session = Some(session_id);
        }

        self.persist_config();
        self.start_git_poll(repo_root, window, cx);
        cx.notify();
    }
}
