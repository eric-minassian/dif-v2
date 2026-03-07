use gpui::{App, ClickEvent, ElementId, Pixels, Svg, svg};

use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconName {
    Check,
    ChevronDown,
    ChevronRight,
    CircleDot,
    ExternalLink,
    GitPullRequest,
    HelpCircle,
    Minus,
    PanelLeft,
    PanelRight,
    Plus,
    Settings,
    X,
}

impl IconName {
    pub fn path(self) -> &'static str {
        match self {
            Self::Check => "icons/check.svg",
            Self::ChevronDown => "icons/chevron_down.svg",
            Self::ChevronRight => "icons/chevron_right.svg",
            Self::CircleDot => "icons/circle_dot.svg",
            Self::ExternalLink => "icons/external_link.svg",
            Self::GitPullRequest => "icons/git_pull_request.svg",
            Self::HelpCircle => "icons/help_circle.svg",
            Self::Minus => "icons/minus.svg",
            Self::PanelLeft => "icons/panel_left.svg",
            Self::PanelRight => "icons/panel_right.svg",
            Self::Plus => "icons/plus.svg",
            Self::Settings => "icons/settings.svg",
            Self::X => "icons/x.svg",
        }
    }
}

// -- Icon (RenderOnce) -------------------------------------------------------

#[derive(IntoElement)]
pub struct Icon {
    name: IconName,
    color: Option<Color>,
    size: Pixels,
}

impl Icon {
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            color: None,
            size: px(16.),
        }
    }

    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Build just the raw Svg element (useful when you need to style it further).
    pub fn svg(self) -> Svg {
        let s = svg().path(self.name.path()).size(self.size).flex_none();
        match self.color {
            Some(color) => s.text_color(color.hsla()),
            None => s,
        }
    }
}

impl RenderOnce for Icon {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        self.svg()
    }
}

// -- IconButton (RenderOnce) -------------------------------------------------

type IconClickHandler = dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static;

#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: IconName,
    icon_size: Pixels,
    icon_color: Color,
    hover_color: Option<Color>,
    on_click: Option<Box<IconClickHandler>>,
    visible_on_hover: Option<SharedString>,
}

impl IconButton {
    pub fn new(id: impl Into<ElementId>, icon: IconName) -> Self {
        Self {
            id: id.into(),
            icon,
            icon_size: px(14.),
            icon_color: Color::Dim,
            hover_color: Some(Color::Default),
            on_click: None,
            visible_on_hover: None,
        }
    }

    pub fn icon_size(mut self, size: Pixels) -> Self {
        self.icon_size = size;
        self
    }

    pub fn icon_color(mut self, color: impl Into<Color>) -> Self {
        self.icon_color = color.into();
        self
    }

    pub fn hover_color(mut self, color: impl Into<Color>) -> Self {
        self.hover_color = Some(color.into());
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }

    /// Make this button invisible until the given group is hovered.
    pub fn visible_on_hover(mut self, group: impl Into<SharedString>) -> Self {
        self.visible_on_hover = Some(group.into());
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let icon = Icon::new(self.icon)
            .size(self.icon_size)
            .color(self.icon_color);

        let mut el = div().id(self.id).cursor_pointer().px_1().flex_shrink_0();

        if let Some(group) = &self.visible_on_hover {
            el = el.invisible().group_hover(group.clone(), |s| s.visible());
        }

        if let Some(hover_color) = self.hover_color {
            let hsla = hover_color.hsla();
            el = el.hover(move |style| style.text_color(hsla));
        }

        if let Some(handler) = self.on_click {
            el = el.on_click(handler);
        }

        el.child(icon)
    }
}

// -- DiffStat (RenderOnce) ---------------------------------------------------

#[derive(IntoElement)]
pub struct DiffStat {
    additions: Option<u32>,
    deletions: Option<u32>,
}

impl DiffStat {
    pub fn new() -> Self {
        Self {
            additions: None,
            deletions: None,
        }
    }

    pub fn additions(mut self, n: u32) -> Self {
        self.additions = Some(n);
        self
    }

    pub fn deletions(mut self, n: u32) -> Self {
        self.deletions = Some(n);
        self
    }
}

impl Default for DiffStat {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DiffStat {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        h_flex()
            .gap_1()
            .text_xs()
            .flex_shrink_0()
            .when_some(self.additions, |el, adds| {
                el.child(
                    div()
                        .text_color(Color::Green.hsla())
                        .child(format!("+{adds}")),
                )
            })
            .when_some(self.deletions.filter(|&d| d > 0), |el, dels| {
                el.child(
                    div()
                        .text_color(Color::Red.hsla())
                        .child(format!("-{dels}")),
                )
            })
    }
}
