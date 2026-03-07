use gpui::ClickEvent;

use crate::components::{button, panel, section_header, PanelSide};
use crate::prelude::*;
use crate::state::SavedProject;

use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn render_left_sidebar(
        &self,
        show_session_shortcuts: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected_repo = self.state.selected_repo.as_ref();
        let selected_session = self.state.selected_session.as_deref();

        let mut project_list = div()
            .id("project-list")
            .flex_1()
            .min_h_0()
            .overflow_scroll();

        for project in &self.state.config.projects {
            let is_selected = selected_repo.is_some_and(|v| v == &project.repo_root);
            let is_collapsed = self.state.collapsed_projects.contains(&project.repo_root);
            project_list = project_list.child(self.render_project_entry(
                project,
                is_selected,
                is_collapsed,
                selected_session,
                show_session_shortcuts,
                cx,
            ));
        }

        panel(PanelSide::Left)
            .w(px(self.state.left_sidebar_width))
            .child(section_header("Projects").py_3())
            .child(project_list)
            .child(self.render_sidebar_footer(cx))
            .into_any_element()
    }

    fn render_project_entry(
        &self,
        project: &SavedProject,
        is_selected: bool,
        is_collapsed: bool,
        selected_session: Option<&str>,
        show_session_shortcuts: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = theme();
        let select_repo = project.repo_root.clone();
        let toggle_repo = project.repo_root.clone();
        let remove_repo = project.repo_root.clone();
        let add_session_repo = project.repo_root.clone();

        let chevron_icon = if is_collapsed {
            IconName::ChevronRight
        } else {
            IconName::ChevronDown
        };

        let project_row_id =
            gpui::ElementId::Name(format!("proj-{}", project.display_name).into());
        let toggle_id =
            gpui::ElementId::Name(format!("toggle-{}", project.display_name).into());
        let select_id =
            gpui::ElementId::Name(format!("select-{}", project.display_name).into());

        let mut container = v_flex().border_b_1().border_color(t.border_subtle);

        // Project header row
        container = container.child(
            h_flex()
                .id(project_row_id)
                .group("project-row")
                .gap_1()
                .px_3()
                .py_2()
                .bg(if is_selected {
                    t.selection_faint
                } else {
                    t.transparent
                })
                .when(is_selected, |el| {
                    el.border_l_2().border_color(t.accent_blue)
                })
                .hover(|style| style.bg(t.hover_overlay))
                .child(
                    div()
                        .id(toggle_id)
                        .w(px(12.))
                        .flex_shrink_0()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _event, _window, cx| {
                            this.on_toggle_project_collapse(toggle_repo.clone(), cx)
                        }))
                        .child(Icon::new(chevron_icon).size(px(12.)).color(Color::Dim)),
                )
                .child(
                    div()
                        .id(select_id)
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .when(project.last_known_valid, |row| {
                            row.cursor_pointer().on_click(
                                cx.listener(move |this, _event, window, cx| {
                                    this.on_select_project(
                                        select_repo.clone(),
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
                        ),
                )
                .child(
                    h_flex()
                        .gap_1()
                        .flex_shrink_0()
                        .invisible()
                        .group_hover("project-row", |style| style.visible())
                        .when(project.last_known_valid, |el| {
                            el.child(
                                IconButton::new("add-session-btn", IconName::Plus)
                                    .icon_size(px(14.))
                                    .on_click(cx.listener(move |this, _event, window, cx| {
                                        this.on_add_session(
                                            add_session_repo.clone(),
                                            window,
                                            cx,
                                        )
                                    })),
                            )
                        })
                        .child(
                            IconButton::new("remove-project-btn", IconName::X)
                                .icon_size(px(14.))
                                .hover_color(t.accent_red)
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.on_remove_project(remove_repo.clone(), cx)
                                })),
                        ),
                ),
        );

        // Session rows (only when expanded and valid)
        if project.last_known_valid && !is_collapsed {
            for (session_index, session) in project.sessions.iter().enumerate() {
                let is_session_selected =
                    is_selected && selected_session.is_some_and(|s| s == session.id);

                container = container.child(self.render_session_row(
                    project,
                    session_index,
                    is_selected,
                    is_session_selected,
                    show_session_shortcuts,
                    cx,
                ));
            }

            // Inline text input for creating a new session
            if let Some(create) = &self.creating_session {
                if create.edit.repo_root == project.repo_root {
                    container = container
                        .child(Self::render_creating_session_row(&create.edit.input, &create.validation_error));
                }
            }
        }

        container
    }

    fn render_session_row(
        &self,
        project: &SavedProject,
        session_index: usize,
        is_project_selected: bool,
        is_session_selected: bool,
        show_session_shortcuts: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = theme();
        let session = &project.sessions[session_index];
        let session_repo = project.repo_root.clone();
        let session_id = session.id.clone();
        let delete_repo = project.repo_root.clone();
        let delete_session_id = session.id.clone();
        let rename_repo = project.repo_root.clone();
        let rename_session_id = session.id.clone();
        let rename_session_name = session.name.clone();

        let is_renaming = self
            .renaming_session
            .as_ref()
            .is_some_and(|r| r.edit.repo_root == project.repo_root && r.session_id == session.id);

        let session_row_id = gpui::ElementId::Name(
            format!("sess-{}-{}", project.display_name, session.id).into(),
        );

        let name_content: AnyElement = if is_renaming {
            let input = self.renaming_session.as_ref().unwrap().edit.input.clone();
            div()
                .flex_1()
                .min_w_0()
                .child(input)
                .into_any_element()
        } else {
            div()
                .id(gpui::ElementId::Name(
                    format!("sess-name-{}-{}", project.display_name, session.id).into(),
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
                            t.text_primary
                        } else {
                            t.text_secondary
                        })
                        .child(session.name.clone()),
                )
                .into_any_element()
        };

        let show_badge = show_session_shortcuts && is_project_selected && session_index < 9;

        h_flex()
            .id(session_row_id)
            .group("session-row")
            .justify_between()
            .pl(px(28.))
            .pr_3()
            .py(px(6.))
            .bg(if is_session_selected {
                t.selection_medium
            } else {
                t.transparent
            })
            .when(is_session_selected, |el| {
                el.border_l_2().border_color(t.accent_blue)
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
                IconButton::new("delete-session-btn", IconName::X)
                    .icon_size(px(14.))
                    .hover_color(t.accent_red)
                    .visible_on_hover("session-row")
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.on_delete_session(
                            delete_repo.clone(),
                            delete_session_id.clone(),
                            window,
                            cx,
                        )
                    })),
            )
    }

    fn render_creating_session_row(
        input_entity: &gpui::Entity<crate::text_input::TextInput>,
        error: &Option<String>,
    ) -> impl IntoElement {
        let t = theme();
        let has_error = error.is_some();
        let mut row = v_flex()
            .id("creating-session-row")
            .pl(px(28.))
            .pr_3()
            .py(px(6.))
            .bg(t.selection_medium)
            .border_l_2()
            .border_color(if has_error {
                t.accent_red
            } else {
                t.accent_blue
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(input_entity.clone()),
            );
        if let Some(msg) = error {
            row = row.child(
                div()
                    .text_xs()
                    .text_color(t.accent_red)
                    .pt(px(2.))
                    .child(msg.clone()),
            );
        }
        row
    }

    fn render_sidebar_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();

        h_flex()
            .justify_between()
            .px_3()
            .py_2()
            .flex_shrink_0()
            .border_t_1()
            .border_color(t.border_default)
            .bg(t.bg_surface)
            .child(
                button()
                    .id("add-project-btn")
                    .flex()
                    .items_center()
                    .gap_1()
                    .text_xs()
                    .child(Icon::new(IconName::Plus).size(px(12.)).color(Color::Default))
                    .child("Add")
                    .on_click(cx.listener(|this, _event, window, cx| {
                        this.on_add_project(window, cx);
                    })),
            )
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        IconButton::new("help-btn", IconName::HelpCircle)
                            .icon_size(px(14.))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.state.viewing_help = !this.state.viewing_help;
                                cx.notify();
                            })),
                    )
                    .child(
                        IconButton::new("settings-btn", IconName::Settings)
                            .icon_size(px(14.))
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.on_open_settings(cx);
                            })),
                    ),
            )
    }

    pub(crate) fn render_collapsed_left_sidebar(&self) -> AnyElement {
        div().into_any_element()
    }
}
