use gpui::{AnyElement, Context, MouseButton, div, prelude::*, px};

use crate::theme::theme;

use super::WorkspaceView;

impl WorkspaceView {
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
            // Right side: sidebar toggle
            .child(
                div()
                    .flex()
                    .items_center()
                    .pr_2()
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
