use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{
    AnyElement, Context, Div, MouseButton, MouseUpEvent, Window, div, prelude::*, px,
};

use crate::git;
use crate::picker;
use crate::state::{
    AppConfig, AppState, GitChange, ProjectRuntime, SavedProject, SavedSession, SessionRuntime,
    TerminalPair,
};
use crate::storage;
use crate::terminal;

pub struct WorkspaceView {
    state: AppState,
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

        let mut this = Self { state };
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

        let session_runtime = match (
            terminal::spawn_terminal(window, cx, repo_root),
            terminal::spawn_terminal(window, cx, repo_root),
        ) {
            (Ok(main), Ok(side)) => SessionRuntime {
                terminals: Some(TerminalPair { main, side }),
                terminal_error: None,
            },
            (Err(error), _) | (_, Err(error)) => SessionRuntime {
                terminals: None,
                terminal_error: Some(error.to_string()),
            },
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
                    let snapshot = git::collect_changes(&repo_root);

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
                                apply_git_snapshot(runtime, &snapshot);

                                cx.notify();
                                true
                            })
                        })
                        .unwrap_or(false);

                    if !keep_running {
                        break;
                    }

                    cx.background_executor().timer(Duration::from_secs(1)).await;
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
        self.state.flash_error.as_ref().map(|message| {
            div()
                .w_full()
                .px_3()
                .py_2()
                .bg(gpui::red().opacity(0.18))
                .text_color(gpui::white())
                .child(message.clone())
                .into_any_element()
        })
    }

    fn render_left_sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let selected_repo = self.state.selected_repo.as_ref();
        let selected_session = self.state.selected_session.as_ref();

        div()
            .w(px(240.))
            .h_full()
            .flex()
            .flex_col()
            .bg(gpui::rgb(0x101722))
            .border_r_1()
            .border_color(gpui::rgb(0x1e293b))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_3()
                    .border_b_1()
                    .border_color(gpui::rgb(0x1e293b))
                    .child(div().font_weight(gpui::FontWeight::BOLD).child("Projects"))
                    .child(
                        button()
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
                        let select_repo = project.repo_root.clone();
                        let remove_repo = project.repo_root.clone();

                        let mut container = div().flex().flex_col().border_b_1().border_color(
                            gpui::rgb(0x18212f),
                        );

                        // Project header row
                        container = container.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .px_3()
                                .py_2()
                                .bg(if is_project_selected {
                                    gpui::rgba(0x1d4ed820)
                                } else {
                                    gpui::rgba(0x00000000)
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
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
                                                .text_color(if project.last_known_valid {
                                                    gpui::rgb(0xffffff)
                                                } else {
                                                    gpui::rgb(0x94a3b8)
                                                })
                                                .child(project.display_name.clone()),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(gpui::rgb(0x64748b))
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
                                .child(button().px_2().child("x").on_mouse_up(
                                    MouseButton::Left,
                                    cx.listener(move |this, event, window, cx| {
                                        this.on_remove_project(
                                            remove_repo.clone(),
                                            event,
                                            window,
                                            cx,
                                        )
                                    }),
                                )),
                        );

                        // Session rows (only for valid projects)
                        if project.last_known_valid {
                            for session in &project.sessions {
                                let session_repo = project.repo_root.clone();
                                let session_id = session.id.clone();
                                let delete_repo = project.repo_root.clone();
                                let delete_session_id = session.id.clone();
                                let is_session_selected = is_project_selected
                                    && selected_session.is_some_and(|s| s == &session.id);

                                container = container.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .pl(px(28.))
                                        .pr_3()
                                        .py_1()
                                        .bg(if is_session_selected {
                                            gpui::rgba(0x1d4ed838)
                                        } else {
                                            gpui::rgba(0x00000000)
                                        })
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
                                                            gpui::rgb(0x93c5fd)
                                                        } else {
                                                            gpui::rgb(0xcbd5e1)
                                                        })
                                                        .child(session.name.clone()),
                                                ),
                                        )
                                        .child(
                                            div()
                                                .cursor_pointer()
                                                .px_1()
                                                .text_xs()
                                                .text_color(gpui::rgb(0x64748b))
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
                                                .child("x"),
                                        ),
                                );
                            }

                            // "+ New Session" button
                            let add_session_repo = project.repo_root.clone();
                            container = container.child(
                                div()
                                    .pl(px(28.))
                                    .pr_3()
                                    .py_1()
                                    .cursor_pointer()
                                    .on_mouse_up(
                                        MouseButton::Left,
                                        cx.listener(move |this, event, window, cx| {
                                            this.on_add_session(
                                                add_session_repo.clone(),
                                                event,
                                                window,
                                                cx,
                                            )
                                        }),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(gpui::rgb(0x64748b))
                                            .child("+ New Session"),
                                    ),
                            );
                        }

                        container
                    })),
            )
            .into_any_element()
    }

    fn render_center(&self) -> AnyElement {
        if self.state.selected_repo.is_none() {
            return self
                .empty_card("Add a Git repository from the left sidebar.")
                .into_any_element();
        }

        if self.state.selected_session.is_none() {
            return self
                .empty_card("No sessions. Create one from the sidebar.")
                .into_any_element();
        }

        let Some(session_runtime) = self.selected_session_runtime() else {
            return self
                .empty_card("Loading session...")
                .into_any_element();
        };

        if let Some(error) = &session_runtime.terminal_error {
            return self
                .empty_card(&format!("Terminal failed to start: {error}"))
                .into_any_element();
        }

        if let Some(terminals) = &session_runtime.terminals {
            return div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .bg(gpui::black())
                .child(terminals.main.clone())
                .into_any_element();
        }

        self.empty_card("Starting terminal...")
            .into_any_element()
    }

    fn render_right_sidebar(&self) -> AnyElement {
        let top = self.render_changes_panel();
        let bottom = self.render_side_terminal();

        div()
            .w(px(320.))
            .h_full()
            .flex()
            .flex_col()
            .bg(gpui::rgb(0x0f172a))
            .border_l_1()
            .border_color(gpui::rgb(0x1e293b))
            .child(top)
            .child(bottom)
            .into_any_element()
    }

    fn render_changes_panel(&self) -> AnyElement {
        let snapshot = self
            .selected_project_runtime()
            .map(|runtime| &runtime.git_snapshot);
        let changes = snapshot
            .map(|snapshot| snapshot.changes.as_slice())
            .unwrap_or(&[]);
        let error = snapshot.and_then(|snapshot| snapshot.last_error.as_ref());

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .border_b_1()
            .border_color(gpui::rgb(0x1e293b))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(gpui::rgb(0x1e293b))
                    .child("Changes"),
            )
            .when_some(error, |panel, message| {
                panel.child(
                    div()
                        .mx_3()
                        .mt_3()
                        .px_2()
                        .py_1()
                        .text_xs()
                        .bg(gpui::red().opacity(0.16))
                        .text_color(gpui::white())
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
                        vec![Self::empty_change_row("Working tree clean")]
                    } else {
                        changes
                            .iter()
                            .map(Self::render_change_row)
                            .collect::<Vec<_>>()
                    }),
            )
            .into_any_element()
    }

    fn render_change_row(change: &GitChange) -> AnyElement {
        div()
            .flex()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(gpui::rgb(0x18212f))
            .child(
                div()
                    .min_w(px(34.))
                    .text_xs()
                    .text_color(gpui::rgb(0x93c5fd))
                    .child(change.status_code.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(gpui::white())
                    .child(change.path.clone()),
            )
            .into_any_element()
    }

    fn empty_change_row(message: &str) -> AnyElement {
        div()
            .px_3()
            .py_3()
            .text_color(gpui::rgb(0x64748b))
            .child(message.to_string())
            .into_any_element()
    }

    fn render_side_terminal(&self) -> AnyElement {
        let Some(session_runtime) = self.selected_session_runtime() else {
            return self
                .empty_card("Select a session to start the side terminal.")
                .into_any_element();
        };

        if let Some(error) = &session_runtime.terminal_error {
            return self
                .empty_card(&format!("Terminal failed to start: {error}"))
                .into_any_element();
        }

        if let Some(terminals) = &session_runtime.terminals {
            return div()
                .flex_1()
                .min_h_0()
                .bg(gpui::black())
                .child(terminals.side.clone())
                .into_any_element();
        }

        self.empty_card("Starting side terminal...")
            .into_any_element()
    }

    fn empty_card(&self, message: &str) -> Div {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgb(0x020617))
            .text_color(gpui::rgb(0x94a3b8))
            .child(
                div()
                    .max_w(px(320.))
                    .text_center()
                    .child(message.to_string()),
            )
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(gpui::rgb(0x020617))
            .text_color(gpui::white())
            .children(self.flash_banner())
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.render_left_sidebar(cx))
                    .child(self.render_center())
                    .child(self.render_right_sidebar()),
            )
    }
}

fn refresh_project_validity(projects: &mut [SavedProject]) {
    for project in projects {
        project.last_known_valid = git::is_valid_repo(&project.repo_root);
    }
}

fn apply_git_snapshot(runtime: &mut ProjectRuntime, snapshot: &Result<Vec<GitChange>, String>) {
    runtime.git_snapshot.last_refresh = Some(Instant::now());

    match snapshot {
        Ok(changes) => {
            runtime.git_snapshot.changes = changes.clone();
            runtime.git_snapshot.last_error = None;
        }
        Err(error) => {
            runtime.git_snapshot.last_error = Some(error.clone());
        }
    }
}

fn button() -> Div {
    div()
        .px_3()
        .py_1()
        .rounded_md()
        .bg(gpui::rgb(0x1e293b))
        .text_color(gpui::white())
        .hover(|style| style.bg(gpui::rgb(0x334155)).cursor_pointer())
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
