use std::rc::Rc;

use gpui::{AnyElement, Context, MouseButton, MouseUpEvent, Window, div, prelude::*, px, uniform_list};

use crate::git;
use crate::icons::icon_x;
use crate::state::{DiffData, SplitLine, SplitLineKind};
use crate::theme::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn on_open_diff(
        &mut self,
        file_path: String,
        status_code: String,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };

        let working_dir = self.working_dir(&repo);

        let view = cx.entity().clone();
        let file_path_clone = file_path.clone();
        let status_code_clone = status_code.clone();

        window
            .spawn(cx, async move |cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        git::compute_file_diff(&working_dir, &file_path_clone, &status_code_clone)
                    })
                    .await;

                cx.update(|_, cx| {
                    view.update(cx, |this, cx| {
                        match result {
                            Ok(diff_data) => {
                                this.state.viewing_diff = Some(diff_data);
                            }
                            Err(error) => {
                                this.state.flash_error =
                                    Some(format!("Failed to load diff: {error}"));
                            }
                        }
                        cx.notify();
                    })
                })
                .ok();
            })
            .detach();
    }

    pub(crate) fn on_close_diff(&mut self, cx: &mut Context<Self>) {
        self.state.viewing_diff = None;
        cx.notify();
    }

    pub(crate) fn render_diff_view(
        &self,
        diff_data: &DiffData,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py(px(6.))
            .bg(t.bg_panel)
            .border_b_1()
            .border_color(t.border_default)
            .child({
                let (dir_part, file_part) = match diff_data.file_path.rfind('/') {
                    Some(pos) => (
                        Some(diff_data.file_path[..=pos].to_string()),
                        diff_data.file_path[pos + 1..].to_string(),
                    ),
                    None => (None, diff_data.file_path.clone()),
                };
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .text_sm()
                            .when_some(dir_part, |el, dir| {
                                el.child(
                                    div()
                                        .text_color(t.text_dim)
                                        .child(dir),
                                )
                            })
                            .child(
                                div()
                                    .text_color(t.text_primary)
                                    .child(file_part),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.accent_green)
                            .child(format!("+{}", diff_data.additions)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.accent_red)
                            .child(format!("-{}", diff_data.deletions)),
                    )
            })
            .child(
                div()
                    .id("close-diff")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_xs()
                    .bg(t.bg_elevated)
                    .text_color(t.text_muted)
                    .hover(|style| style.bg(t.bg_elevated_hover).text_color(t.text_primary))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.on_close_diff(cx);
                        }),
                    )
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(icon_x().size_3().text_color(t.text_muted))
                    .child("Esc"),
            );

        let lines = Rc::new(diff_data.lines.clone());
        let line_count = lines.len();

        let diff_list = uniform_list("diff-lines", line_count, move |range, _window, _cx| {
            range
                .map(|ix| render_split_line(&lines[ix]))
                .collect::<Vec<_>>()
        })
        .flex_1()
        .min_h_0()
        .bg(t.bg_base);

        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(header)
            .child(diff_list)
            .into_any_element()
    }
}

fn render_split_line(line: &SplitLine) -> AnyElement {
    let t = theme();

    let (left_bg, right_bg) = match line.kind {
        SplitLineKind::Equal => (t.transparent, t.transparent),
        SplitLineKind::Insert => (t.transparent, t.diff_add_bg),
        SplitLineKind::Delete => (t.diff_del_bg, t.transparent),
        SplitLineKind::Replace => (t.diff_del_bg, t.diff_add_bg),
    };

    let left_text_color = match line.kind {
        SplitLineKind::Delete | SplitLineKind::Replace => t.diff_del_text,
        _ => t.text_muted,
    };

    let right_text_color = match line.kind {
        SplitLineKind::Insert | SplitLineKind::Replace => t.diff_add_text,
        _ => t.text_muted,
    };

    div()
        .flex()
        .w_full()
        .text_xs()
        .whitespace_nowrap()
        .child(
            // Left half (old)
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .overflow_hidden()
                .bg(left_bg)
                .child(
                    div()
                        .w(px(40.))
                        .flex_shrink_0()
                        .text_right()
                        .px_1()
                        .border_r_1()
                        .border_color(t.border_subtle)
                        .text_color(t.text_line_number)
                        .child(
                            line.old_lineno
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .px_1()
                        .text_color(left_text_color)
                        .child(line.old_text.clone()),
                ),
        )
        .child(
            // Divider
            div().w(px(1.)).flex_shrink_0().bg(t.border_default),
        )
        .child(
            // Right half (new)
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .overflow_hidden()
                .bg(right_bg)
                .child(
                    div()
                        .w(px(40.))
                        .flex_shrink_0()
                        .text_right()
                        .px_1()
                        .border_r_1()
                        .border_color(t.border_subtle)
                        .text_color(t.text_line_number)
                        .child(
                            line.new_lineno
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .px_1()
                        .text_color(right_text_color)
                        .child(line.new_text.clone()),
                ),
        )
        .into_any_element()
}
