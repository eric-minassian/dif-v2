use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    actions, AnyElement, Context, FocusHandle, MouseButton, MouseUpEvent, Window, div,
    prelude::*, px, uniform_list,
};

use crate::components::{button, empty_state, panel, section_header, PanelSide};

use crate::git;
use crate::picker;
use crate::state::{
    AppConfig, AppState, DiffData, GitChange, ProjectRuntime, SavedProject, SavedSession,
    SessionRuntime, SplitLine, SplitLineKind, TerminalTab, LEFT_SIDEBAR_WIDTH,
    RIGHT_SIDEBAR_WIDTH,
};
use crate::storage;
use crate::terminal;
use crate::theme::theme;

actions!(
    workspace,
    [
        NewSideTab,
        SelectSideTab1,
        SelectSideTab2,
        SelectSideTab3,
        SelectSideTab4,
        SelectSideTab5,
        SelectSideTab6,
        SelectSideTab7,
        SelectSideTab8,
        SelectSideTab9,
        CloseDiffView,
        ToggleLeftSidebar,
        ToggleRightSidebar,
        RefreshGitStatus,
    ]
);

pub struct WorkspaceView {
    state: AppState,
    focus_handle: FocusHandle,
}

impl WorkspaceView {
    pub fn new(config: AppConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut state = AppState {
            config,
            ..AppState::default()
        };
        refresh_project_validity(&mut state.config.projects);

        state.selected_repo = pick_initial_selection(&state.config);
        state.selected_session = state
            .selected_repo
            .as_ref()
            .and_then(|repo| pick_initial_session(&state.config, repo));

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        let mut this = Self { state, focus_handle };
        if let Some(repo) = this.state.selected_repo.clone() {
            if let Some(session_id) = this.state.selected_session.clone() {
                this.activate_session(repo, session_id, window, cx);
            }
        }

        this
    }

    fn on_add_project(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = picker::choose_folder() else {
            return;
        };

        self.add_project_from_path(path, window, cx);
    }

    fn on_remove_project(
        &mut self,
        repo_root: PathBuf,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let removed_selected = self
            .state
            .selected_repo
            .as_ref()
            .is_some_and(|selected| selected == &repo_root);

        self.state
            .config
            .projects
            .retain(|project| project.repo_root != repo_root);
        self.state.runtimes.remove(&repo_root);

        if removed_selected {
            self.state.selected_repo = pick_initial_selection(&self.state.config);
            self.state.selected_session = self
                .state
                .selected_repo
                .as_ref()
                .and_then(|repo| pick_initial_session(&self.state.config, repo));
            self.state.git_poll_generation = self.state.git_poll_generation.wrapping_add(1);
        }

        self.persist_config();
        cx.notify();
    }

    fn on_select_project(
        &mut self,
        repo_root: PathBuf,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_id = pick_initial_session(&self.state.config, &repo_root);
        if let Some(session_id) = session_id {
            self.activate_session(repo_root, session_id, window, cx);
        }
    }

    fn on_select_session(
        &mut self,
        repo_root: PathBuf,
        session_id: String,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_session(repo_root, session_id, window, cx);
    }

    fn on_add_session(
        &mut self,
        repo_root: PathBuf,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        else {
            return;
        };

        let new_id = project.next_session_id();
        let new_name = project.next_session_name();
        project.sessions.push(SavedSession {
            id: new_id.clone(),
            name: new_name,
        });

        self.activate_session(repo_root, new_id, window, cx);
    }

    fn on_delete_session(
        &mut self,
        repo_root: PathBuf,
        session_id: String,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(runtime) = self.state.runtimes.get_mut(&repo_root) {
            runtime.session_runtimes.remove(&session_id);
        }

        if let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        {
            project.sessions.retain(|s| s.id != session_id);
        }

        let was_selected = self
            .state
            .selected_repo
            .as_ref()
            .is_some_and(|r| r == &repo_root)
            && self
                .state
                .selected_session
                .as_ref()
                .is_some_and(|s| s == &session_id);

        if was_selected {
            let new_session = pick_initial_session(&self.state.config, &repo_root);
            if let Some(new_session) = new_session {
                self.activate_session(repo_root, new_session, window, cx);
                return;
            } else {
                self.state.selected_session = None;
            }
        }

        self.persist_config();
        cx.notify();
    }

    fn on_select_side_tab(&mut self, tab_id: String, cx: &mut Context<Self>) {
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

    fn on_add_side_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };
        let Some(session_id) = self.state.selected_session.clone() else {
            return;
        };

        let Some(project_runtime) = self.state.runtimes.get_mut(&repo) else {
            return;
        };
        let Some(session_runtime) = project_runtime.session_runtimes.get_mut(&session_id) else {
            return;
        };

        match terminal::spawn_terminal(window, cx, &repo) {
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

    fn on_delete_side_tab(&mut self, tab_id: String, cx: &mut Context<Self>) {
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

    fn on_open_diff(
        &mut self,
        file_path: String,
        status_code: String,
        _event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(repo) = self.state.selected_repo.clone() else {
            return;
        };

        let view = cx.entity().clone();
        let file_path_clone = file_path.clone();
        let status_code_clone = status_code.clone();

        window
            .spawn(cx, async move |cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        git::compute_file_diff(&repo, &file_path_clone, &status_code_clone)
                    })
                    .await;

                cx.update(|_, cx| {
                    view.update(cx, |this, cx| {
                        match result {
                            Ok(diff_data) => {
                                this.state.viewing_diff = Some(diff_data);
                            }
                            Err(error) => {
                                this.state.flash_error =
                                    Some(format!("Failed to load diff: {error}"));
                            }
                        }
                        cx.notify();
                    })
                })
                .ok();
            })
            .detach();
    }

    fn on_close_diff(&mut self, cx: &mut Context<Self>) {
        self.state.viewing_diff = None;
        cx.notify();
    }

    fn on_toggle_left_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.left_sidebar_collapsed = !self.state.left_sidebar_collapsed;
        cx.notify();
    }

    fn on_toggle_right_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.right_sidebar_collapsed = !self.state.right_sidebar_collapsed;
        cx.notify();
    }

    fn on_refresh_git_status(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(repo) = self.state.selected_repo.clone() {
            self.start_git_poll(repo, window, cx);
        }
    }


    fn select_side_tab_by_index(&mut self, index: usize, cx: &mut Context<Self>) {
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

    fn add_project_from_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match git::normalize_repo_path(&path) {
            Ok(repo_root) => {
                if let Some(existing) = self
                    .state
                    .config
                    .projects
                    .iter()
                    .find(|project| project.repo_root == repo_root)
                    .map(|project| project.repo_root.clone())
                {
                    let session_id = pick_initial_session(&self.state.config, &existing)
                        .unwrap_or_else(|| "1".to_string());
                    self.activate_session(existing, session_id, window, cx);
                    return;
                }

                let project = SavedProject::from_repo_root(repo_root.clone());
                let session_id = project.sessions[0].id.clone();
                self.state.config.projects.push(project);
                self.activate_session(repo_root, session_id, window, cx);
            }
            Err(error) => {
                self.state.flash_error = Some(error);
                cx.notify();
            }
        }
    }

    fn activate_session(
        &mut self,
        repo_root: PathBuf,
        session_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.state.selected_repo = Some(repo_root.clone());
        self.state.selected_session = Some(session_id.clone());
        self.ensure_session_runtime(&repo_root, &session_id, window, cx);
        self.state.config.last_selected_repo = Some(repo_root.clone());

        if let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        {
            project.last_selected_session = Some(session_id);
        }

        self.persist_config();
        self.start_git_poll(repo_root, window, cx);
        cx.notify();
    }

    fn ensure_session_runtime(
        &mut self,
        repo_root: &Path,
        session_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(runtime) = self.state.runtimes.get(repo_root) {
            if runtime.session_runtimes.contains_key(session_id) {
                return;
            }
        }

        // Create main terminal
        let (main_terminal, main_error) = match terminal::spawn_terminal(window, cx, repo_root) {
            Ok(view) => (Some(view), None),
            Err(error) => (None, Some(error.to_string())),
        };

        // Create initial side terminal tab
        let (side_tabs, selected_tab, next_id) = match terminal::spawn_terminal(window, cx, repo_root)
        {
            Ok(view) => {
                let tab = TerminalTab {
                    id: "1".to_string(),
                    name: "Terminal 1".to_string(),
                    view,
                };
                (vec![tab], Some("1".to_string()), 2)
            }
            Err(error) => {
                self.state.flash_error =
                    Some(format!("Failed to create side terminal: {error}"));
                (vec![], None, 1)
            }
        };

        let session_runtime = SessionRuntime {
            main_terminal,
            main_terminal_error: main_error,
            side_tabs,
            selected_side_tab: selected_tab,
            next_tab_id: next_id,
        };

        let runtime = self
            .state
            .runtimes
            .entry(repo_root.to_path_buf())
            .or_default();
        runtime
            .session_runtimes
            .insert(session_id.to_string(), session_runtime);
    }

    fn start_git_poll(&mut self, repo_root: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        self.state.git_poll_generation = self.state.git_poll_generation.wrapping_add(1);
        let generation = self.state.git_poll_generation;
        let view = cx.entity().clone();

        window
            .spawn(cx, async move |cx| {
                loop {
                    let repo = repo_root.clone();
                    let snapshot = cx
                        .background_executor()
                        .spawn(async move { git::collect_changes(&repo) })
                        .await;

                    let keep_running = cx
                        .update(|_, cx| {
                            view.update(cx, |this, cx| {
                                if this.state.git_poll_generation != generation
                                    || this.state.selected_repo.as_ref() != Some(&repo_root)
                                {
                                    return false;
                                }

                                let runtime = this
                                    .state
                                    .runtimes
                                    .entry(repo_root.clone())
                                    .or_insert_with(ProjectRuntime::default);

                                if apply_git_snapshot(runtime, &snapshot) {
                                    cx.notify();
                                }

                                true
                            })
                        })
                        .unwrap_or(false);

                    if !keep_running {
                        break;
                    }

                    cx.background_executor().timer(Duration::from_secs(2)).await;
                }
            })
            .detach();
    }

    fn persist_config(&mut self) {
        if let Err(error) = storage::save_config(&self.state.config) {
            self.state.flash_error = Some(error.to_string());
        }
    }

    fn selected_project_runtime(&self) -> Option<&ProjectRuntime> {
        let repo = self.state.selected_repo.as_ref()?;
        self.state.runtimes.get(repo)
    }

    fn selected_session_runtime(&self) -> Option<&SessionRuntime> {
        let repo = self.state.selected_repo.as_ref()?;
        let session_id = self.state.selected_session.as_ref()?;
        let project_runtime = self.state.runtimes.get(repo)?;
        project_runtime.session_runtimes.get(session_id)
    }

    fn flash_banner(&self) -> Option<AnyElement> {
        let t = theme();
        self.state.flash_error.as_ref().map(|message| {
            div()
                .w_full()
                .px_3()
                .py_2()
                .bg(t.error_bg)
                .text_color(t.text_primary)
                .child(message.clone())
                .into_any_element()
        })
    }

    fn on_toggle_project_collapse(
        &mut self,
        repo_root: PathBuf,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.collapsed_projects.contains(&repo_root) {
            self.state.collapsed_projects.remove(&repo_root);
        } else {
            self.state.collapsed_projects.insert(repo_root);
        }
        cx.notify();
    }

    fn render_left_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let selected_repo = self.state.selected_repo.as_ref();
        let selected_session = self.state.selected_session.as_ref();

        panel(PanelSide::Left)
            .w(px(LEFT_SIDEBAR_WIDTH))
            .child(
                section_header("Projects")
                    .py_3()
                    .child(
                        button()
                            .text_xs()
                            .child("+ Add")
                            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_add_project)),
                    ),
            )
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

                        let chevron = if is_collapsed { "▸" } else { "▾" };

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
                                        .text_xs()
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
                                                    .text_xs()
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
                                                    .child("+"),
                                            )
                                        })
                                        .child(
                                            div()
                                                .id("remove-project-btn")
                                                .cursor_pointer()
                                                .px_1()
                                                .text_xs()
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
                                                .child("×"),
                                        ),
                                ),
                        );

                        // Session rows (only when expanded and valid)
                        if project.last_known_valid && !is_collapsed {
                            for session in &project.sessions {
                                let session_repo = project.repo_root.clone();
                                let session_id = session.id.clone();
                                let delete_repo = project.repo_root.clone();
                                let delete_session_id = session.id.clone();
                                let is_session_selected = is_project_selected
                                    && selected_session.is_some_and(|s| s == &session.id);

                                let session_row_id = gpui::ElementId::Name(
                                    format!("sess-{}-{}", project.display_name, session.id).into(),
                                );

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
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .cursor_pointer()
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(
                                                        move |this, event, window, cx| {
                                                            this.on_select_session(
                                                                session_repo.clone(),
                                                                session_id.clone(),
                                                                event,
                                                                window,
                                                                cx,
                                                            )
                                                        },
                                                    ),
                                                )
                                                .child(
                                                    div()
                                                        .text_sm()
                                                        .text_color(if is_session_selected {
                                                            t.accent
                                                        } else {
                                                            t.text_secondary
                                                        })
                                                        .child(session.name.clone()),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .id("delete-session-btn")
                                                .cursor_pointer()
                                                .px_1()
                                                .text_xs()
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
                                                .child("×"),
                                        ),
                                );
                            }
                        }

                        container
                    })),
            )
            .into_any_element()
    }

    fn render_titlebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let left_collapsed = self.state.left_sidebar_collapsed;
        let right_collapsed = self.state.right_sidebar_collapsed;

        div()
            .h(px(36.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .bg(t.bg_titlebar)
            .border_b_1()
            .border_color(t.border_subtle)
            // Left side: traffic light padding + sidebar toggle
            .child(
                div()
                    .flex()
                    .items_center()
                    .pl(px(78.))
                    .child(
                        div()
                            .id("toggle-left-sidebar")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(if left_collapsed {
                                t.text_dim
                            } else {
                                t.text_muted
                            })
                            .hover(|style| {
                                style.text_color(t.text_primary).cursor_pointer()
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.on_toggle_left_sidebar(cx);
                                }),
                            )
                            .child("⊞"),
                    ),
            )
            // Right side: sidebar toggle
            .child(
                div()
                    .flex()
                    .items_center()
                    .pr_2()
                    .child(
                        div()
                            .id("toggle-right-sidebar")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .text_color(if right_collapsed {
                                t.text_dim
                            } else {
                                t.text_muted
                            })
                            .hover(|style| {
                                style.text_color(t.text_primary).cursor_pointer()
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _window, cx| {
                                    this.on_toggle_right_sidebar(cx);
                                }),
                            )
                            .child("⊟"),
                    ),
            )
            .into_any_element()
    }

    fn render_collapsed_left_sidebar(&self) -> AnyElement {
        div().into_any_element()
    }

    fn render_center(&self, cx: &mut Context<Self>) -> AnyElement {
        if let Some(diff_data) = &self.state.viewing_diff {
            return self.render_diff_view(diff_data, cx);
        }

        if self.state.selected_repo.is_none() {
            return empty_state("Add a Git repository from the left sidebar.").into_any_element();
        }

        if self.state.selected_session.is_none() {
            return empty_state("No sessions. Create one from the sidebar.").into_any_element();
        }

        let Some(session_runtime) = self.selected_session_runtime() else {
            return empty_state("Loading session...").into_any_element();
        };

        if let Some(error) = &session_runtime.main_terminal_error {
            return empty_state(&format!("Terminal failed to start: {error}")).into_any_element();
        }

        if let Some(terminal) = &session_runtime.main_terminal {
            return div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .bg(gpui::black())
                .child(terminal.clone())
                .into_any_element();
        }

        empty_state("Starting terminal...").into_any_element()
    }

    fn render_right_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let top = self.render_changes_panel(cx);
        let bottom = self.render_side_terminal(cx);

        panel(PanelSide::Right)
            .w(px(RIGHT_SIDEBAR_WIDTH))
            .child(top)
            .child(bottom)
            .into_any_element()
    }

    fn render_collapsed_right_sidebar(&self) -> AnyElement {
        div().into_any_element()
    }

    fn render_changes_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let snapshot = self
            .selected_project_runtime()
            .map(|runtime| &runtime.git_snapshot);
        let changes = snapshot
            .map(|snapshot| snapshot.changes.as_slice())
            .unwrap_or(&[]);
        let error = snapshot.and_then(|snapshot| snapshot.last_error.as_ref());
        let count = changes.len();

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(t.border_default)
            .child(
                section_header("Changes").when(count > 0, |header| {
                    header.child(
                        div()
                            .text_xs()
                            .text_color(t.text_dim)
                            .child(format!("{count}")),
                    )
                }),
            )
            .when_some(error, |p, message| {
                p.child(
                    div()
                        .mx_3()
                        .mt_1()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .bg(t.error_bg)
                        .text_color(t.text_primary)
                        .child(message.clone()),
                )
            })
            .child(
                div()
                    .id("changes-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_scroll()
                    .children(if changes.is_empty() {
                        vec![div()
                            .px_3()
                            .py_2()
                            .text_xs()
                            .text_color(t.text_dim)
                            .child("Working tree clean".to_string())
                            .into_any_element()]
                    } else {
                        changes
                            .iter()
                            .map(|change| self.render_change_row(change, cx))
                            .collect::<Vec<_>>()
                    }),
            )
            .into_any_element()
    }

    fn render_change_row(&self, change: &GitChange, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let file_path = change.path.clone();
        let status_code = change.status_code.clone();

        let is_viewing = self
            .state
            .viewing_diff
            .as_ref()
            .is_some_and(|d| d.file_path == change.path);

        let status_color = match change.status_code.as_str() {
            "A" | "??" => t.accent_green,
            "D" => t.accent_red,
            _ => t.text_muted,
        };

        let change_row_id =
            gpui::ElementId::Name(format!("change-{}", change.path).into());

        div()
            .id(change_row_id)
            .group("change-row")
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .py_1()
            .bg(if is_viewing {
                t.selection_medium
            } else {
                t.transparent
            })
            .cursor_pointer()
            .hover(|style| style.bg(t.hover_overlay))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |this, event, window, cx| {
                    this.on_open_diff(file_path.clone(), status_code.clone(), event, window, cx);
                }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(status_color)
                    .w(px(20.))
                    .flex_shrink_0()
                    .child(change.status_code.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_sm()
                    .text_color(if is_viewing {
                        t.text_primary
                    } else {
                        t.text_secondary
                    })
                    .child(change.path.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .invisible()
                    .group_hover("change-row", |style| style.visible())
                    .when_some(change.additions, |el, adds| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(t.accent_green)
                                .child(format!("+{adds}")),
                        )
                    })
                    .when_some(change.deletions.filter(|&d| d > 0), |el, dels| {
                        el.child(
                            div()
                                .text_xs()
                                .text_color(t.accent_red)
                                .child(format!("-{dels}")),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_side_terminal(&self, cx: &mut Context<Self>) -> AnyElement {
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
                            .text_xs()
                            .text_color(t.text_line_number)
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(move |this, _event, _window, cx| {
                                    this.on_delete_side_tab(delete_tab_id.clone(), cx);
                                }),
                            )
                            .child("x"),
                    )
            }))
            .child(
                div()
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(t.text_dim)
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _event, window, cx| {
                            this.on_add_side_tab(window, cx);
                        }),
                    )
                    .child("+"),
            )
            .into_any_element()
    }

    fn render_diff_view(&self, diff_data: &DiffData, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .py_1()
            .bg(t.bg_panel)
            .border_b_1()
            .border_color(t.border_default)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_sm()
                    .child(
                        div()
                            .text_color(t.text_secondary)
                            .child(diff_data.file_path.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.accent_green)
                            .child(format!("+{}", diff_data.additions)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.accent_red)
                            .child(format!("-{}", diff_data.deletions)),
                    ),
            )
            .child(
                div()
                    .id("close-diff")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(t.text_dim)
                    .hover(|style| style.text_color(t.text_primary))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.on_close_diff(cx);
                        }),
                    )
                    .child("✕ Esc"),
            );

        let lines = Rc::new(diff_data.lines.clone());
        let line_count = lines.len();

        let diff_list = uniform_list("diff-lines", line_count, move |range, _window, _cx| {
            range
                .map(|ix| render_split_line(&lines[ix]))
                .collect::<Vec<_>>()
        })
        .flex_1()
        .min_h_0()
        .bg(t.bg_base);

        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .child(header)
            .child(diff_list)
            .into_any_element()
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();

        let left = if self.state.left_sidebar_collapsed {
            self.render_collapsed_left_sidebar()
        } else {
            self.render_left_sidebar(cx)
        };

        let right = if self.state.right_sidebar_collapsed {
            self.render_collapsed_right_sidebar()
        } else {
            self.render_right_sidebar(cx)
        };

        div()
            .id("workspace")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &NewSideTab, window, cx| {
                this.on_add_side_tab(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseDiffView, _window, cx| {
                this.on_close_diff(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleLeftSidebar, _window, cx| {
                this.on_toggle_left_sidebar(cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleRightSidebar, _window, cx| {
                this.on_toggle_right_sidebar(cx);
            }))
            .on_action(cx.listener(|this, _: &RefreshGitStatus, window, cx| {
                this.on_refresh_git_status(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectSideTab1, _w, cx| {
                this.select_side_tab_by_index(0, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectSideTab2, _w, cx| {
                this.select_side_tab_by_index(1, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectSideTab3, _w, cx| {
                this.select_side_tab_by_index(2, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectSideTab4, _w, cx| {
                this.select_side_tab_by_index(3, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectSideTab5, _w, cx| {
                this.select_side_tab_by_index(4, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectSideTab6, _w, cx| {
                this.select_side_tab_by_index(5, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectSideTab7, _w, cx| {
                this.select_side_tab_by_index(6, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectSideTab8, _w, cx| {
                this.select_side_tab_by_index(7, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectSideTab9, _w, cx| {
                this.select_side_tab_by_index(8, cx);
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(t.bg_base)
            .text_color(t.text_primary)
            .child(self.render_titlebar(cx))
            .children(self.flash_banner())
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(left)
                    .child(self.render_center(cx))
                    .child(right),
            )
    }
}

fn refresh_project_validity(projects: &mut [SavedProject]) {
    for project in projects {
        project.last_known_valid = git::is_valid_repo(&project.repo_root);
    }
}

/// Returns `true` if the snapshot actually changed (so callers know whether to re-render).
fn apply_git_snapshot(
    runtime: &mut ProjectRuntime,
    snapshot: &Result<Vec<GitChange>, String>,
) -> bool {
    runtime.git_snapshot.last_refresh = Some(Instant::now());

    match snapshot {
        Ok(changes) => {
            let changed = runtime.git_snapshot.changes != *changes
                || runtime.git_snapshot.last_error.is_some();
            runtime.git_snapshot.changes = changes.clone();
            runtime.git_snapshot.last_error = None;
            changed
        }
        Err(error) => {
            let changed = runtime.git_snapshot.last_error.as_ref() != Some(error);
            runtime.git_snapshot.last_error = Some(error.clone());
            changed
        }
    }
}

fn render_split_line(line: &SplitLine) -> AnyElement {
    let t = theme();

    let (left_bg, right_bg) = match line.kind {
        SplitLineKind::Equal => (t.transparent, t.transparent),
        SplitLineKind::Insert => (t.transparent, t.diff_add_bg),
        SplitLineKind::Delete => (t.diff_del_bg, t.transparent),
        SplitLineKind::Replace => (t.diff_del_bg, t.diff_add_bg),
    };

    let left_text_color = match line.kind {
        SplitLineKind::Delete | SplitLineKind::Replace => t.diff_del_text,
        _ => t.text_secondary,
    };

    let right_text_color = match line.kind {
        SplitLineKind::Insert | SplitLineKind::Replace => t.diff_add_text,
        _ => t.text_secondary,
    };

    div()
        .flex()
        .w_full()
        .text_xs()
        .whitespace_nowrap()
        .child(
            // Left half (old)
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .overflow_hidden()
                .bg(left_bg)
                .child(
                    div()
                        .w(px(40.))
                        .flex_shrink_0()
                        .text_right()
                        .px_1()
                        .text_color(t.text_line_number)
                        .child(
                            line.old_lineno
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .px_1()
                        .text_color(left_text_color)
                        .child(line.old_text.clone()),
                ),
        )
        .child(
            // Divider
            div().w(px(1.)).flex_shrink_0().bg(t.border_default),
        )
        .child(
            // Right half (new)
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .overflow_hidden()
                .bg(right_bg)
                .child(
                    div()
                        .w(px(40.))
                        .flex_shrink_0()
                        .text_right()
                        .px_1()
                        .text_color(t.text_line_number)
                        .child(
                            line.new_lineno
                                .map(|n| n.to_string())
                                .unwrap_or_default(),
                        ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .px_1()
                        .text_color(right_text_color)
                        .child(line.new_text.clone()),
                ),
        )
        .into_any_element()
}

fn pick_initial_selection(config: &AppConfig) -> Option<PathBuf> {
    if let Some(last) = &config.last_selected_repo
        && config
            .projects
            .iter()
            .any(|project| project.repo_root == *last && project.last_known_valid)
    {
        return Some(last.clone());
    }

    config
        .projects
        .iter()
        .find(|project| project.last_known_valid)
        .map(|project| project.repo_root.clone())
}

fn pick_initial_session(config: &AppConfig, repo: &Path) -> Option<String> {
    let project = config.projects.iter().find(|p| p.repo_root == repo)?;
    if let Some(last) = &project.last_selected_session {
        if project.sessions.iter().any(|s| s.id == *last) {
            return Some(last.clone());
        }
    }
    project.sessions.first().map(|s| s.id.clone())
}
