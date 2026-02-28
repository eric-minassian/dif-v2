use gpui::{AnyElement, Context, Hsla, MouseButton, div, prelude::*, px};

use crate::components::{panel, section_header, PanelSide};
use crate::icons::{icon_check, icon_x};
use crate::state::{ActionPhase, BranchStatus, GitChange};
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
        let commit_message = project_runtime
            .map(|rt| rt.commit_message.clone())
            .unwrap_or_default();

        let panel_action = derive_panel_action(has_changes, staged_count, &branch_status);
        let is_busy = matches!(action_phase, ActionPhase::Working(_));

        // Build the action button for the header bar
        let header_action = self.render_header_action_button(&panel_action, &action_phase, cx);

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.border_default)
            .child(
                section_header("Changes")
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
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

                        if let Some(url) = &branch_status.pr_url {
                            info_items.push(
                                div()
                                    .px_3()
                                    .pb_1()
                                    .text_xs()
                                    .text_color(t.text_dim)
                                    .child(url.clone())
                                    .into_any_element(),
                            );
                        }

                        info_items
                    } else {
                        changes
                            .iter()
                            .map(|change| self.render_change_row(change, &staged_files, cx))
                            .collect::<Vec<_>>()
                    }),
            )
            .into_any_element()
    }

    fn render_header_action_button(
        &self,
        action: &PanelAction,
        phase: &ActionPhase,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let is_busy = matches!(phase, ActionPhase::Working(_));

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
                        cx.listener(move |this, event, window, cx| {
                            match action_clone {
                                PanelAction::Commit => this.on_commit(window, cx),
                                PanelAction::Amend => this.on_amend(window, cx),
                                PanelAction::CreatePR => this.on_create_pr(window, cx),
                                PanelAction::Rebase => this.on_rebase(window, cx),
                                PanelAction::CloseSession => {
                                    this.on_close_session(event, window, cx)
                                }
                                PanelAction::None => {}
                            }
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

    fn render_change_row(
        &self,
        change: &GitChange,
        staged_files: &std::collections::HashSet<String>,
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

        let change_row_id =
            gpui::ElementId::Name(format!("change-{}", change.path).into());
        let checkbox_id =
            gpui::ElementId::Name(format!("chk-{}", change.path).into());

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
                    .when(!is_staged, |el| {
                        el.border_color(t.text_dim)
                    })
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
                    .hover(|style| style.bg(t.hover_overlay))
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
                    .group_hover("change-row", |style| style.visible())
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
