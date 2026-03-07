use std::sync::LazyLock;

use gpui::Hsla;

pub struct Theme {
    // Surfaces
    pub bg_base: Hsla,
    pub bg_surface: Hsla,
    pub bg_panel: Hsla,
    pub bg_elevated: Hsla,
    pub bg_elevated_hover: Hsla,
    pub bg_titlebar: Hsla,

    // Borders
    pub border_default: Hsla,
    pub border_subtle: Hsla,

    // Text
    pub text_primary: Hsla,
    pub text_secondary: Hsla,
    pub text_muted: Hsla,
    pub text_dim: Hsla,
    pub text_line_number: Hsla,

    // Accents
    pub accent: Hsla,
    pub accent_blue: Hsla,
    pub accent_green: Hsla,
    pub accent_red: Hsla,
    pub accent_yellow: Hsla,
    pub accent_purple: Hsla,
    pub diff_add_text: Hsla,
    pub diff_del_text: Hsla,

    // Alpha / overlays
    pub transparent: Hsla,
    pub selection_faint: Hsla,
    pub selection_medium: Hsla,
    pub hover_overlay: Hsla,
    pub diff_add_bg: Hsla,
    pub diff_del_bg: Hsla,
    pub diff_collapsed_bg: Hsla,
    pub diff_collapsed_text: Hsla,
    pub diff_collapsed_hover: Hsla,
    pub error_bg: Hsla,
}

static THEME: LazyLock<Theme> = LazyLock::new(|| Theme {
    // Surfaces — neutral grays matching terminal #1E1E1E
    bg_base: gpui::rgb(0x181818).into(),
    bg_surface: gpui::rgb(0x1e1e1e).into(),
    bg_panel: gpui::rgb(0x222222).into(),
    bg_elevated: gpui::rgb(0x2d2d2d).into(),
    bg_elevated_hover: gpui::rgb(0x3a3a3a).into(),
    bg_titlebar: gpui::rgb(0x1b1b1b).into(),

    // Borders
    border_default: gpui::rgb(0x333333).into(),
    border_subtle: gpui::rgb(0x2a2a2a).into(),

    // Text — matching terminal foreground #D4D4D4
    text_primary: gpui::rgb(0xd4d4d4).into(),
    text_secondary: gpui::rgb(0xb0b0b0).into(),
    text_muted: gpui::rgb(0x888888).into(),
    text_dim: gpui::rgb(0x666666).into(),
    text_line_number: gpui::rgb(0x555555).into(),

    // Accents — using terminal ANSI palette
    accent: gpui::rgb(0xd4d4d4).into(),
    accent_blue: gpui::rgb(0x58a6ff).into(),
    accent_green: gpui::rgb(0x8ae234).into(),
    accent_red: gpui::rgb(0xef2929).into(),
    accent_yellow: gpui::rgb(0xfce94f).into(),
    accent_purple: gpui::rgb(0xa371f7).into(),
    diff_add_text: gpui::rgb(0x8ae234).into(),
    diff_del_text: gpui::rgb(0xef2929).into(),

    // Alpha / overlays — neutral gray tints
    transparent: gpui::rgba(0x00000000).into(),
    selection_faint: gpui::rgba(0xffffff18).into(),
    selection_medium: gpui::rgba(0xffffff28).into(),
    hover_overlay: gpui::rgba(0xffffff15).into(),
    diff_add_bg: gpui::rgba(0x4e9a0618).into(),
    diff_del_bg: gpui::rgba(0xcc000018).into(),
    diff_collapsed_bg: gpui::rgba(0x58a6ff0c).into(),
    diff_collapsed_text: gpui::rgb(0x58a6ff).into(),
    diff_collapsed_hover: gpui::rgba(0x58a6ff18).into(),
    error_bg: gpui::rgba(0xef292930).into(),
});

pub fn theme() -> &'static Theme {
    &THEME
}

/// Semantic color names that resolve to `Hsla` via the active theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Default,
    Muted,
    Dim,
    Green,
    Red,
    Yellow,
    Custom(Hsla),
}

impl Color {
    pub fn hsla(self) -> Hsla {
        let t = theme();
        match self {
            Color::Default => t.text_primary,
            Color::Muted => t.text_muted,
            Color::Dim => t.text_dim,
            Color::Green => t.accent_green,
            Color::Red => t.accent_red,
            Color::Yellow => t.accent_yellow,
            Color::Custom(color) => color,
        }
    }
}

impl From<Hsla> for Color {
    fn from(color: Hsla) -> Self {
        Color::Custom(color)
    }
}
