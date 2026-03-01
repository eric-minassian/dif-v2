use gpui::{div, prelude::*, AnyElement, Context, KeyDownEvent, MouseButton, Window};

use crate::theme::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn render_commit_input(
        &self,
        message: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let display_text = if message.is_empty() {
            "Enter commit message...".to_string()
        } else {
            message.to_string()
        };
        let is_placeholder = message.is_empty();
        let is_focused = self.commit_input_focus.is_focused(window);

        div()
            .mx_3()
            .mt_2()
            .mb_1()
            .border_t_1()
            .border_color(t.border_subtle)
            .pt_2()
            .child(
                div()
                    .id("commit-input")
                    .w_full()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .rounded_md()
                    .border_1()
                    .border_color(if is_focused {
                        t.accent_blue
                    } else {
                        t.border_subtle
                    })
                    .bg(if is_focused {
                        gpui::rgba(0xffffff10)
                    } else {
                        gpui::rgba(0xffffff08)
                    })
                    .text_color(if is_placeholder {
                        t.text_dim
                    } else {
                        t.text_primary
                    })
                    .cursor_text()
                    .track_focus(&self.commit_input_focus)
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _event, window, cx| {
                            this.commit_input_focus.focus(window, cx);
                            cx.notify();
                        }),
                    )
                    .on_key_down(cx.listener(Self::on_commit_input_key_down))
                    .child(display_text),
            )
            .into_any_element()
    }

    pub(crate) fn on_commit_input_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.state.selected_repo.as_ref().cloned() else {
            return;
        };
        let Some(session_id) = self.state.selected_session.as_ref().cloned() else {
            return;
        };
        let Some(runtime) = self.state.runtimes.get_mut(&repo) else {
            return;
        };
        let Some(session_runtime) = runtime.session_runtimes.get_mut(&session_id) else {
            return;
        };

        // Use key_char for composed characters, fallback to key
        let key_char = event.keystroke.key_char.as_deref();
        let key = &event.keystroke.key;

        let has_platform = event.keystroke.modifiers.platform;
        let has_ctrl = event.keystroke.modifiers.control;

        match key.as_str() {
            "backspace" if !has_platform && !has_ctrl => {
                session_runtime.commit_message.pop();
            }
            "backspace" if has_platform || has_ctrl => {
                session_runtime.commit_message.clear();
            }
            "escape" => {
                self.focus_handle.focus(_window, cx);
            }
            _ if has_platform || has_ctrl => {
                return; // let shortcuts propagate
            }
            _ => {
                // Insert the typed character if available
                if let Some(ch) = key_char {
                    if !ch.is_empty() && ch.chars().all(|c: char| !c.is_control()) {
                        session_runtime.commit_message.push_str(ch);
                    } else {
                        return;
                    }
                } else {
                    return;
                }
            }
        }

        cx.notify();
    }
}
