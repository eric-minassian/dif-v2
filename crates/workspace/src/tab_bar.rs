use gpui::Focusable;

use crate::runtime::{SessionRuntime, TerminalTab};
use ui::empty_state;
use ui::prelude::*;

use crate::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn on_select_side_tab(&mut self, tab_id: String, cx: &mut Context<Self>) {
        let Some(session_runtime) = self.selected_session_runtime_mut() else {
            return;
        };
        session_runtime.selected_side_tab = Some(tab_id);
        cx.notify();
    }

    pub(crate) fn on_add_side_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };
        let Some(session_id) = self.state.selected_session.clone() else {
            return;
        };
        let working_dir = self.worktree_or_repo(&repo, &session_id);

        let Some(session_runtime) = self.selected_session_runtime_mut() else {
            return;
        };

        match terminal::spawn_terminal(window, cx, &working_dir) {
            Ok(view) => {
                let id = session_runtime.next_tab_id.to_string();
                session_runtime.next_tab_id += 1;
                session_runtime.side_tabs.push(TerminalTab {
                    id: id.clone(),
                    view,
                });
                session_runtime.selected_side_tab = Some(id);
            }
            Err(error) => {
                self.state.flash_error = Some(format!("Failed to create terminal: {error}"));
            }
        }
        cx.notify();
    }

    pub(crate) fn on_close_active_side_tab(&mut self, cx: &mut Context<Self>) {
        let Some(tab_id) = self
            .selected_session_runtime()
            .and_then(|rt| rt.selected_side_tab.clone())
        else {
            return;
        };
        self.on_delete_side_tab(tab_id, cx);
    }

    pub(crate) fn on_delete_side_tab(&mut self, tab_id: String, cx: &mut Context<Self>) {
        let Some(session_runtime) = self.selected_session_runtime_mut() else {
            return;
        };

        session_runtime.side_tabs.retain(|t| t.id != tab_id);

        if session_runtime
            .selected_side_tab
            .as_ref()
            .is_some_and(|s| s == &tab_id)
        {
            session_runtime.selected_side_tab =
                session_runtime.side_tabs.first().map(|t| t.id.clone());
        }

        cx.notify();
    }

    /// Toggle focus between the main terminal and the active side terminal.
    pub(crate) fn on_focus_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session_runtime) = self.selected_session_runtime() else {
            return;
        };

        // Collect the focus handles we need before mutably borrowing cx
        let side_handle = session_runtime
            .selected_side_tab
            .as_ref()
            .and_then(|tab_id| session_runtime.side_tabs.iter().find(|t| t.id == *tab_id))
            .map(|tab| tab.view.focus_handle(cx));

        let main_handle = session_runtime
            .main_terminal
            .as_ref()
            .map(|main| main.focus_handle(cx));

        let side_focused = side_handle
            .as_ref()
            .map(|h| h.is_focused(window))
            .unwrap_or(false);

        if side_focused {
            if let Some(handle) = main_handle {
                handle.focus(window, cx);
            }
        } else {
            if let Some(handle) = side_handle {
                handle.focus(window, cx);
            }
        }
    }

    pub(crate) fn render_side_terminal(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(session_runtime) = self.selected_session_runtime() else {
            return empty_state("Select a session to start terminals.").into_any_element();
        };

        let tab_bar = self.render_tab_bar(session_runtime, cx);

        let terminal_content = if let Some(selected_id) = &session_runtime.selected_side_tab {
            if let Some(tab) = session_runtime
                .side_tabs
                .iter()
                .find(|t| t.id == *selected_id)
            {
                div()
                    .flex_1()
                    .min_h_0()
                    .bg(gpui::black())
                    .child(tab.view.clone())
                    .into_any_element()
            } else {
                empty_state("No terminal selected.").into_any_element()
            }
        } else if session_runtime.side_tabs.is_empty() {
            empty_state("Click + to add a terminal.").into_any_element()
        } else {
            empty_state("No terminal selected.").into_any_element()
        };

        v_flex()
            .flex_1()
            .min_h_0()
            .child(tab_bar)
            .child(terminal_content)
            .into_any_element()
    }

    fn render_tab_bar(
        &self,
        session_runtime: &SessionRuntime,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let selected_id = session_runtime.selected_side_tab.as_ref();

        h_flex()
            .gap_1()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(t.border_default)
            .bg(t.bg_surface)
            .children(session_runtime.side_tabs.iter().map(|tab| {
                let is_selected = selected_id.is_some_and(|s| s == &tab.id);
                let select_tab_id = tab.id.clone();
                let delete_tab_id = tab.id.clone();
                let select_btn_id = gpui::ElementId::Name(format!("sel-tab-{}", tab.id).into());

                h_flex()
                    .group("tab-item")
                    .gap_1()
                    .px_2()
                    .py_1()
                    .bg(if is_selected {
                        t.bg_elevated
                    } else {
                        t.transparent
                    })
                    .when(is_selected, |el| {
                        el.border_b_2().border_color(t.accent_blue)
                    })
                    .when(!is_selected, |el| {
                        el.hover(|style| style.bg(t.hover_overlay))
                    })
                    .child(
                        div()
                            .id(select_btn_id)
                            .cursor_pointer()
                            .text_xs()
                            .text_color(if is_selected {
                                t.text_primary
                            } else {
                                t.text_muted
                            })
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.on_select_side_tab(select_tab_id.clone(), cx);
                            }))
                            .child(format!("Terminal {}", tab.id)),
                    )
                    .child(
                        IconButton::new(
                            gpui::ElementId::Name(format!("close-tab-{}", tab.id).into()),
                            IconName::X,
                        )
                        .icon_size(px(12.))
                        .visible_on_hover("tab-item")
                        .on_click(cx.listener(
                            move |this, _event, _window, cx| {
                                this.on_delete_side_tab(delete_tab_id.clone(), cx);
                            },
                        )),
                    )
            }))
            .child(
                div()
                    .id("add-tab-btn")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_color(t.text_dim)
                    .hover(|style| style.bg(t.hover_overlay).text_color(t.text_muted))
                    .on_click(cx.listener(|this, _event, window, cx| {
                        this.on_add_side_tab(window, cx);
                    }))
                    .child(Icon::new(IconName::Plus).size(px(14.)).color(Color::Dim)),
            )
            .into_any_element()
    }
}
