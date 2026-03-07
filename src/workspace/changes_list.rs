use gpui::{SharedString, div, prelude::*, px, AnyElement, Context, MouseButton};

use crate::icons::icon_check;
use crate::state::GitChange;
use crate::theme::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn render_change_row(
        &self,
        change: &GitChange,
        staged_files: &std::collections::HashSet<String>,
        popover_open: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let path = change.path.clone();
        let status_code = change.status_code.clone();
        let is_staged = staged_files.contains(&path);

        let is_viewing = self
            .state
            .viewing_diff
            .as_ref()
            .is_some_and(|d| d.file_path == path);

        let status_color = match change.status_code.as_str() {
            "A" | "??" => t.accent_green,
            "D" => t.accent_red,
            _ => t.text_muted,
        };

        let change_row_id = gpui::ElementId::Name(format!("change-{}", path).into());
        let checkbox_id = gpui::ElementId::Name(format!("chk-{}", path).into());

        let toggle_path = path.clone();
        let file_path = path.clone();

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
            .when(is_viewing, |el| {
                el.border_l_2().border_color(t.accent_blue)
            })
            .when(!popover_open, |el| {
                el.hover(|style| style.bg(t.hover_overlay))
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
                        el.bg(t.accent_blue)
                            .border_color(t.accent_blue)
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
                    .child(SharedString::from(change.status_code.clone())),
            )
            // File path (clickable for diff)
            .child({
                let (dir_part, file_part) = match path.rfind('/') {
                    Some(pos) => (
                        Some(SharedString::from(path[..=pos].to_string())),
                        SharedString::from(path[pos + 1..].to_string()),
                    ),
                    None => (None, SharedString::from(path)),
                };
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .flex()
                    .text_xs()
                    .cursor_pointer()
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
                    .when_some(dir_part, |el, dir| {
                        el.child(
                            div()
                                .flex_shrink_0()
                                .text_color(t.text_dim)
                                .child(dir),
                        )
                    })
                    .child(
                        div()
                            .text_color(if is_viewing {
                                t.text_primary
                            } else {
                                t.text_secondary
                            })
                            .child(file_part),
                    )
            })
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
