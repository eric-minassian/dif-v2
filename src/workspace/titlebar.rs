use gpui::{AnyElement, Context, MouseButton, div, prelude::*, px};

use crate::state::UpdateStatus;
use crate::theme::theme;

use super::WorkspaceView;

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
                    div()
                        .id("update-available")
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .text_color(t.accent_green)
                        .hover(|style| style.cursor_pointer().bg(t.hover_overlay))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| {
                                this.on_start_update(url.clone(), window, cx);
                            }),
                        )
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
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, window, cx| {
                                this.spawn_update_check(window, cx);
                            }),
                        )
                        .child("Update failed - retry")
                        .into_any_element(),
                )
            }
            UpdateStatus::Idle | UpdateStatus::Checking => None,
        }
    }

    pub(crate) fn render_titlebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let left_collapsed = self.state.left_sidebar_collapsed;
        let right_collapsed = self.state.right_sidebar_collapsed;

        div()
            .h(px(36.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .bg(t.bg_titlebar)
            .border_b_1()
            .border_color(t.border_subtle)
            // Left side: traffic light padding + sidebar toggle
            .child(
                div()
                    .flex()
                    .items_center()
                    .pl(px(78.))
                    .child(
                        div()
                            .id("toggle-left-sidebar")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(if left_collapsed {
                                t.text_dim
                            } else {
                                t.text_muted
                            })
                            .hover(|style| {
                                style.text_color(t.text_primary).cursor_pointer()
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.on_toggle_left_sidebar(cx);
                                }),
                            )
                            .child("⊞"),
                    ),
            )
            // Right side: update indicator + sidebar toggle
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .pr_2()
                    .children(self.render_update_indicator(cx))
                    .child(
                        div()
                            .id("toggle-right-sidebar")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(if right_collapsed {
                                t.text_dim
                            } else {
                                t.text_muted
                            })
                            .hover(|style| {
                                style.text_color(t.text_primary).cursor_pointer()
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.on_toggle_right_sidebar(cx);
                                }),
                            )
                            .child("⊟"),
                    ),
            )
            .into_any_element()
    }
}
