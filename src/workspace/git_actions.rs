use std::path::PathBuf;
use std::time::Duration;

use gpui::{Context, KeyDownEvent, Window};

use crate::git;
use crate::state::{ActionPhase, ProjectRuntime};

use super::helpers::apply_git_snapshot;
use super::WorkspaceView;

/// Checks whether a commit message follows the Conventional Commits spec.
/// Expects: `type[(scope)][!]: description`
fn is_conventional_commit(message: &str) -> bool {
    let first_line = message.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return false;
    }

    // Split on first ':'
    let Some(colon_pos) = first_line.find(':') else {
        return false;
    };

    let prefix = &first_line[..colon_pos];
    let description = first_line[colon_pos + 1..].trim();

    // Description after colon must be non-empty
    if description.is_empty() {
        return false;
    }

    // Strip optional '!' before colon (breaking change marker)
    let prefix = prefix.strip_suffix('!').unwrap_or(prefix);

    // Strip optional scope in parens
    let type_part = if let Some(paren_start) = prefix.find('(') {
        if !prefix.ends_with(')') {
            return false;
        }
        &prefix[..paren_start]
    } else {
        prefix
    };

    // Type must be non-empty, lowercase alphanumeric
    !type_part.is_empty()
        && type_part
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

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

    pub(crate) fn on_commit_input_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.state.selected_repo.as_ref().cloned() else {
            return;
        };
        let Some(session_id) = self.state.selected_session.as_ref().cloned() else {
            return;
        };
        let Some(runtime) = self.state.runtimes.get_mut(&repo) else {
            return;
        };
        let Some(session_runtime) = runtime.session_runtimes.get_mut(&session_id) else {
            return;
        };

        // Use key_char for composed characters, fallback to key
        let key_char = event.keystroke.key_char.as_deref();
        let key = &event.keystroke.key;

        let has_platform = event.keystroke.modifiers.platform;
        let has_ctrl = event.keystroke.modifiers.control;

        match key.as_str() {
            "backspace" if !has_platform && !has_ctrl => {
                session_runtime.commit_message.pop();
            }
            "backspace" if has_platform || has_ctrl => {
                session_runtime.commit_message.clear();
            }
            "escape" => {
                self.focus_handle.focus(_window, cx);
            }
            _ if has_platform || has_ctrl => {
                return; // let shortcuts propagate
            }
            _ => {
                // Insert the typed character if available
                if let Some(ch) = key_char {
                    if !ch.is_empty() && ch.chars().all(|c: char| !c.is_control()) {
                        session_runtime.commit_message.push_str(ch);
                    } else {
                        return;
                    }
                } else {
                    return;
                }
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

        let working_dir = self
            .state
            .selected_session
            .as_deref()
            .map(|sid| self.worktree_or_repo(&repo, sid))
            .unwrap_or_else(|| repo.clone());

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

        let working_dir = self
            .state
            .selected_session
            .as_deref()
            .map(|sid| self.worktree_or_repo(&repo, sid))
            .unwrap_or_else(|| repo.clone());

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

        let working_dir = self
            .state
            .selected_session
            .as_deref()
            .map(|sid| self.worktree_or_repo(&repo, sid))
            .unwrap_or_else(|| repo.clone());

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

                cx.update(|_, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(rt) = this.state.runtimes.get_mut(&repo_clone) {
                            match pr_result {
                                Ok(_) => {
                                    rt.action_phase = ActionPhase::Idle;
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

        let auto_merge = self
            .state
            .config
            .projects
            .iter()
            .find(|p| p.repo_root == repo)
            .map(|p| p.settings.auto_merge)
            .unwrap_or(false);

        runtime.action_phase = ActionPhase::Working("Merging...".into());
        cx.notify();

        let working_dir = self
            .state
            .selected_session
            .as_deref()
            .map(|sid| self.worktree_or_repo(&repo, sid))
            .unwrap_or_else(|| repo.clone());

        let view = cx.entity().clone();
        let repo_clone = repo.clone();

        window
            .spawn(cx, async move |cx| {
                let dir = working_dir.clone();

                let merge_result = cx
                    .background_executor()
                    .spawn({
                        let dir = dir.clone();
                        async move { git::merge_pr_rebase(&dir, auto_merge) }
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

    pub(crate) fn start_git_poll(
        &mut self,
        repo_root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.git_poll_generation = self.state.git_poll_generation.wrapping_add(1);
        let generation = self.state.git_poll_generation;
        let view = cx.entity().clone();

        let git_dir = self
            .state
            .selected_session
            .as_deref()
            .map(|sid| self.worktree_or_repo(&repo_root, sid))
            .unwrap_or_else(|| repo_root.clone());

        window
            .spawn(cx, async move |cx| {
                let mut tick: u32 = 0;
                loop {
                    let dir = git_dir.clone();
                    let snapshot = cx
                        .background_executor()
                        .spawn(async move { git::collect_changes(&dir) })
                        .await;

                    // Every 5th tick (~10s) or on first tick, also collect branch status
                    let branch_status = if tick % 5 == 0 {
                        let dir = git_dir.clone();
                        Some(
                            cx.background_executor()
                                .spawn(async move { git::collect_branch_status(&dir) })
                                .await,
                        )
                    } else {
                        None
                    };

                    let keep_running = cx
                        .update(|_, cx| {
                            view.update(cx, |this, cx| {
                                if this.state.git_poll_generation != generation
                                    || this.state.selected_repo.as_ref() != Some(&repo_root)
                                {
                                    return false;
                                }

                                let runtime = this
                                    .state
                                    .runtimes
                                    .entry(repo_root.clone())
                                    .or_insert_with(ProjectRuntime::default);

                                let mut changed = apply_git_snapshot(runtime, &snapshot);

                                if let Some(status) = branch_status {
                                    if runtime.branch_status != status {
                                        runtime.branch_status = status;
                                        changed = true;
                                    }
                                }

                                if changed {
                                    cx.notify();
                                }

                                true
                            })
                        })
                        .unwrap_or(false);

                    if !keep_running {
                        break;
                    }

                    tick = tick.wrapping_add(1);
                    cx.background_executor().timer(Duration::from_secs(2)).await;
                }
            })
            .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::is_conventional_commit;

    #[test]
    fn valid_conventional_commits() {
        assert!(is_conventional_commit("feat: add login"));
        assert!(is_conventional_commit("fix: resolve crash"));
        assert!(is_conventional_commit("feat(auth): add oauth"));
        assert!(is_conventional_commit("fix(ui): button alignment"));
        assert!(is_conventional_commit("chore: bump deps"));
        assert!(is_conventional_commit("docs: update readme"));
        assert!(is_conventional_commit("feat!: breaking change"));
        assert!(is_conventional_commit("feat(api)!: remove endpoint"));
        assert!(is_conventional_commit("ci: update workflow"));
        assert!(is_conventional_commit("refactor: simplify logic"));
        assert!(is_conventional_commit("feat: add thing\n\nbody text here"));
    }

    #[test]
    fn invalid_conventional_commits() {
        assert!(!is_conventional_commit(""));
        assert!(!is_conventional_commit("just a message"));
        assert!(!is_conventional_commit("Fix the bug"));
        assert!(!is_conventional_commit("feat:"));
        assert!(!is_conventional_commit("feat: "));
        assert!(!is_conventional_commit("FEAT: uppercase type"));
        assert!(!is_conventional_commit("feat(: bad scope"));
        assert!(!is_conventional_commit(": no type"));
    }
}
