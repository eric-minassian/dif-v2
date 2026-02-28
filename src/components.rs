use gpui::{Div, div, prelude::*, px};

use crate::theme::theme;

pub enum PanelSide {
    Left,
    Right,
}

pub fn button() -> Div {
    let t = theme();
    div()
        .px_3()
        .py_1()
        .rounded_md()
        .bg(t.bg_elevated)
        .text_color(t.text_primary)
        .hover(|style| style.bg(t.bg_elevated_hover).cursor_pointer())
}

pub fn section_header(title: &str) -> Div {
    let t = theme();
    div()
        .flex()
        .items_center()
        .justify_between()
        .px_3()
        .py_2()
        .border_b_1()
        .border_color(t.border_default)
        .child(div().font_weight(gpui::FontWeight::BOLD).child(title.to_string()))
}

pub fn panel(side: PanelSide) -> Div {
    let t = theme();
    let base = div().h_full().flex().flex_col();
    match side {
        PanelSide::Left => base
            .bg(t.bg_panel)
            .border_r_1()
            .border_color(t.border_default),
        PanelSide::Right => base
            .bg(t.bg_surface)
            .border_l_1()
            .border_color(t.border_default),
    }
}

pub fn empty_state(message: &str) -> Div {
    let t = theme();
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(t.bg_base)
        .text_color(t.text_muted)
        .child(
            div()
                .max_w(px(320.))
                .text_center()
                .child(message.to_string()),
        )
}

