use std::path::PathBuf;
use std::time::Duration;

use crate::git;
use crate::prelude::*;
use crate::state::ProjectRuntime;

use super::helpers::apply_git_snapshot;
use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn start_git_poll(
        &mut self,
        repo_root: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.git_poll_generation = self.state.git_poll_generation.wrapping_add(1);
        let generation = self.state.git_poll_generation;
        let view = cx.entity().clone();

        let git_dir = self.working_dir(&repo_root);

        self._git_poll_task = Some(window
            .spawn(cx, async move |cx| {
                let mut tick: u32 = 0;
                loop {
                    let dir = git_dir.clone();
                    let snap_task = cx
                        .background_executor()
                        .spawn(async move { git::collect_changes(&dir) });

                    // Every 5th tick (~10s) or on first tick, also collect branch status
                    // and repo capabilities — spawn concurrently with collect_changes
                    let status_task = if tick % 5 == 0 {
                        let dir = git_dir.clone();
                        Some(cx.background_executor().spawn(async move {
                            git::collect_branch_status(&dir)
                        }))
                    } else {
                        None
                    };
                    let caps_task = if tick % 5 == 0 {
                        let dir = git_dir.clone();
                        Some(cx.background_executor().spawn(async move {
                            git::check_repo_capabilities(&dir)
                        }))
                    } else {
                        None
                    };

                    // Update UI immediately with file changes (fast, local git only)
                    let snapshot = snap_task.await;
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
                                if apply_git_snapshot(runtime, &snapshot) {
                                    cx.notify();
                                }
                                true
                            })
                        })
                        .unwrap_or(false);

                    if !keep_running {
                        break;
                    }

                    // Then wait for slow network tasks and update UI again
                    if let Some(status_task) = status_task {
                        let branch_status = status_task.await;
                        let repo_caps = match caps_task {
                            Some(t) => Some(t.await),
                            None => None,
                        };

                        let still_running = cx
                            .update(|_, cx| {
                                view.update(cx, |this, cx| {
                                    if this.state.git_poll_generation != generation
                                        || this.state.selected_repo.as_ref() != Some(&repo_root)
                                    {
                                        return false;
                                    }
                                    let session_id = this.state.selected_session.clone();
                                    let runtime = this
                                        .state
                                        .runtimes
                                        .entry(repo_root.clone())
                                        .or_insert_with(ProjectRuntime::default);

                                    let mut changed = false;
                                    if runtime.branch_status != branch_status {
                                        if runtime.branch_status.checks != branch_status.checks {
                                            this.state.checks_popover_open = false;
                                        }
                                        runtime.branch_status = branch_status;
                                        changed = true;
                                    }
                                    if let Some(caps) = repo_caps {
                                        if runtime.repo_capabilities != caps {
                                            runtime.repo_capabilities = caps;
                                            changed = true;
                                        }
                                    }
                                    // Cache fresh values in the active session
                                    if changed {
                                        if let Some(sid) = &session_id {
                                            if let Some(srt) = runtime.session_runtimes.get_mut(sid) {
                                                srt.cached_branch_status = Some(runtime.branch_status.clone());
                                                srt.cached_repo_capabilities = Some(runtime.repo_capabilities.clone());
                                            }
                                        }
                                        cx.notify();
                                    }
                                    true
                                })
                            })
                            .unwrap_or(false);

                        if !still_running {
                            break;
                        }
                    }

                    tick = tick.wrapping_add(1);
                    cx.background_executor().timer(Duration::from_secs(2)).await;
                }
            }));
    }
}
