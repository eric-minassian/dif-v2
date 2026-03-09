mod bottom_panel;
mod changes_list;
mod checks_popover;
pub mod config;
mod diff_view;
mod git_actions;
mod git_poll;
mod help;
mod helpers;
pub mod keybindings;
mod left_panel;
mod pane_group;
mod panel_action;
mod picker;
mod project;
mod right_panel;
pub mod runtime;
mod session;
mod settings;
mod sidebar;
pub mod storage;
mod titlebar;
mod ui_state;
mod update_actions;
mod updater;

#[cfg(test)]
mod workspace_tests;

use std::path::{Path, PathBuf};

use gpui::{App, CursorStyle, Entity, FocusHandle, Focusable, MouseButton, Subscription, actions};

use config::{
    AppConfig, DEFAULT_BOTTOM_PANEL_HEIGHT, DEFAULT_LEFT_SIDEBAR_WIDTH, DEFAULT_RIGHT_SIDEBAR_WIDTH,
};
use pane_group::PaneGroup;
use runtime::{AppState, ProjectRuntime, SessionRuntime, TerminalTab};
use ui::empty_state;
use ui::prelude::*;
use ui::text_input::TextInput;
use ui_state::ResizingSidebar;

use helpers::{
    pick_initial_selection, pick_initial_session, refresh_project_validity, resize_handle,
    resize_handle_horizontal,
};

actions!(
    workspace,
    [
        NewSideTab,
        CloseSideTab,
        CloseOtherTabs,
        CloseAllTabs,
        SelectSession1,
        SelectSession2,
        SelectSession3,
        SelectSession4,
        SelectSession5,
        SelectSession6,
        SelectSession7,
        SelectSession8,
        SelectSession9,
        CloseDiffView,
        ToggleLeftSidebar,
        ToggleRightSidebar,
        ToggleBottomPanel,
        RefreshGitStatus,
        OpenSettings,
        NewSession,
        FocusTerminal,
        SplitTerminalRight,
        SplitTerminalLeft,
        SplitTerminalDown,
        SplitTerminalUp,
        ActivatePaneLeft,
        ActivatePaneRight,
        ActivatePaneUp,
        ActivatePaneDown,
        ToggleZoomTerminalPane,
        NextTerminalTab,
        PrevTerminalTab,
        ToggleHelp,
        RunGitAction,
        UpdateFromMain,
        AbortRebase,
        CopyConflictPrompt,
        Quit,
        HideApp,
        HideOtherApps,
        MinimizeWindow,
        ZoomWindow,
    ]
);

struct InlineEdit {
    repo_root: PathBuf,
    input: Entity<TextInput>,
    _event_sub: Subscription,
    _blur_sub: Subscription,
}

struct SessionRename {
    edit: InlineEdit,
    session_id: String,
}

struct SessionCreate {
    edit: InlineEdit,
    validation_error: Option<String>,
}

struct SettingsEdit {
    repo_root: PathBuf,
    input: Entity<TextInput>,
    /// `None` = adding a new command, `Some(i)` = editing command at index `i`.
    editing_index: Option<usize>,
    _event_sub: Subscription,
    _blur_sub: Subscription,
}

pub struct WorkspaceView {
    state: AppState,
    focus_handle: FocusHandle,
    renaming_session: Option<SessionRename>,
    creating_session: Option<SessionCreate>,
    settings_input: Option<SettingsEdit>,
    _window_activation_sub: Subscription,
    _git_poll_task: Option<gpui::Task<()>>,
}

impl Focusable for WorkspaceView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl WorkspaceView {
    pub fn new(config: AppConfig, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let left_w = config
            .left_sidebar_width
            .unwrap_or(DEFAULT_LEFT_SIDEBAR_WIDTH);
        let right_w = config
            .right_sidebar_width
            .unwrap_or(DEFAULT_RIGHT_SIDEBAR_WIDTH);
        let bottom_h = config
            .bottom_panel_height
            .unwrap_or(DEFAULT_BOTTOM_PANEL_HEIGHT);
        let mut state = AppState {
            config,
            left_sidebar_width: left_w,
            right_sidebar_width: right_w,
            bottom_panel_height: bottom_h,
            bottom_panel_collapsed: true,
            ..AppState::default()
        };
        refresh_project_validity(&mut state.config.projects);

        state.selected_repo = pick_initial_selection(&state.config);
        state.selected_session = state
            .selected_repo
            .as_ref()
            .and_then(|repo| pick_initial_session(&state.config, repo));

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let window_activation_sub =
            cx.observe_window_activation(window, Self::on_window_activation_changed);

        let mut this = Self {
            state,
            focus_handle,
            renaming_session: None,
            creating_session: None,
            settings_input: None,
            _window_activation_sub: window_activation_sub,
            _git_poll_task: None,
        };
        if let Some(repo) = this.state.selected_repo.clone()
            && let Some(session_id) = this.state.selected_session.clone()
        {
            this.activate_session(repo, session_id, window, cx);
        }

        this.spawn_update_check(window, cx);

        this
    }

    fn on_window_activation_changed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
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

    fn working_dir(&self, repo: &Path) -> PathBuf {
        self.state
            .selected_session
            .as_deref()
            .map(|sid| self.worktree_or_repo(repo, sid))
            .unwrap_or_else(|| repo.to_path_buf())
    }

    fn ensure_session_runtime(
        &mut self,
        repo_root: &Path,
        session_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(runtime) = self.state.runtimes.get(repo_root)
            && runtime.session_runtimes.contains_key(session_id)
        {
            return;
        }

        let working_dir = self.worktree_or_repo(repo_root, session_id);

        // Create main terminal
        let (main_terminal, main_error) = match terminal::spawn_terminal(window, cx, &working_dir) {
            Ok(view) => (Some(view), None),
            Err(error) => (None, Some(error.to_string())),
        };

        // Create initial bottom panel terminal tab
        let tabs = match terminal::spawn_terminal(window, cx, &working_dir) {
            Ok(view) => {
                vec![TerminalTab {
                    pane_group: PaneGroup::new(view.clone()),
                    active_pane: Some(view),
                    zoomed_pane_group: None,
                }]
            }
            Err(error) => {
                self.state.flash_error = Some(format!("Failed to create side terminal: {error}"));
                Vec::new()
            }
        };

        let session_runtime = SessionRuntime {
            main_terminal,
            main_terminal_error: main_error,
            tabs,
            active_tab_index: 0,
            cached_branch_status: None,
            cached_repo_capabilities: None,
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

    fn run_init_commands(
        &mut self,
        repo_root: &Path,
        worktree_path: &Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let commands = self
            .state
            .config
            .projects
            .iter()
            .find(|p| p.repo_root.as_path() == repo_root)
            .map(|p| p.settings.workspace_init_commands.clone())
            .unwrap_or_default();

        if commands.is_empty() {
            return;
        }

        let wt = worktree_path.to_path_buf();
        let repo = repo_root.to_path_buf();
        let view = cx.entity().clone();

        window
            .spawn(cx, async move |cx| {
                // Use the user's login shell so tools like pnpm/node/etc. are on PATH.
                // macOS GUI apps don't inherit the shell's PATH, and a bare `sh -c`
                // won't source ~/.zshrc or ~/.bashrc.
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

                for cmd in &commands {
                    let shell = shell.clone();
                    let cmd = cmd.clone();
                    let wt = wt.clone();
                    let repo = repo.clone();
                    let cmd_display = cmd.clone();

                    let result = cx
                        .background_executor()
                        .spawn(async move {
                            std::process::Command::new(&shell)
                                .arg("-l")
                                .arg("-c")
                                .arg(&cmd)
                                .current_dir(&wt)
                                .env("DIF_WORKTREE_DIR", &wt)
                                .env("DIF_REPO_DIR", &repo)
                                .output()
                        })
                        .await;

                    let error = match result {
                        Ok(output) if !output.status.success() => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            Some(format!("Init command failed: {cmd_display}\n{stderr}"))
                        }
                        Err(e) => Some(format!("Init command failed: {cmd_display}\n{e}")),
                        _ => None,
                    };

                    if let Some(msg) = error {
                        let view = view.clone();
                        cx.update(|_, cx| {
                            view.update(cx, |this, cx| {
                                this.state.flash_error = Some(msg);
                                cx.notify();
                            })
                        })
                        .ok();
                        return;
                    }
                }
            })
            .detach();
    }

    fn selected_project_runtime(&self) -> Option<&ProjectRuntime> {
        let repo = self.state.selected_repo.as_ref()?;
        self.state.runtimes.get(repo)
    }

    fn selected_project_runtime_mut(&mut self) -> Option<&mut ProjectRuntime> {
        let repo = self.state.selected_repo.as_ref()?;
        self.state.runtimes.get_mut(repo)
    }

    fn selected_session_runtime(&self) -> Option<&SessionRuntime> {
        let repo = self.state.selected_repo.as_ref()?;
        let session_id = self.state.selected_session.as_ref()?;
        let project_runtime = self.state.runtimes.get(repo)?;
        project_runtime.session_runtimes.get(session_id)
    }

    fn selected_session_runtime_mut(&mut self) -> Option<&mut SessionRuntime> {
        let repo = self.state.selected_repo.as_ref()?;
        let session_id = self.state.selected_session.as_ref()?;
        let project_runtime = self.state.runtimes.get_mut(repo)?;
        project_runtime.session_runtimes.get_mut(session_id)
    }

    fn flash_banner(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = theme();
        self.state.flash_error.as_ref().map(|message| {
            h_flex()
                .id("flash-banner")
                .w_full()
                .px_3()
                .py_2()
                .bg(t.error_bg)
                .justify_between()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_xs()
                        .text_color(t.text_primary)
                        .child(message.clone()),
                )
                .child(
                    div()
                        .id("dismiss-flash")
                        .cursor_pointer()
                        .px_1()
                        .text_color(t.text_dim)
                        .hover(|s| s.text_color(t.text_primary))
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.state.flash_error = None;
                            cx.notify();
                        }))
                        .child(Icon::new(IconName::X).size(px(14.)).color(Color::Dim)),
                )
                .into_any_element()
        })
    }

    fn render_center(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.state.viewing_help {
            return self.render_help_view(cx);
        }

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

impl WorkspaceView {
    fn register_actions(
        &self,
        root: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        root.on_action(cx.listener(|this, _: &NewSideTab, window, cx| {
            this.on_add_terminal(window, cx);
        }))
        .on_action(cx.listener(|this, _: &CloseSideTab, window, cx| {
            this.on_close_active_terminal(window, cx);
        }))
        .on_action(cx.listener(|_this, _: &MinimizeWindow, window, _cx| {
            window.minimize_window();
        }))
        .on_action(cx.listener(|_this, _: &ZoomWindow, window, _cx| {
            window.zoom_window();
        }))
        .on_action(cx.listener(|this, _: &CloseDiffView, _window, cx| {
            if this.state.viewing_help {
                this.state.viewing_help = false;
                cx.notify();
            } else if this.state.viewing_settings {
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
        .on_action(cx.listener(|this, _: &ToggleBottomPanel, window, cx| {
            this.on_toggle_bottom_panel(window, cx);
        }))
        .on_action(cx.listener(|this, _: &RefreshGitStatus, window, cx| {
            this.on_refresh_git_status(window, cx);
        }))
        .on_action(cx.listener(|this, _: &SelectSession1, window, cx| {
            this.select_session_by_index(0, window, cx);
        }))
        .on_action(cx.listener(|this, _: &SelectSession2, window, cx| {
            this.select_session_by_index(1, window, cx);
        }))
        .on_action(cx.listener(|this, _: &SelectSession3, window, cx| {
            this.select_session_by_index(2, window, cx);
        }))
        .on_action(cx.listener(|this, _: &SelectSession4, window, cx| {
            this.select_session_by_index(3, window, cx);
        }))
        .on_action(cx.listener(|this, _: &SelectSession5, window, cx| {
            this.select_session_by_index(4, window, cx);
        }))
        .on_action(cx.listener(|this, _: &SelectSession6, window, cx| {
            this.select_session_by_index(5, window, cx);
        }))
        .on_action(cx.listener(|this, _: &SelectSession7, window, cx| {
            this.select_session_by_index(6, window, cx);
        }))
        .on_action(cx.listener(|this, _: &SelectSession8, window, cx| {
            this.select_session_by_index(7, window, cx);
        }))
        .on_action(cx.listener(|this, _: &SelectSession9, window, cx| {
            this.select_session_by_index(8, window, cx);
        }))
        .on_action(cx.listener(|this, _: &OpenSettings, _window, cx| {
            this.on_open_settings(cx);
        }))
        .on_action(cx.listener(|this, _: &NewSession, window, cx| {
            this.on_new_session(window, cx);
        }))
        .on_action(cx.listener(|this, _: &FocusTerminal, window, cx| {
            this.on_focus_terminal(window, cx);
        }))
        .on_action(cx.listener(|this, _: &SplitTerminalRight, window, cx| {
            this.on_split_terminal(window, cx, pane_group::SplitDirection::Right);
        }))
        .on_action(cx.listener(|this, _: &SplitTerminalLeft, window, cx| {
            this.on_split_terminal(window, cx, pane_group::SplitDirection::Left);
        }))
        .on_action(cx.listener(|this, _: &SplitTerminalDown, window, cx| {
            this.on_split_terminal(window, cx, pane_group::SplitDirection::Down);
        }))
        .on_action(cx.listener(|this, _: &SplitTerminalUp, window, cx| {
            this.on_split_terminal(window, cx, pane_group::SplitDirection::Up);
        }))
        .on_action(cx.listener(|this, _: &ActivatePaneLeft, window, cx| {
            this.on_activate_pane_in_direction(window, cx, pane_group::SplitDirection::Left);
        }))
        .on_action(cx.listener(|this, _: &ActivatePaneRight, window, cx| {
            this.on_activate_pane_in_direction(window, cx, pane_group::SplitDirection::Right);
        }))
        .on_action(cx.listener(|this, _: &ActivatePaneUp, window, cx| {
            this.on_activate_pane_in_direction(window, cx, pane_group::SplitDirection::Up);
        }))
        .on_action(cx.listener(|this, _: &ActivatePaneDown, window, cx| {
            this.on_activate_pane_in_direction(window, cx, pane_group::SplitDirection::Down);
        }))
        .on_action(
            cx.listener(|this, _: &ToggleZoomTerminalPane, _window, cx| {
                this.on_toggle_zoom_terminal_pane(cx);
            }),
        )
        .on_action(cx.listener(|this, _: &NextTerminalTab, window, cx| {
            this.on_next_terminal_tab(window, cx);
        }))
        .on_action(cx.listener(|this, _: &PrevTerminalTab, window, cx| {
            this.on_prev_terminal_tab(window, cx);
        }))
        .on_action(cx.listener(|this, _: &CloseOtherTabs, _window, cx| {
            this.on_close_other_tabs(cx);
        }))
        .on_action(cx.listener(|this, _: &CloseAllTabs, _window, cx| {
            this.on_close_all_tabs(cx);
        }))
        .on_action(cx.listener(|this, _: &RunGitAction, window, cx| {
            this.on_run_git_action(window, cx);
        }))
        .on_action(cx.listener(|this, _: &UpdateFromMain, window, cx| {
            this.on_update_from_main(window, cx);
        }))
        .on_action(cx.listener(|this, _: &AbortRebase, window, cx| {
            this.on_abort_rebase(window, cx);
        }))
        .on_action(cx.listener(|this, _: &CopyConflictPrompt, _window, cx| {
            this.on_copy_conflict_prompt(cx);
        }))
        .on_action(cx.listener(|this, _: &ToggleHelp, _window, cx| {
            this.state.viewing_help = !this.state.viewing_help;
            cx.notify();
        }))
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        // Track which terminal pane has focus (for split-pane click-to-focus)
        self.track_focused_terminal_pane(window, cx);

        let show_session_shortcuts = window.modifiers().platform;

        let left = if self.state.left_sidebar_collapsed {
            self.render_collapsed_left_sidebar()
        } else {
            self.render_left_sidebar(show_session_shortcuts, cx)
        };

        let right = if self.state.right_sidebar_collapsed {
            self.render_collapsed_right_sidebar()
        } else {
            self.render_right_sidebar(cx)
        };

        let is_resizing_h = matches!(
            self.state.resizing_sidebar,
            Some(ResizingSidebar::Left | ResizingSidebar::Right)
        );
        let is_resizing_v = matches!(self.state.resizing_sidebar, Some(ResizingSidebar::Bottom));
        let left_collapsed = self.state.left_sidebar_collapsed;
        let right_collapsed = self.state.right_sidebar_collapsed;
        let bottom_collapsed = self.state.bottom_panel_collapsed;

        // Compute checks popover state (clone to avoid borrow conflicts)
        let popover_branch_status = self
            .selected_project_runtime()
            .map(|rt| rt.branch_status.clone());
        let checks_popover_open = self.state.checks_popover_open
            && popover_branch_status
                .as_ref()
                .map_or(false, |bs| !bs.checks.is_empty());

        let root = div().id("workspace").track_focus(&self.focus_handle);

        let mut el = self
            .register_actions(root, cx)
            .on_modifiers_changed(
                cx.listener(|_, _: &gpui::ModifiersChangedEvent, _window, cx| {
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(Self::on_resize_drag))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_resize_end))
            .when(is_resizing_h, |el| el.cursor(CursorStyle::ResizeLeftRight))
            .when(is_resizing_v, |el| el.cursor(CursorStyle::ResizeUpDown))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(t.bg_base)
            .text_color(t.text_primary)
            .child(self.render_titlebar(window, cx))
            .children(self.flash_banner(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(left)
                    .when(!left_collapsed, |el| {
                        el.child(resize_handle("left-resize", cx, ResizingSidebar::Left))
                    })
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(self.render_center(cx))
                            .when(!bottom_collapsed, |el| {
                                el.child(resize_handle_horizontal("bottom-resize", cx))
                                    .child(self.render_bottom_panel(cx))
                            }),
                    )
                    .when(!right_collapsed, |el| {
                        el.child(resize_handle("right-resize", cx, ResizingSidebar::Right))
                    })
                    .child(right),
            );

        // Checks popover rendered at workspace root for proper z-ordering
        if checks_popover_open {
            if let Some(ref bs) = popover_branch_status {
                let backdrop_listener =
                    cx.listener(|this, _event: &gpui::MouseUpEvent, _window, cx| {
                        this.on_close_checks_popover(cx);
                    });
                el = el
                    .child(
                        div()
                            .id("checks-backdrop")
                            .occlude()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .on_mouse_up(MouseButton::Left, backdrop_listener),
                    )
                    .child(self.render_checks_popover(bs, cx));
            }
        }

        el
    }
}
