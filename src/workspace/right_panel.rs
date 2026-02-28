use gpui::{div, prelude::*, px, AnyElement, Context, Hsla, MouseButton};

use crate::components::{panel, section_header, PanelSide};
use crate::icons::{icon_check, icon_circle_dot, icon_external_link, icon_minus, icon_x};
use crate::state::{ActionPhase, BranchStatus, CheckBucket, CiCheck, GitChange, RepoCapabilities};
use crate::theme::theme;

use super::WorkspaceView;

#[derive(Clone, Debug, PartialEq)]
enum PanelAction {
    Commit,
    Amend,
    CreatePR,
    Rebase,
    CloseSession,
    None,
}

fn derive_panel_action(
    has_changes: bool,
    staged_count: usize,
    status: &BranchStatus,
) -> PanelAction {
    if status.pr_merged {
        return PanelAction::CloseSession;
    }
    if has_changes && staged_count > 0 && status.commits_ahead == 0 {
        return PanelAction::Commit;
    }
    if has_changes && staged_count > 0 && status.commits_ahead > 0 {
        return PanelAction::Amend;
    }
    if !has_changes && status.commits_ahead > 0 && status.pr_url.is_none() {
        return PanelAction::CreatePR;
    }
    if !has_changes && status.pr_url.is_some() && !status.pr_merged {
        return PanelAction::Rebase;
    }
    PanelAction::None
}

impl WorkspaceView {
    pub(crate) fn render_right_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let top = self.render_changes_panel(cx);
        let bottom = self.render_side_terminal(cx);

        panel(PanelSide::Right)
            .w(px(self.state.right_sidebar_width))
            .child(top)
            .child(bottom)
            .into_any_element()
    }

    pub(crate) fn render_collapsed_right_sidebar(&self) -> AnyElement {
        div().into_any_element()
    }

    fn render_changes_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let project_runtime = self.selected_project_runtime();
        let snapshot = project_runtime.map(|runtime| &runtime.git_snapshot);
        let changes = snapshot
            .map(|snapshot| snapshot.changes.as_slice())
            .unwrap_or(&[]);
        let error = snapshot.and_then(|snapshot| snapshot.last_error.as_ref());
        let count = changes.len();
        let has_changes = !changes.is_empty();

        let staged_files = project_runtime
            .map(|rt| &rt.staged_files)
            .cloned()
            .unwrap_or_default();
        let staged_count = staged_files.len();
        let branch_status = project_runtime
            .map(|rt| &rt.branch_status)
            .cloned()
            .unwrap_or_default();
        let action_phase = project_runtime
            .map(|rt| &rt.action_phase)
            .cloned()
            .unwrap_or_default();
        let commit_message = self
            .selected_session_runtime()
            .map(|rt| rt.commit_message.clone())
            .unwrap_or_default();

        let repo_capabilities = project_runtime
            .map(|rt| &rt.repo_capabilities)
            .cloned()
            .unwrap_or_default();

        let panel_action = derive_panel_action(has_changes, staged_count, &branch_status);
        let is_busy = matches!(action_phase, ActionPhase::Working(_));

        // Build the action button for the header bar
        let header_action = self.render_header_action_button(
            &panel_action,
            &action_phase,
            &branch_status,
            &repo_capabilities,
            cx,
        );

        // Build inline PR link for header
        let pr_link = self.render_header_pr_link(&branch_status);

        // Build inline CI status icon for header
        let ci_status = self.render_checks_status_icon(&branch_status, cx);

        let popover_open =
            self.state.checks_popover_open && !branch_status.checks.is_empty();
        let backdrop_listener =
            cx.listener(|this, _event: &gpui::MouseUpEvent, _window, cx| {
                this.on_close_checks_popover(cx);
            });

        let mut panel_div = div()
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.border_default)
            .child(
                section_header("Changes").child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(pr_link)
                        .child(ci_status)
                        .when(count > 0, |el| {
                            el.child(
                                div()
                                    .text_xs()
                                    .text_color(t.text_dim)
                                    .child(format!("{count}")),
                            )
                        })
                        .child(header_action),
                ),
            )
            // Status bar (working / error)
            .child(self.render_action_status(&action_phase, cx))
            .when_some(error, |p, message| {
                p.child(
                    div()
                        .mx_3()
                        .mt_1()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .bg(t.error_bg)
                        .text_color(t.text_primary)
                        .child(message.clone()),
                )
            })
            // Commit message input (shown when action is Commit)
            .when(panel_action == PanelAction::Commit && !is_busy, |el| {
                el.child(self.render_commit_input(&commit_message, cx))
            })
            .child(
                div()
                    .id("changes-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .children(if changes.is_empty() {
                        let mut info_items: Vec<AnyElement> = vec![div()
                            .px_3()
                            .py_2()
                            .text_xs()
                            .text_color(t.text_dim)
                            .child("Working tree clean")
                            .into_any_element()];

                        if branch_status.commits_ahead > 0 {
                            let n = branch_status.commits_ahead;
                            let label = if n == 1 {
                                "1 commit ahead".to_string()
                            } else {
                                format!("{n} commits ahead")
                            };
                            info_items.push(
                                div()
                                    .px_3()
                                    .pb_1()
                                    .text_xs()
                                    .text_color(t.text_dim)
                                    .child(label)
                                    .into_any_element(),
                            );
                        }

                        info_items
                    } else {
                        changes
                            .iter()
                            .map(|change| self.render_change_row(change, &staged_files, popover_open, cx))
                            .collect::<Vec<_>>()
                    }),
            );

        // Backdrop + popover rendered as last children so they paint on top
        if popover_open {
            panel_div = panel_div
                .child(
                    div()
                        .id("checks-backdrop")
                        .absolute()
                        .top(px(-2000.))
                        .left(px(-2000.))
                        .w(px(10000.))
                        .h(px(10000.))
                        .on_mouse_up(MouseButton::Left, backdrop_listener),
                )
                .child(self.render_checks_popover(&branch_status, &repo_capabilities, cx));
        }

        panel_div.into_any_element()
    }

    fn render_header_action_button(
        &self,
        action: &PanelAction,
        phase: &ActionPhase,
        branch_status: &BranchStatus,
        repo_capabilities: &RepoCapabilities,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let is_busy = matches!(phase, ActionPhase::Working(_));

        // For the Rebase action, decide whether to show "Rebase & Merge" or "Auto Merge"
        if *action == PanelAction::Rebase {
            let checks = &branch_status.checks;
            let all_checks_pass = !checks.is_empty()
                && checks
                    .iter()
                    .all(|c| matches!(c.bucket, CheckBucket::Pass | CheckBucket::Skipping));
            let auto_merge_enabled = branch_status.auto_merge_enabled;

            // If auto-merge is already enabled on the PR, show toggle to disable
            if auto_merge_enabled {
                return div()
                    .id("action-auto-merge")
                    .px_2()
                    .py(px(2.))
                    .rounded_md()
                    .text_xs()
                    .bg(t.accent_purple)
                    .text_color(t.bg_panel)
                    .when(is_busy, |el| el.opacity(0.5))
                    .when(!is_busy, |el| {
                        el.cursor_pointer()
                            .hover(|style| style.opacity(0.85))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _event, window, cx| {
                                    this.on_toggle_pr_auto_merge(window, cx);
                                }),
                            )
                    })
                    .child("Auto Merge")
                    .into_any_element();
            }

            // Checks still pending/failing: show auto-merge toggle if repo supports it
            if !all_checks_pass && repo_capabilities.auto_merge_allowed {
                return div()
                    .id("action-auto-merge")
                    .px_2()
                    .py(px(2.))
                    .rounded_md()
                    .text_xs()
                    .bg(t.accent_green)
                    .text_color(gpui::rgb(0x1e1e1e))
                    .when(is_busy, |el| el.opacity(0.5))
                    .when(!is_busy, |el| {
                        el.cursor_pointer()
                            .hover(|style| style.opacity(0.85))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _event, window, cx| {
                                    this.on_toggle_pr_auto_merge(window, cx);
                                }),
                            )
                    })
                    .child("Auto Merge")
                    .into_any_element();
            }

            // All checks pass (or no checks): show immediate merge if repo allows rebase
            if !repo_capabilities.rebase_merge_allowed {
                return div().into_any_element();
            }
        }

        let (button_id, label, bg_color): (&str, &str, Hsla) = match action {
            PanelAction::Commit => ("action-commit", "Commit", t.accent_green),
            PanelAction::Amend => ("action-amend", "Amend", t.accent_green),
            PanelAction::CreatePR => ("action-create-pr", "Create PR", t.accent_green),
            PanelAction::Rebase => ("action-rebase", "Rebase & Merge", t.accent_green),
            PanelAction::CloseSession => ("action-close", "Close Session", t.accent_red),
            PanelAction::None => return div().into_any_element(),
        };

        let action_clone = action.clone();

        div()
            .id(button_id)
            .px_2()
            .py(px(2.))
            .rounded_md()
            .text_xs()
            .bg(bg_color)
            .text_color(gpui::rgb(0x1e1e1e))
            .when(is_busy, |el| el.opacity(0.5))
            .when(!is_busy, |el| {
                el.cursor_pointer()
                    .hover(|style| style.opacity(0.85))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, event, window, cx| match action_clone {
                            PanelAction::Commit => this.on_commit(window, cx),
                            PanelAction::Amend => this.on_amend(window, cx),
                            PanelAction::CreatePR => this.on_create_pr(window, cx),
                            PanelAction::Rebase => this.on_rebase(window, cx),
                            PanelAction::CloseSession => this.on_close_session(event, window, cx),
                            PanelAction::None => {}
                        }),
                    )
            })
            .child(label)
            .into_any_element()
    }

    fn render_action_status(&self, phase: &ActionPhase, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();

        match phase {
            ActionPhase::Idle => div().into_any_element(),
            ActionPhase::Working(label) => div()
                .mx_3()
                .mt_1()
                .mb_1()
                .px_2()
                .py_1()
                .text_xs()
                .rounded_md()
                .bg(gpui::rgba(0xffffff08))
                .text_color(t.text_muted)
                .child(label.clone())
                .into_any_element(),
            ActionPhase::Error(msg) => div()
                .id("action-error")
                .mx_3()
                .mt_1()
                .mb_1()
                .px_2()
                .py_1()
                .text_xs()
                .rounded_md()
                .bg(t.error_bg)
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(t.accent_red)
                        .child(msg.clone()),
                )
                .child(
                    div()
                        .id("dismiss-action-err")
                        .text_color(t.text_dim)
                        .cursor_pointer()
                        .hover(|s| s.text_color(t.text_primary))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _event, _window, cx| {
                                this.on_dismiss_action_error(cx);
                            }),
                        )
                        .child(icon_x().size_3p5().text_color(t.text_dim)),
                )
                .into_any_element(),
        }
    }

    fn render_commit_input(&self, message: &str, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let display_text = if message.is_empty() {
            "Enter commit message...".to_string()
        } else {
            message.to_string()
        };
        let is_placeholder = message.is_empty();

        div()
            .mx_3()
            .mt_1()
            .mb_1()
            .child(
                div()
                    .id("commit-input")
                    .w_full()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .rounded_md()
                    .border_1()
                    .border_color(t.border_subtle)
                    .bg(gpui::rgba(0xffffff08))
                    .text_color(if is_placeholder {
                        t.text_dim
                    } else {
                        t.text_primary
                    })
                    .cursor_text()
                    .track_focus(&self.commit_input_focus)
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _event, window, cx| {
                            this.commit_input_focus.focus(window, cx);
                            cx.notify();
                        }),
                    )
                    .on_key_down(cx.listener(Self::on_commit_input_key_down))
                    .child(display_text),
            )
            .into_any_element()
    }

    fn render_header_pr_link(&self, branch_status: &BranchStatus) -> AnyElement {
        let t = theme();

        let Some(url) = branch_status.pr_url.clone() else {
            return div().into_any_element();
        };

        let pr_label = match branch_status.pr_number {
            Some(n) => format!("#{n}"),
            None => "PR".to_string(),
        };

        div()
            .id("header-pr-link")
            .flex()
            .items_center()
            .gap(px(2.))
            .cursor_pointer()
            .hover(|s| s.opacity(0.7))
            .on_mouse_up(MouseButton::Left, move |_event, _window, _cx| {
                let _ = std::process::Command::new("open").arg(&url).spawn();
            })
            .child(
                div()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(pr_label),
            )
            .child(
                icon_external_link()
                    .size(px(12.))
                    .text_color(t.text_dim),
            )
            .into_any_element()
    }

    fn render_checks_status_icon(
        &self,
        branch_status: &BranchStatus,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let checks = &branch_status.checks;

        if checks.is_empty() {
            return div().into_any_element();
        }

        let fail_count = checks
            .iter()
            .filter(|c| c.bucket == CheckBucket::Fail)
            .count();
        let pending_count = checks
            .iter()
            .filter(|c| c.bucket == CheckBucket::Pending)
            .count();

        let status_icon = if fail_count > 0 {
            icon_x()
                .size(px(14.))
                .text_color(t.accent_red)
                .into_any_element()
        } else if pending_count > 0 {
            icon_circle_dot()
                .size(px(14.))
                .text_color(t.accent_yellow)
                .into_any_element()
        } else {
            icon_check()
                .size(px(14.))
                .text_color(t.accent_green)
                .into_any_element()
        };

        div()
            .id("checks-status-icon")
            .flex_shrink_0()
            .cursor_pointer()
            .hover(|s| s.opacity(0.7))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _event, _window, cx| {
                    this.on_toggle_checks_popover(cx);
                }),
            )
            .child(status_icon)
            .into_any_element()
    }

    fn render_checks_popover(
        &self,
        branch_status: &BranchStatus,
        repo_capabilities: &RepoCapabilities,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();

        let checks = &branch_status.checks;
        let mut sorted_checks: Vec<&CiCheck> = checks.iter().collect();
        sorted_checks.sort_by_key(|c| match c.bucket {
            CheckBucket::Fail => 0,
            CheckBucket::Pending => 1,
            CheckBucket::Pass => 2,
            CheckBucket::Skipping => 3,
            CheckBucket::Cancel => 4,
        });

        let pr_url = branch_status.pr_url.clone();
        let pr_is_open = branch_status
            .pr_state
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("OPEN"))
            .unwrap_or(true)
            && pr_url.is_some()
            && !branch_status.pr_merged;

        let mut popover = div()
            .id("checks-popover")
            .absolute()
            .top(px(38.))
            .right(px(4.))
            .w(px(280.))
            .rounded_md()
            .border_1()
            .border_color(t.border_default)
            .bg(gpui::rgb(0x2d2d2d))
            .shadow_lg()
            .overflow_hidden()
            .on_mouse_move(|_event, _window, _cx| {
                // Stop mouse events from reaching elements behind the popover
            });

        // PR state row at top (if PR exists)
        if let Some(url) = &pr_url {
            let state_str = branch_status
                .pr_state
                .as_deref()
                .unwrap_or("OPEN")
                .to_uppercase();
            let is_merged = state_str == "MERGED";
            let is_closed = state_str == "CLOSED";

            let (badge_bg, badge_text_color): (Hsla, Hsla) = if is_merged {
                (gpui::rgba(0xa371f730).into(), t.accent_purple)
            } else if is_closed {
                (gpui::rgba(0xef292930).into(), t.accent_red)
            } else {
                (gpui::rgba(0x8ae23430).into(), t.accent_green)
            };

            let state_label = match state_str.as_str() {
                "MERGED" => "Merged",
                "CLOSED" => "Closed",
                _ => "Open",
            };

            let pr_label = match branch_status.pr_number {
                Some(n) => format!("#{n}"),
                None => "PR".to_string(),
            };

            let url_owned = url.clone();

            popover = popover.child(
                div()
                    .id("popover-pr-row")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px_2()
                    .py(px(6.))
                    .border_b_1()
                    .border_color(t.border_subtle)
                    .cursor_pointer()
                    .hover(|s| s.bg(gpui::rgba(0xffffff08)))
                    .on_mouse_up(MouseButton::Left, move |_event, _window, _cx| {
                        let _ = std::process::Command::new("open")
                            .arg(&url_owned)
                            .spawn();
                    })
                    // State badge pill
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(2.))
                            .rounded(px(12.))
                            .bg(badge_bg)
                            .text_xs()
                            .text_color(badge_text_color)
                            .child(state_label.to_string()),
                    )
                    // PR number
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(t.text_primary)
                            .child(pr_label),
                    )
                    .child(div().flex_1())
                    // External link icon
                    .child(
                        div().flex_shrink_0().child(
                            icon_external_link()
                                .size(px(12.))
                                .text_color(t.text_dim),
                        ),
                    ),
            );
        }

        // Check rows
        popover = popover.children(
            sorted_checks
                .iter()
                .enumerate()
                .map(|(i, check)| Self::render_popover_check_row(check, i)),
        );

        // Auto-merge toggle row (only when repo allows it and PR is open)
        if repo_capabilities.auto_merge_allowed && pr_is_open {
            let is_auto = branch_status.auto_merge_enabled;
            popover = popover.child(
                div()
                    .id("popover-auto-merge-toggle")
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_2()
                    .py(px(6.))
                    .border_t_1()
                    .border_color(t.border_subtle)
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.text_secondary)
                            .child("Auto-merge"),
                    )
                    .child(
                        div()
                            .id("auto-merge-btn")
                            .cursor_pointer()
                            .px_2()
                            .py(px(2.))
                            .rounded_sm()
                            .text_xs()
                            .bg(if is_auto { t.accent_green } else { t.bg_surface })
                            .text_color(if is_auto {
                                t.bg_panel
                            } else {
                                t.text_muted
                            })
                            .hover(|style| style.opacity(0.85))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _event, window, cx| {
                                    this.on_toggle_pr_auto_merge(window, cx);
                                }),
                            )
                            .child(if is_auto { "On" } else { "Off" }),
                    ),
            );
        }

        popover.into_any_element()
    }

    fn render_popover_check_row(check: &CiCheck, index: usize) -> AnyElement {
        let t = theme();

        let (status_icon, status_color) = match check.bucket {
            CheckBucket::Pass => (
                icon_check().size(px(12.)).into_any_element(),
                t.accent_green,
            ),
            CheckBucket::Fail => (
                icon_x().size(px(12.)).into_any_element(),
                t.accent_red,
            ),
            CheckBucket::Pending => (
                icon_circle_dot().size(px(12.)).into_any_element(),
                t.accent_yellow,
            ),
            CheckBucket::Skipping | CheckBucket::Cancel => (
                icon_minus().size(px(12.)).into_any_element(),
                t.text_dim,
            ),
        };

        let row_id = gpui::ElementId::Name(format!("popover-check-{index}").into());
        let link = check.link.clone();
        let has_link = link.is_some();

        div()
            .id(row_id)
            .group("popover-check-row")
            .flex()
            .items_center()
            .gap(px(6.))
            .px_2()
            .py(px(4.))
            .when(has_link, |el| {
                el.cursor_pointer()
                    .hover(|s| s.bg(gpui::rgba(0xffffff08)))
            })
            .on_mouse_up(MouseButton::Left, move |_event, _window, _cx| {
                if let Some(url) = &link {
                    let _ = std::process::Command::new("open").arg(url).spawn();
                }
            })
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(status_color)
                    .child(status_icon),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_xs()
                    .text_color(t.text_secondary)
                    .child(check.name.clone()),
            )
            .when(has_link, |el| {
                el.child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(t.text_dim)
                        .invisible()
                        .group_hover("popover-check-row", |s| s.visible())
                        .child("Details"),
                )
            })
            .into_any_element()
    }

    fn on_toggle_checks_popover(&mut self, cx: &mut Context<Self>) {
        self.state.checks_popover_open = !self.state.checks_popover_open;
        cx.notify();
    }

    fn on_close_checks_popover(&mut self, cx: &mut Context<Self>) {
        self.state.checks_popover_open = false;
        cx.notify();
    }

    fn render_change_row(
        &self,
        change: &GitChange,
        staged_files: &std::collections::HashSet<String>,
        popover_open: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let file_path = change.path.clone();
        let status_code = change.status_code.clone();
        let is_staged = staged_files.contains(&change.path);

        let is_viewing = self
            .state
            .viewing_diff
            .as_ref()
            .is_some_and(|d| d.file_path == change.path);

        let status_color = match change.status_code.as_str() {
            "A" | "??" => t.accent_green,
            "D" => t.accent_red,
            _ => t.text_muted,
        };

        let change_row_id = gpui::ElementId::Name(format!("change-{}", change.path).into());
        let checkbox_id = gpui::ElementId::Name(format!("chk-{}", change.path).into());

        let toggle_path = change.path.clone();

        div()
            .id(change_row_id)
            .group("change-row")
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .py_1()
            .bg(if is_viewing {
                t.selection_medium
            } else {
                t.transparent
            })
            // Checkbox
            .child(
                div()
                    .id(checkbox_id)
                    .w(px(14.))
                    .h(px(14.))
                    .flex_shrink_0()
                    .rounded(px(3.))
                    .border_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .when(is_staged, |el| {
                        el.bg(t.accent_green)
                            .border_color(t.accent_green)
                            .text_color(gpui::rgb(0x1e1e1e))
                            .child(icon_check().size(px(10.)).text_color(gpui::rgb(0x1e1e1e)))
                    })
                    .when(!is_staged, |el| el.border_color(t.text_dim))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _event, _window, cx| {
                            this.on_toggle_staged(toggle_path.clone(), cx);
                        }),
                    ),
            )
            // Status code
            .child(
                div()
                    .text_xs()
                    .text_color(status_color)
                    .w(px(20.))
                    .flex_shrink_0()
                    .child(change.status_code.clone()),
            )
            // File path (clickable for diff)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_sm()
                    .cursor_pointer()
                    .when(!popover_open, |el| {
                        el.hover(|style| style.bg(t.hover_overlay))
                    })
                    .text_color(if is_viewing {
                        t.text_primary
                    } else {
                        t.text_secondary
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, event, window, cx| {
                            this.on_open_diff(
                                file_path.clone(),
                                status_code.clone(),
                                event,
                                window,
                                cx,
                            );
                        }),
                    )
                    .child(change.path.clone()),
            )
            // +/- stats on hover
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .invisible()
                    .when(!popover_open, |el| {
                        el.group_hover("change-row", |style| style.visible())
                    })
                    .when_some(change.additions, |el, adds| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(t.accent_green)
                                .child(format!("+{adds}")),
                        )
                    })
                    .when_some(change.deletions.filter(|&d| d > 0), |el, dels| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(t.accent_red)
                                .child(format!("-{dels}")),
                        )
                    }),
            )
            .into_any_element()
    }
}
