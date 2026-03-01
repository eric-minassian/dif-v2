use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton, Window};

use crate::components::{panel, section_header, PanelSide};
use crate::state::ActionPhase;
use crate::theme::theme;

use super::panel_action::{derive_panel_action, PanelAction};
use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn render_right_sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let top = self.render_changes_panel(window, cx);
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

    fn render_changes_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let project_runtime = self.selected_project_runtime();
        let snapshot = project_runtime.map(|runtime| &runtime.git_snapshot);
        let changes = snapshot
            .map(|snapshot| snapshot.changes.as_slice())
            .unwrap_or(&[]);
        let error = snapshot.and_then(|snapshot| snapshot.last_error.as_ref());
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
                        .child(ci_status)
                        .child(pr_link),
                ),
            )
            // Full-width primary action button / status
            .when(panel_action != PanelAction::None || !matches!(action_phase, ActionPhase::Idle), |el| {
                el.child(
                    div()
                        .px_3()
                        .pt_2()
                        .pb_1()
                        .child(self.render_action_or_status(&panel_action, &action_phase, header_action, cx)),
                )
            })
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
                el.child(self.render_commit_input(&commit_message, window, cx))
            })
            .child(
                div()
                    .id("changes-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .children(if changes.is_empty() {
                        let mut label = "Working tree clean".to_string();
                        if branch_status.commits_ahead > 0 {
                            let n = branch_status.commits_ahead;
                            if n == 1 {
                                label.push_str(" · 1 commit ahead");
                            } else {
                                label.push_str(&format!(" · {n} commits ahead"));
                            }
                        }
                        vec![div()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .py_4()
                            .text_xs()
                            .text_color(t.text_dim)
                            .child(label)
                            .into_any_element()]
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
                        .occlude()
                        .absolute()
                        .top(px(-2000.))
                        .left(px(-2000.))
                        .w(px(10000.))
                        .h(px(10000.))
                        .on_mouse_up(MouseButton::Left, backdrop_listener),
                )
                .child(self.render_checks_popover(&branch_status, cx));
        }

        panel_div.into_any_element()
    }
}
