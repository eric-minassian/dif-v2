use crate::panel_action::{PanelAction, derive_panel_action};
use crate::runtime::ActionPhase;
use crate::ui_state::UpdateStatus;
use git::{BranchStatus, CheckBucket, RepoCapabilities};
use ui::prelude::*;

use crate::WorkspaceView;

impl WorkspaceView {
    fn render_update_indicator(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = theme();
        match &self.state.update_status {
            UpdateStatus::Available {
                version,
                download_url,
            } => {
                let url = download_url.clone();
                Some(
                    h_flex()
                        .id("update-available")
                        .gap_1()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .text_color(t.accent_green)
                        .hover(|style| style.cursor_pointer().bg(t.hover_overlay))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.on_start_update(url.clone(), window, cx);
                        }))
                        .child(format!("Update {version}"))
                        .into_any_element(),
                )
            }
            UpdateStatus::Updating => Some(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child("Updating...")
                    .into_any_element(),
            ),
            UpdateStatus::Error(msg) => {
                let _ = msg;
                Some(
                    div()
                        .id("update-error")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .text_color(t.accent_red)
                        .hover(|style| style.cursor_pointer().bg(t.hover_overlay))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.spawn_update_check(window, cx);
                        }))
                        .child("Update failed - retry")
                        .into_any_element(),
                )
            }
            UpdateStatus::Idle | UpdateStatus::Checking => None,
        }
    }

    fn render_titlebar_action(
        &self,
        action: &PanelAction,
        phase: &ActionPhase,
        branch_status: &BranchStatus,
        repo_capabilities: &RepoCapabilities,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let t = theme();

        match phase {
            ActionPhase::Working(label) => {
                return Some(
                    div()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(t.text_muted)
                        .opacity(0.7)
                        .child(label.clone())
                        .into_any_element(),
                );
            }
            ActionPhase::Error(msg) => {
                return Some(
                    h_flex()
                        .id("titlebar-action-error")
                        .gap_1()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .text_color(t.accent_red)
                        .max_w(px(200.))
                        .overflow_hidden()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .overflow_hidden()
                                .child(msg.clone()),
                        )
                        .child(
                            div()
                                .id("dismiss-titlebar-err")
                                .cursor_pointer()
                                .hover(|s| s.text_color(t.text_primary))
                                .on_click(cx.listener(|this, _event, _window, cx| {
                                    this.on_dismiss_action_error(cx);
                                }))
                                .child(Icon::new(IconName::X).size(px(12.)).color(Color::Dim)),
                        )
                        .into_any_element(),
                );
            }
            ActionPhase::Idle => {}
        }

        if *action == PanelAction::None {
            return None;
        }

        // Rebase action: decide between Rebase & Merge and Auto Merge
        if *action == PanelAction::Rebase {
            let checks = &branch_status.checks;
            let all_checks_pass = !checks.is_empty()
                && checks
                    .iter()
                    .all(|c| matches!(c.bucket, CheckBucket::Pass | CheckBucket::Skipping));
            let auto_merge_enabled = branch_status.auto_merge_enabled;

            if auto_merge_enabled {
                return Some(
                    div()
                        .id("action-auto-merge")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .text_color(t.accent_purple)
                        .cursor_pointer()
                        .hover(|s| s.bg(t.hover_overlay))
                        .on_click(cx.listener(|this, _event, window, cx| {
                            this.on_toggle_pr_auto_merge(window, cx);
                        }))
                        .child("Auto Merge")
                        .into_any_element(),
                );
            }

            if !all_checks_pass && repo_capabilities.auto_merge_allowed {
                return Some(
                    div()
                        .id("action-auto-merge")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .text_color(t.accent_green)
                        .cursor_pointer()
                        .hover(|s| s.bg(t.hover_overlay))
                        .on_click(cx.listener(|this, _event, window, cx| {
                            this.on_toggle_pr_auto_merge(window, cx);
                        }))
                        .child("Auto Merge")
                        .into_any_element(),
                );
            }

            if !repo_capabilities.rebase_merge_allowed {
                return None;
            }
        }

        let (button_id, label, text_color): (&str, &str, gpui::Hsla) = match action {
            PanelAction::Commit => ("action-commit", "Commit", t.text_primary),
            PanelAction::Amend => ("action-amend", "Amend", t.text_primary),
            PanelAction::CreatePR => ("action-create-pr", "Create PR", t.accent_blue),
            PanelAction::Rebase => ("action-rebase", "Rebase & Merge", t.text_primary),
            PanelAction::CloseSession => ("action-close", "Close Session", t.accent_red),
            PanelAction::None => return None,
        };

        let action_clone = action.clone();

        Some(
            div()
                .id(button_id)
                .px_2()
                .py_1()
                .rounded_sm()
                .text_xs()
                .text_color(text_color)
                .cursor_pointer()
                .hover(|s| s.bg(t.hover_overlay))
                .on_click(
                    cx.listener(move |this, _event, window, cx| match action_clone {
                        PanelAction::Commit => this.on_commit(window, cx),
                        PanelAction::Amend => this.on_amend(window, cx),
                        PanelAction::CreatePR => this.on_create_pr(window, cx),
                        PanelAction::Rebase => this.on_rebase(window, cx),
                        PanelAction::CloseSession => this.on_close_session(window, cx),
                        PanelAction::None => {}
                    }),
                )
                .child(label)
                .into_any_element(),
        )
    }

    fn render_titlebar_pr_link(&self, branch_status: &BranchStatus) -> Option<AnyElement> {
        let t = theme();
        let url = branch_status.pr_url.clone()?;

        let pr_label = match branch_status.pr_number {
            Some(n) => format!("#{n}"),
            None => "PR".to_string(),
        };

        Some(
            h_flex()
                .id("titlebar-pr-link")
                .gap(px(2.))
                .px_1()
                .py_1()
                .rounded_sm()
                .cursor_pointer()
                .hover(|s| s.bg(t.hover_overlay))
                .on_click(move |_event, _window, _cx| {
                    let _ = std::process::Command::new("open").arg(&url).spawn();
                })
                .child(div().text_xs().text_color(t.text_muted).child(pr_label))
                .child(
                    Icon::new(IconName::ExternalLink)
                        .size(px(12.))
                        .color(Color::Dim),
                )
                .into_any_element(),
        )
    }

    pub(crate) fn render_titlebar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let is_fullscreen = window.is_fullscreen();
        let left_collapsed = self.state.left_sidebar_collapsed;
        let right_collapsed = self.state.right_sidebar_collapsed;

        let project_name = self.state.selected_repo.as_ref().and_then(|repo| {
            self.state
                .config
                .projects
                .iter()
                .find(|p| &p.repo_root == repo)
                .map(|p| p.display_name.clone())
        });
        let branch_name = self
            .selected_project_runtime()
            .and_then(|rt| rt.branch_status.branch_name.clone());

        // Git lifecycle data
        let project_runtime = self.selected_project_runtime();
        let has_changes = project_runtime.map_or(false, |rt| !rt.git_snapshot.changes.is_empty());
        let staged_count = project_runtime.map_or(0, |rt| rt.staged_files.len());
        let default_branch_status = BranchStatus::default();
        let branch_status = project_runtime
            .map(|rt| &rt.branch_status)
            .unwrap_or(&default_branch_status);
        let default_action_phase = ActionPhase::default();
        let action_phase = project_runtime
            .map(|rt| &rt.action_phase)
            .unwrap_or(&default_action_phase);
        let default_repo_caps = RepoCapabilities::default();
        let repo_capabilities = project_runtime
            .map(|rt| &rt.repo_capabilities)
            .unwrap_or(&default_repo_caps);

        let panel_action = derive_panel_action(has_changes, staged_count, branch_status);

        let action_element = self.render_titlebar_action(
            &panel_action,
            action_phase,
            branch_status,
            repo_capabilities,
            cx,
        );
        let ci_status = self.render_checks_status_icon(branch_status, cx);
        let pr_link = self.render_titlebar_pr_link(branch_status);

        h_flex()
            .h(px(36.))
            .flex_shrink_0()
            .justify_between()
            .bg(t.bg_titlebar)
            .border_b_1()
            .border_color(t.border_subtle)
            // Left side: traffic light padding + sidebar toggle + app name
            .child(
                h_flex()
                    .pl(if is_fullscreen { px(8.) } else { px(78.) })
                    .gap_1()
                    .items_center()
                    .child(
                        div()
                            .id("toggle-left-sidebar")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|style| style.bg(t.hover_overlay))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.on_toggle_left_sidebar(cx);
                            }))
                            .child(Icon::new(IconName::PanelLeft).size(px(14.)).color(
                                if left_collapsed {
                                    t.text_dim
                                } else {
                                    t.text_muted
                                },
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(t.text_secondary)
                            .pl_1()
                            .child("Dif"),
                    ),
            )
            // Center: project name / branch
            .child(
                h_flex()
                    .flex_1()
                    .justify_center()
                    .gap(px(4.))
                    .overflow_hidden()
                    .text_xs()
                    .when_some(project_name, |el, name| {
                        el.child(div().text_color(t.text_secondary).child(name))
                    })
                    .when_some(branch_name, |el, branch| {
                        el.child(div().text_color(t.text_dim).child("/"))
                            .child(div().text_color(t.text_muted).child(branch))
                    }),
            )
            // Right side: action + CI + PR + update + sidebar toggle
            .child(
                h_flex()
                    .gap_1()
                    .pr_2()
                    .items_center()
                    .children(action_element)
                    .child(ci_status)
                    .children(pr_link)
                    .children(self.render_update_indicator(cx))
                    .child(
                        div()
                            .id("toggle-right-sidebar")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|style| style.bg(t.hover_overlay))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.on_toggle_right_sidebar(cx);
                            }))
                            .child(Icon::new(IconName::PanelRight).size(px(14.)).color(
                                if right_collapsed {
                                    t.text_dim
                                } else {
                                    t.text_muted
                                },
                            )),
                    ),
            )
            .into_any_element()
    }
}
