pub use gpui::prelude::*;
pub use gpui::{AnyElement, Context, MouseButton, SharedString, Window, div, px};

pub use crate::icon::{DiffStat, Icon, IconButton, IconName};
pub use crate::theme::{Color, theme};

pub fn h_flex() -> gpui::Div {
    div().flex().items_center()
}

pub fn v_flex() -> gpui::Div {
    div().flex().flex_col()
}
