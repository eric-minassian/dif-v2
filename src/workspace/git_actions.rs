use gpui::{Context, Window};

use crate::git;
use crate::git::conventional::is_conventional_commit;
use crate::state::ActionPhase;

use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn on_refresh_git_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(repo) = self.state.selected_repo.clone() {
            self.start_git_poll(repo, window, cx);
        }
    }

    pub(crate) fn on_toggle_staged(&mut self, path: String, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.as_ref() else {
            return;
        };
        let Some(runtime) = self.state.runtimes.get_mut(repo) else {
            return;
        };
        if runtime.staged_files.contains(&path) {
            runtime.staged_files.remove(&path);
        } else {
            runtime.staged_files.insert(path);
        }
        cx.notify();
    }

    pub(crate) fn on_dismiss_action_error(&mut self, cx: &mut Context<Self>) {
        if let Some(repo) = self.state.selected_repo.as_ref() {
            if let Some(runtime) = self.state.runtimes.get_mut(repo) {
                runtime.action_phase = ActionPhase::Idle;
            }
        }
        cx.notify();
    }

    pub(crate) fn on_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };
        let Some(session_id) = self.state.selected_session.clone() else {
            return;
        };

        let (files, message) = {
            let Some(runtime) = self.state.runtimes.get(&repo) else {
                return;
            };
            if matches!(runtime.action_phase, ActionPhase::Working(_)) {
                return;
            }
            let files: Vec<String> = runtime.staged_files.iter().cloned().collect();
            if files.is_empty() {
                return;
            }
            let Some(session_runtime) = runtime.session_runtimes.get(&session_id) else {
                return;
            };
            if session_runtime.commit_message.trim().is_empty() {
                return;
            }
            (files, session_runtime.commit_message.clone())
        };

        // Validate conventional commit format if enforced
        if let Some(project) = self
            .state
            .config
            .projects
            .iter()
            .find(|p| p.repo_root == repo)
        {
            if project.settings.enforce_conventional_commits && !is_conventional_commit(&message) {
                if let Some(runtime) = self.state.runtimes.get_mut(&repo) {
                    runtime.action_phase = ActionPhase::Error(
                        "Commit message must follow Conventional Commits: type[(scope)]: description"
                            .into(),
                    );
                }
                cx.notify();
                return;
            }
        }

        let Some(runtime) = self.state.runtimes.get_mut(&repo) else {
            return;
        };
        runtime.action_phase = ActionPhase::Working("Committing...".into());
        cx.notify();

        let working_dir = self.working_dir(&repo);

        let view = cx.entity().clone();
        let repo_clone = repo.clone();
        let session_id_clone = session_id.clone();

        window
            .spawn(cx, async move |cx| {
                let dir = working_dir.clone();

                let result = cx
                    .background_executor()
                    .spawn({
                        let dir = dir.clone();
                        let files = files.clone();
                        async move { git::commit_selected(&dir, &files, &message) }
                    })
                    .await;

                if let Err(e) = result {
                    cx.update(|_, cx| {
                        view.update(cx, |this, cx| {
                            if let Some(rt) = this.state.runtimes.get_mut(&repo_clone) {
                                rt.action_phase = ActionPhase::Error(format!("Commit: {e}"));
                            }
                            cx.notify();
                        })
                    })
                    .ok();
                    return;
                }

                // Push
                let push_result = cx
                    .background_executor()
                    .spawn({
                        let dir = dir.clone();
                        async move { git::push(&dir) }
                    })
                    .await;

                cx.update(|_, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(rt) = this.state.runtimes.get_mut(&repo_clone) {
                            match push_result {
                                Ok(()) => {
                                    rt.staged_files.clear();
                                    rt.action_phase = ActionPhase::Idle;
                                    if let Some(session_rt) =
                                        rt.session_runtimes.get_mut(&session_id_clone)
                                    {
                                        session_rt.commit_message.clear();
                                    }
                                }
                                Err(e) => {
                                    rt.action_phase = ActionPhase::Error(format!("Push: {e}"));
                                }
                            }
                        }
                        cx.notify();
                    })
                })
                .ok();
            })
            .detach();
    }

    pub(crate) fn on_amend(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };
        let Some(runtime) = self.state.runtimes.get_mut(&repo) else {
            return;
        };
        if matches!(runtime.action_phase, ActionPhase::Working(_)) {
            return;
        }
        let files: Vec<String> = runtime.staged_files.iter().cloned().collect();
        if files.is_empty() {
            return;
        }
        runtime.action_phase = ActionPhase::Working("Amending...".into());
        cx.notify();

        let working_dir = self.working_dir(&repo);

        let view = cx.entity().clone();
        let repo_clone = repo.clone();

        window
            .spawn(cx, async move |cx| {
                let dir = working_dir.clone();

                let result = cx
                    .background_executor()
                    .spawn({
                        let dir = dir.clone();
                        let files = files.clone();
                        async move { git::amend_selected(&dir, &files) }
                    })
                    .await;

                if let Err(e) = result {
                    cx.update(|_, cx| {
                        view.update(cx, |this, cx| {
                            if let Some(rt) = this.state.runtimes.get_mut(&repo_clone) {
                                rt.action_phase = ActionPhase::Error(format!("Amend: {e}"));
                            }
                            cx.notify();
                        })
                    })
                    .ok();
                    return;
                }

                // Force push
                let push_result = cx
                    .background_executor()
                    .spawn({
                        let dir = dir.clone();
                        async move { git::force_push(&dir) }
                    })
                    .await;

                cx.update(|_, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(rt) = this.state.runtimes.get_mut(&repo_clone) {
                            match push_result {
                                Ok(()) => {
                                    rt.staged_files.clear();
                                    rt.action_phase = ActionPhase::Idle;
                                }
                                Err(e) => {
                                    rt.action_phase = ActionPhase::Error(format!("Push: {e}"));
                                }
                            }
                        }
                        cx.notify();
                    })
                })
                .ok();
            })
            .detach();
    }

    pub(crate) fn on_create_pr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };
        let Some(runtime) = self.state.runtimes.get_mut(&repo) else {
            return;
        };
        if matches!(runtime.action_phase, ActionPhase::Working(_)) {
            return;
        }
        runtime.action_phase = ActionPhase::Working("Creating PR...".into());
        cx.notify();

        let working_dir = self.working_dir(&repo);

        let view = cx.entity().clone();
        let repo_clone = repo.clone();

        window
            .spawn(cx, async move |cx| {
                let dir = working_dir.clone();

                let branch_name = cx
                    .background_executor()
                    .spawn({
                        let dir = dir.clone();
                        async move { git::get_branch_name(&dir) }
                    })
                    .await
                    .unwrap_or_else(|_| "changes".to_string());

                let pr_result = cx
                    .background_executor()
                    .spawn({
                        let dir = dir.clone();
                        async move { git::create_pr(&dir, &branch_name) }
                    })
                    .await;

                cx.update(|window, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(rt) = this.state.runtimes.get_mut(&repo_clone) {
                            match pr_result {
                                Ok(_) => {
                                    rt.action_phase = ActionPhase::Idle;
                                    // Restart poll so checks are fetched immediately
                                    this.start_git_poll(repo_clone.clone(), window, cx);
                                }
                                Err(e) => {
                                    rt.action_phase = ActionPhase::Error(format!("PR: {e}"));
                                }
                            }
                        }
                        cx.notify();
                    })
                })
                .ok();
            })
            .detach();
    }

    pub(crate) fn on_rebase(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };
        let Some(runtime) = self.state.runtimes.get_mut(&repo) else {
            return;
        };
        if matches!(runtime.action_phase, ActionPhase::Working(_)) {
            return;
        }

        runtime.action_phase = ActionPhase::Working("Merging...".into());
        cx.notify();

        let working_dir = self.working_dir(&repo);

        let view = cx.entity().clone();
        let repo_clone = repo.clone();

        window
            .spawn(cx, async move |cx| {
                let dir = working_dir.clone();

                let merge_result = cx
                    .background_executor()
                    .spawn({
                        let dir = dir.clone();
                        async move { git::merge_pr_rebase(&dir) }
                    })
                    .await;

                cx.update(|_, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(rt) = this.state.runtimes.get_mut(&repo_clone) {
                            match merge_result {
                                Ok(()) => {
                                    rt.action_phase = ActionPhase::Idle;
                                }
                                Err(e) => {
                                    rt.action_phase = ActionPhase::Error(format!("Merge: {e}"));
                                }
                            }
                        }
                        cx.notify();
                    })
                })
                .ok();
            })
            .detach();
    }

    pub(crate) fn on_toggle_pr_auto_merge(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };
        let Some(runtime) = self.state.runtimes.get_mut(&repo) else {
            return;
        };
        if matches!(runtime.action_phase, ActionPhase::Working(_)) {
            return;
        }

        let currently_enabled = runtime.branch_status.auto_merge_enabled;
        let label = if currently_enabled {
            "Disabling auto-merge..."
        } else {
            "Enabling auto-merge..."
        };
        runtime.action_phase = ActionPhase::Working(label.into());
        cx.notify();

        let working_dir = self.working_dir(&repo);

        let view = cx.entity().clone();
        let repo_clone = repo.clone();

        window
            .spawn(cx, async move |cx| {
                let dir = working_dir.clone();

                let result = cx
                    .background_executor()
                    .spawn({
                        let dir = dir.clone();
                        async move {
                            if currently_enabled {
                                git::disable_auto_merge(&dir)
                            } else {
                                git::enable_auto_merge(&dir)
                            }
                        }
                    })
                    .await;

                cx.update(|window, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(rt) = this.state.runtimes.get_mut(&repo_clone) {
                            match result {
                                Ok(()) => {
                                    rt.branch_status.auto_merge_enabled = !currently_enabled;
                                    rt.action_phase = ActionPhase::Idle;
                                    // Restart poll to pick up fresh state
                                    this.start_git_poll(repo_clone.clone(), window, cx);
                                }
                                Err(e) => {
                                    rt.action_phase =
                                        ActionPhase::Error(format!("Auto-merge: {e}"));
                                }
                            }
                        }
                        cx.notify();
                    })
                })
                .ok();
            })
            .detach();
    }
}
