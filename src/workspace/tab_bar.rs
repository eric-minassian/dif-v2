use gpui::{AnyElement, Context, MouseButton, Window, div, prelude::*};

use crate::components::empty_state;
use crate::icons::{icon_plus, icon_x};
use crate::state::{SessionRuntime, TerminalTab};
use crate::terminal;
use crate::theme::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn on_select_side_tab(&mut self, tab_id: String, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.as_ref() else {
            return;
        };
        let Some(session_id) = self.state.selected_session.as_ref() else {
            return;
        };
        let Some(project_runtime) = self.state.runtimes.get_mut(repo) else {
            return;
        };
        let Some(session_runtime) = project_runtime.session_runtimes.get_mut(session_id) else {
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

        let Some(project_runtime) = self.state.runtimes.get_mut(&repo) else {
            return;
        };
        let Some(session_runtime) = project_runtime.session_runtimes.get_mut(&session_id) else {
            return;
        };

        match terminal::spawn_terminal(window, cx, &working_dir) {
            Ok(view) => {
                let id = session_runtime.next_tab_id.to_string();
                session_runtime.next_tab_id += 1;
                let name = format!("Terminal {id}");
                session_runtime.side_tabs.push(TerminalTab {
                    id: id.clone(),
                    name,
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

    pub(crate) fn on_delete_side_tab(&mut self, tab_id: String, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.as_ref() else {
            return;
        };
        let Some(session_id) = self.state.selected_session.as_ref() else {
            return;
        };
        let Some(project_runtime) = self.state.runtimes.get_mut(repo) else {
            return;
        };
        let Some(session_runtime) = project_runtime.session_runtimes.get_mut(session_id) else {
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

    pub(crate) fn select_side_tab_by_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.as_ref() else {
            return;
        };
        let Some(session_id) = self.state.selected_session.as_ref() else {
            return;
        };
        let Some(project_runtime) = self.state.runtimes.get_mut(repo) else {
            return;
        };
        let Some(session_runtime) = project_runtime.session_runtimes.get_mut(session_id) else {
            return;
        };

        if let Some(tab) = session_runtime.side_tabs.get(index) {
            session_runtime.selected_side_tab = Some(tab.id.clone());
            cx.notify();
        }
    }

    pub(crate) fn render_side_terminal(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(session_runtime) = self.selected_session_runtime() else {
            return empty_state("Select a session to start terminals.").into_any_element();
        };

        let tab_bar = self.render_tab_bar(session_runtime, cx);

        let terminal_content =
            if let Some(selected_id) = &session_runtime.selected_side_tab {
                if let Some(tab) =
                    session_runtime.side_tabs.iter().find(|t| t.id == *selected_id)
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

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
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

        div()
            .flex()
            .items_center()
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

                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(if is_selected {
                        t.bg_elevated
                    } else {
                        t.transparent
                    })
                    .child(
                        div()
                            .cursor_pointer()
                            .text_xs()
                            .text_color(if is_selected {
                                t.text_primary
                            } else {
                                t.text_muted
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _event, _window, cx| {
                                    this.on_select_side_tab(select_tab_id.clone(), cx);
                                }),
                            )
                            .child(tab.name.clone()),
                    )
                    .child(
                        div()
                            .cursor_pointer()
                            .text_color(t.text_line_number)
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _event, _window, cx| {
                                    this.on_delete_side_tab(delete_tab_id.clone(), cx);
                                }),
                            )
                            .child(icon_x().size_3().text_color(t.text_line_number)),
                    )
            }))
            .child(
                div()
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .text_color(t.text_dim)
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _event, window, cx| {
                            this.on_add_side_tab(window, cx);
                        }),
                    )
                    .child(icon_plus().size_3p5().text_color(t.text_dim)),
            )
            .into_any_element()
    }
}
