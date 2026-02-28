use gpui::{AnyElement, ClickEvent, Context, MouseButton, div, prelude::*, px};

use crate::components::{button, panel, section_header, PanelSide};
use crate::icons::{icon_chevron_down, icon_chevron_right, icon_plus, icon_settings, icon_x};
use crate::theme::theme;

use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn render_left_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let selected_repo = self.state.selected_repo.as_ref();
        let selected_session = self.state.selected_session.as_ref();

        panel(PanelSide::Left)
            .w(px(self.state.left_sidebar_width))
            .child(section_header("Projects").py_3())
            .child(
                div()
                    .id("project-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .children(self.state.config.projects.iter().map(|project| {
                        let is_project_selected =
                            selected_repo.is_some_and(|v| v == &project.repo_root);
                        let is_collapsed = self
                            .state
                            .collapsed_projects
                            .contains(&project.repo_root);
                        let select_repo = project.repo_root.clone();
                        let toggle_repo = project.repo_root.clone();
                        let remove_repo = project.repo_root.clone();
                        let add_session_repo = project.repo_root.clone();

                        let chevron = if is_collapsed {
                            icon_chevron_right().size_3().text_color(t.text_dim).into_any_element()
                        } else {
                            icon_chevron_down().size_3().text_color(t.text_dim).into_any_element()
                        };

                        let project_row_id =
                            gpui::ElementId::Name(format!("proj-{}", project.display_name).into());

                        let mut container =
                            div().flex().flex_col().border_b_1().border_color(t.border_subtle);

                        // Project header row
                        container = container.child(
                            div()
                                .id(project_row_id)
                                .group("project-row")
                                .flex()
                                .items_center()
                                .gap_1()
                                .px_3()
                                .py_2()
                                .bg(if is_project_selected {
                                    t.selection_faint
                                } else {
                                    t.transparent
                                })
                                .hover(|style| style.bg(t.hover_overlay))
                                // Chevron
                                .child(
                                    div()
                                        .text_color(t.text_dim)
                                        .w(px(12.))
                                        .flex_shrink_0()
                                        .cursor_pointer()
                                        .on_mouse_up(
                                            MouseButton::Left,
                                            cx.listener(move |this, event, window, cx| {
                                                this.on_toggle_project_collapse(
                                                    toggle_repo.clone(),
                                                    event,
                                                    window,
                                                    cx,
                                                )
                                            }),
                                        )
                                        .child(chevron),
                                )
                                // Project name + path
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .when(project.last_known_valid, |row| {
                                            row.cursor_pointer().on_mouse_up(
                                                MouseButton::Left,
                                                cx.listener(move |this, event, window, cx| {
                                                    this.on_select_project(
                                                        select_repo.clone(),
                                                        event,
                                                        window,
                                                        cx,
                                                    )
                                                }),
                                            )
                                        })
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(if project.last_known_valid {
                                                    t.text_primary
                                                } else {
                                                    t.text_muted
                                                })
                                                .child(project.display_name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(t.text_dim)
                                                .overflow_hidden()
                                                .child(if project.last_known_valid {
                                                    project.repo_root.display().to_string()
                                                } else {
                                                    format!(
                                                        "Missing: {}",
                                                        project.repo_root.display()
                                                    )
                                                }),
                                        ),
                                )
                                // Actions: + session, x remove (hover-only)
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .flex_shrink_0()
                                        .invisible()
                                        .group_hover("project-row", |style| style.visible())
                                        .when(project.last_known_valid, |el| {
                                            el.child(
                                                div()
                                                    .id("add-session-btn")
                                                    .cursor_pointer()
                                                    .px_1()
                                                    .text_color(t.text_dim)
                                                    .hover(|style| {
                                                        style.text_color(t.text_primary)
                                                    })
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |this, event, window, cx| {
                                                                this.on_add_session(
                                                                    add_session_repo.clone(),
                                                                    event,
                                                                    window,
                                                                    cx,
                                                                )
                                                            },
                                                        ),
                                                    )
                                                    .child(icon_plus().size_3p5().text_color(t.text_dim)),
                                            )
                                        })
                                        .child(
                                            div()
                                                .id("remove-project-btn")
                                                .cursor_pointer()
                                                .px_1()
                                                .text_color(t.text_dim)
                                                .hover(|style| style.text_color(t.accent_red))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |this, event, window, cx| {
                                                            this.on_remove_project(
                                                                remove_repo.clone(),
                                                                event,
                                                                window,
                                                                cx,
                                                            )
                                                        },
                                                    ),
                                                )
                                                .child(icon_x().size_3p5().text_color(t.text_dim)),
                                        ),
                                ),
                        );

                        // Session rows (only when expanded and valid)
                        if project.last_known_valid && !is_collapsed {
                            for (session_index, session) in project.sessions.iter().enumerate() {
                                let session_repo = project.repo_root.clone();
                                let session_id = session.id.clone();
                                let delete_repo = project.repo_root.clone();
                                let delete_session_id = session.id.clone();
                                let rename_repo = project.repo_root.clone();
                                let rename_session_id = session.id.clone();
                                let rename_session_name = session.name.clone();
                                let is_session_selected = is_project_selected
                                    && selected_session.is_some_and(|s| s == &session.id);

                                let is_renaming = self
                                    .renaming_session
                                    .as_ref()
                                    .is_some_and(|(r, s, _, _, _)| {
                                        r == &project.repo_root && s == &session.id
                                    });

                                let session_row_id = gpui::ElementId::Name(
                                    format!("sess-{}-{}", project.display_name, session.id).into(),
                                );

                                let name_content: AnyElement = if is_renaming {
                                    let input = self
                                        .renaming_session
                                        .as_ref()
                                        .unwrap()
                                        .2
                                        .clone();
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(input)
                                        .into_any_element()
                                } else {
                                    div()
                                        .id(gpui::ElementId::Name(
                                            format!(
                                                "sess-name-{}-{}",
                                                project.display_name, session.id
                                            )
                                            .into(),
                                        ))
                                        .flex_1()
                                        .min_w_0()
                                        .cursor_pointer()
                                        .on_click(cx.listener(
                                            move |this, event: &ClickEvent, window, cx| {
                                                if event.click_count() == 2 {
                                                    this.on_rename_session_start(
                                                        rename_repo.clone(),
                                                        rename_session_id.clone(),
                                                        rename_session_name.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                } else if event.click_count() == 1 {
                                                    this.activate_session(
                                                        session_repo.clone(),
                                                        session_id.clone(),
                                                        window,
                                                        cx,
                                                    );
                                                }
                                            },
                                        ))
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(if is_session_selected {
                                                    t.accent
                                                } else {
                                                    t.text_secondary
                                                })
                                                .child(session.name.clone()),
                                        )
                                        .into_any_element()
                                };

                                let show_badge = self.state.cmd_held
                                    && is_project_selected
                                    && session_index < 9;

                                container = container.child(
                                    div()
                                        .id(session_row_id)
                                        .group("session-row")
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .pl(px(28.))
                                        .pr_3()
                                        .py_1()
                                        .bg(if is_session_selected {
                                            t.selection_medium
                                        } else {
                                            t.transparent
                                        })
                                        .hover(|style| style.bg(t.hover_overlay))
                                        .when(show_badge, |el| {
                                            el.child(
                                                div()
                                                    .text_xs()
                                                    .text_color(t.text_muted)
                                                    .w(px(14.))
                                                    .flex_shrink_0()
                                                    .child(format!("{}", session_index + 1)),
                                            )
                                        })
                                        .child(name_content)
                                        .child(
                                            div()
                                                .id("delete-session-btn")
                                                .cursor_pointer()
                                                .px_1()
                                                .text_color(t.text_dim)
                                                .invisible()
                                                .group_hover("session-row", |style| {
                                                    style.visible()
                                                })
                                                .hover(|style| style.text_color(t.accent_red))
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |this, event, window, cx| {
                                                            this.on_delete_session(
                                                                delete_repo.clone(),
                                                                delete_session_id.clone(),
                                                                event,
                                                                window,
                                                                cx,
                                                            )
                                                        },
                                                    ),
                                                )
                                                .child(icon_x().size_3p5().text_color(t.text_dim)),
                                        ),
                                );
                            }
                        }

                        container
                    })),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(t.border_default)
                    .child(
                        button()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .child(icon_plus().size_3().text_color(t.text_primary))
                            .child("Add")
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_add_project)),
                    )
                    .child(
                        div()
                            .id("settings-btn")
                            .cursor_pointer()
                            .px_1()
                            .text_color(t.text_dim)
                            .hover(|style| style.text_color(t.text_primary))
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _event, _window, cx| {
                                    this.on_open_settings(cx);
                                }),
                            )
                            .child(icon_settings().size_3p5().text_color(t.text_dim)),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn render_collapsed_left_sidebar(&self) -> AnyElement {
        div().into_any_element()
    }
}
