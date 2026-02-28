use gpui::{Svg, svg};

pub fn icon(name: &str) -> Svg {
    svg().path(format!("icons/{name}.svg"))
}

// Sidebar toggles
pub fn icon_panel_left() -> Svg {
    icon("panel_left")
}

pub fn icon_panel_right() -> Svg {
    icon("panel_right")
}

// Collapse/expand
pub fn icon_chevron_right() -> Svg {
    icon("chevron_right")
}

pub fn icon_chevron_down() -> Svg {
    icon("chevron_down")
}

// Actions
pub fn icon_check() -> Svg {
    icon("check")
}

pub fn icon_x() -> Svg {
    icon("x")
}

pub fn icon_plus() -> Svg {
    icon("plus")
}

pub fn icon_settings() -> Svg {
    icon("settings")
}

pub fn icon_circle_dot() -> Svg {
    icon("circle_dot")
}

pub fn icon_external_link() -> Svg {
    icon("external_link")
}

pub fn icon_minus() -> Svg {
    icon("minus")
}

