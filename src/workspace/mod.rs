mod diff_view;
mod git_actions;
mod helpers;
mod left_panel;
mod project;
mod right_panel;
mod session;
mod settings;
mod sidebar;
mod tab_bar;
mod titlebar;

use std::path::{Path, PathBuf};

use gpui::{
    actions, AnyElement, Context, CursorStyle, Entity, FocusHandle, MouseButton,
    Subscription, Window, div, prelude::*,
};

use crate::components::empty_state;
use crate::state::{
    AppConfig, AppState, ProjectRuntime, ResizingSidebar, SessionRuntime, TerminalTab,
    DEFAULT_LEFT_SIDEBAR_WIDTH, DEFAULT_RIGHT_SIDEBAR_WIDTH,
};
use crate::storage;
use crate::terminal;
use crate::text_input::TextInput;
use crate::theme::theme;

use helpers::{pick_initial_selection, pick_initial_session, refresh_project_validity, resize_handle};

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
    commit_input_focus: FocusHandle,
    /// (repo_root, session_id, input entity, event subscription, blur subscription)
    renaming_session: Option<(PathBuf, String, Entity<TextInput>, Subscription, Subscription)>,
    /// (repo_root, input entity, event subscription) for adding init commands in settings
    settings_input: Option<(PathBuf, Entity<TextInput>, Subscription)>,
}

impl WorkspaceView {
    pub fn new(config: AppConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let left_w = config
            .left_sidebar_width
            .unwrap_or(DEFAULT_LEFT_SIDEBAR_WIDTH);
        let right_w = config
            .right_sidebar_width
            .unwrap_or(DEFAULT_RIGHT_SIDEBAR_WIDTH);
        let mut state = AppState {
            config,
            left_sidebar_width: left_w,
            right_sidebar_width: right_w,
            ..AppState::default()
        };
        refresh_project_validity(&mut state.config.projects);

        state.selected_repo = pick_initial_selection(&state.config);
        state.selected_session = state
            .selected_repo
            .as_ref()
            .and_then(|repo| pick_initial_session(&state.config, repo));

        let focus_handle = cx.focus_handle();
        let commit_input_focus = cx.focus_handle();
        focus_handle.focus(window, cx);

        let mut this = Self {
            state,
            focus_handle,
            commit_input_focus,
            renaming_session: None,
            settings_input: None,
        };
        if let Some(repo) = this.state.selected_repo.clone() {
            if let Some(session_id) = this.state.selected_session.clone() {
                this.activate_session(repo, session_id, window, cx);
            }
        }

        this
    }

    fn worktree_or_repo(&self, repo_root: &Path, session_id: &str) -> PathBuf {
        self.state
            .config
            .projects
            .iter()
            .find(|p| p.repo_root.as_path() == repo_root)
            .and_then(|p| p.sessions.iter().find(|s| s.id == session_id))
            .and_then(|s| s.worktree_path.as_ref())
            .filter(|p| p.exists())
            .cloned()
            .unwrap_or_else(|| repo_root.to_path_buf())
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

        let working_dir = self.worktree_or_repo(repo_root, session_id);

        // Create main terminal
        let (main_terminal, main_error) = match terminal::spawn_terminal(window, cx, &working_dir) {
            Ok(view) => (Some(view), None),
            Err(error) => (None, Some(error.to_string())),
        };

        // Create initial side terminal tab
        let (side_tabs, selected_tab, next_id) =
            match terminal::spawn_terminal(window, cx, &working_dir) {
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

    fn persist_config(&mut self) {
        if let Err(error) = storage::save_config(&self.state.config) {
            self.state.flash_error = Some(error.to_string());
        }
    }

    fn run_init_commands(&mut self, repo_root: &Path, worktree_path: &Path) {
        let commands = self
            .state
            .config
            .projects
            .iter()
            .find(|p| p.repo_root.as_path() == repo_root)
            .map(|p| p.settings.workspace_init_commands.clone())
            .unwrap_or_default();

        for cmd in &commands {
            let result = std::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(worktree_path)
                .env("DIF_WORKTREE_DIR", worktree_path)
                .env("DIF_REPO_DIR", repo_root)
                .output();

            match result {
                Ok(output) if !output.status.success() => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    self.state.flash_error = Some(format!(
                        "Init command failed: {cmd}\n{stderr}"
                    ));
                }
                Err(error) => {
                    self.state.flash_error = Some(format!(
                        "Init command failed: {cmd}\n{error}"
                    ));
                }
                _ => {}
            }
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

    fn render_center(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.state.viewing_settings {
            return self.render_settings_view(cx);
        }

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

        let is_resizing = self.state.resizing_sidebar.is_some();
        let left_collapsed = self.state.left_sidebar_collapsed;
        let right_collapsed = self.state.right_sidebar_collapsed;

        div()
            .id("workspace")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &NewSideTab, window, cx| {
                this.on_add_side_tab(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseDiffView, _window, cx| {
                if this.state.viewing_settings {
                    this.on_close_settings(cx);
                } else if this.state.viewing_diff.is_some() {
                    this.on_close_diff(cx);
                } else {
                    cx.propagate();
                }
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
            .on_mouse_move(cx.listener(Self::on_resize_drag))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_resize_end))
            .when(is_resizing, |el| el.cursor(CursorStyle::ResizeLeftRight))
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
                    .when(!left_collapsed, |el| {
                        el.child(resize_handle("left-resize", cx, ResizingSidebar::Left))
                    })
                    .child(self.render_center(cx))
                    .when(!right_collapsed, |el| {
                        el.child(resize_handle("right-resize", cx, ResizingSidebar::Right))
                    })
                    .child(right),
            )
    }
}
