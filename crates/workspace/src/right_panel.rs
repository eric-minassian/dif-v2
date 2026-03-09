use crate::runtime::RebaseConflict;
use git::BranchStatus;
use ui::prelude::*;
use ui::{PanelSide, panel};

use crate::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn render_right_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        panel(PanelSide::Right)
            .w(px(self.state.right_sidebar_width))
            .child(self.render_changes_panel(cx))
            .into_any_element()
    }

    pub(crate) fn render_collapsed_right_sidebar(&self) -> AnyElement {
        div().into_any_element()
    }

    fn render_conflict_banner(
        &self,
        conflict: &RebaseConflict,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();

        v_flex()
            .mx_3()
            .mt_1()
            .px_2()
            .py_2()
            .gap_2()
            .rounded_sm()
            .bg(t.error_bg)
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(t.accent_yellow)
                    .child("Rebase conflict"),
            )
            .child(
                v_flex().gap(px(2.)).children(
                    conflict
                        .conflict_files
                        .iter()
                        .map(|f| {
                            div()
                                .text_xs()
                                .text_color(t.text_secondary)
                                .child(f.clone())
                        })
                        .collect::<Vec<_>>(),
                ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div()
                            .id("copy-conflict-prompt")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(t.accent_blue)
                            .cursor_pointer()
                            .bg(t.bg_elevated)
                            .hover(|s| s.bg(t.bg_elevated_hover))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.on_copy_conflict_prompt(cx);
                                this.state.flash_error =
                                    Some("Conflict prompt copied to clipboard".into());
                                cx.notify();
                            }))
                            .child("Copy Prompt"),
                    )
                    .child(
                        div()
                            .id("abort-rebase")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(t.accent_red)
                            .cursor_pointer()
                            .bg(t.bg_elevated)
                            .hover(|s| s.bg(t.bg_elevated_hover))
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.on_abort_rebase(window, cx);
                            }))
                            .child("Abort Rebase"),
                    ),
            )
            .into_any_element()
    }

    fn render_changes_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let project_runtime = self.selected_project_runtime();
        let snapshot = project_runtime.map(|runtime| &runtime.git_snapshot);
        let changes = snapshot
            .map(|snapshot| snapshot.changes.as_slice())
            .unwrap_or(&[]);
        let error = snapshot.and_then(|snapshot| snapshot.last_error.as_ref());
        let rebase_conflict = project_runtime.and_then(|rt| rt.rebase_conflict.clone());

        let empty_staged = std::collections::HashSet::new();
        let staged_files = project_runtime
            .map(|rt| &rt.staged_files)
            .unwrap_or(&empty_staged);
        let default_branch_status = BranchStatus::default();
        let branch_status = project_runtime
            .map(|rt| &rt.branch_status)
            .unwrap_or(&default_branch_status);

        v_flex()
            .flex_1()
            .min_h_0()
            .when_some(rebase_conflict, |p, conflict| {
                p.child(self.render_conflict_banner(&conflict, cx))
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
                        vec![
                            div()
                                .flex_1()
                                .min_h_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .py_4()
                                .text_xs()
                                .text_color(t.text_dim)
                                .child(label)
                                .into_any_element(),
                        ]
                    } else {
                        changes
                            .iter()
                            .map(|change| self.render_change_row(change, staged_files, false, cx))
                            .collect::<Vec<_>>()
                    }),
            )
            .into_any_element()
    }
}
