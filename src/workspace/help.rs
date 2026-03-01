use gpui::{AnyElement, Context, MouseButton, div, prelude::*, px};

use crate::icons::icon_x;
use crate::theme::theme;

use super::WorkspaceView;

struct KeybindingEntry {
    keys: &'static str,
    description: &'static str,
}

struct KeybindingSection {
    title: &'static str,
    entries: &'static [KeybindingEntry],
}

const SECTIONS: &[KeybindingSection] = &[
    KeybindingSection {
        title: "GENERAL",
        entries: &[
            KeybindingEntry { keys: "Cmd + ,", description: "Open settings" },
            KeybindingEntry { keys: "Cmd + /", description: "Toggle keyboard shortcuts" },
            KeybindingEntry { keys: "Cmd + Q", description: "Quit" },
            KeybindingEntry { keys: "Cmd + H", description: "Hide app" },
            KeybindingEntry { keys: "Cmd + M", description: "Minimize window" },
            KeybindingEntry { keys: "Escape", description: "Close current view" },
        ],
    },
    KeybindingSection {
        title: "GIT ACTIONS",
        entries: &[
            KeybindingEntry { keys: "Cmd + Enter", description: "Run current git action (commit, amend, create PR, rebase)" },
            KeybindingEntry { keys: "Cmd + R", description: "Refresh git status" },
        ],
    },
    KeybindingSection {
        title: "SESSIONS",
        entries: &[
            KeybindingEntry { keys: "Cmd + N", description: "New session in current project" },
            KeybindingEntry { keys: "Cmd + 1-9", description: "Switch to session by index" },
        ],
    },
    KeybindingSection {
        title: "SIDEBARS",
        entries: &[
            KeybindingEntry { keys: "Cmd + B", description: "Toggle left sidebar" },
            KeybindingEntry { keys: "Cmd + Shift + B", description: "Toggle right sidebar" },
        ],
    },
    KeybindingSection {
        title: "TERMINALS",
        entries: &[
            KeybindingEntry { keys: "Cmd + `", description: "Switch focus between main & side terminal" },
            KeybindingEntry { keys: "Cmd + T", description: "New side terminal tab" },
            KeybindingEntry { keys: "Cmd + W", description: "Close side terminal tab" },
        ],
    },
];

impl WorkspaceView {
    pub(crate) fn render_help_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_2()
            .bg(t.bg_panel)
            .border_b_1()
            .border_color(t.border_default)
            .child(
                div()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_sm()
                    .child("Keyboard Shortcuts"),
            )
            .child(
                div()
                    .id("close-help")
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
                            this.state.viewing_help = false;
                            cx.notify();
                        }),
                    )
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(icon_x().size_3().text_color(t.text_muted))
                    .child("Esc"),
            );

        let mut content = div()
            .id("help-content")
            .flex_1()
            .min_h_0()
            .overflow_scroll()
            .p_4()
            .flex()
            .flex_col()
            .gap_4();

        for section in SECTIONS {
            let mut section_div = div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(t.text_muted)
                        .mb_1()
                        .child(section.title),
                );

            for entry in section.entries {
                section_div = section_div.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_3()
                        .py(px(5.))
                        .rounded(px(3.))
                        .bg(t.bg_elevated)
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.text_secondary)
                                .child(entry.description),
                        )
                        .child(
                            div()
                                .px_2()
                                .py(px(2.))
                                .rounded(px(3.))
                                .bg(t.bg_surface)
                                .text_xs()
                                .text_color(t.text_muted)
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .child(entry.keys),
                        ),
                );
            }

            content = content.child(section_div);
        }

        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(header)
            .child(content)
            .into_any_element()
    }
}
