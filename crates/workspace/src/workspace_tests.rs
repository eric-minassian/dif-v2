use crate::WorkspaceView;
use crate::config::{AppConfig, SavedProject, SavedSession};
use std::path::PathBuf;

/// Create a WorkspaceView with an empty config (no repos, no terminals).
fn build_empty_workspace(
    window: &mut gpui::Window,
    cx: &mut gpui::Context<WorkspaceView>,
) -> WorkspaceView {
    WorkspaceView::new(AppConfig::default(), window, cx)
}

/// Create a WorkspaceView with a single project that has sessions,
/// but uses a non-existent repo path so no terminals are spawned.
fn build_workspace_with_project(
    window: &mut gpui::Window,
    cx: &mut gpui::Context<WorkspaceView>,
) -> WorkspaceView {
    let config = AppConfig {
        projects: vec![SavedProject {
            repo_root: PathBuf::from("/nonexistent/test-repo"),
            display_name: "test-repo".to_string(),
            last_known_valid: false,
            sessions: vec![
                SavedSession {
                    id: "1".to_string(),
                    name: "main".to_string(),
                    worktree_path: None,
                },
                SavedSession {
                    id: "2".to_string(),
                    name: "feature".to_string(),
                    worktree_path: None,
                },
            ],
            last_selected_session: Some("1".to_string()),
            settings: Default::default(),
        }],
        last_selected_repo: None,
        left_sidebar_width: Some(250.0),
        right_sidebar_width: Some(300.0),
        bottom_panel_height: Some(200.0),
    };
    WorkspaceView::new(config, window, cx)
}

// ── Empty workspace state ──────────────────────────────────────────────

#[gpui::test]
fn test_empty_workspace_initial_state(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, _window, _cx| {
        assert!(ws.state.selected_repo.is_none());
        assert!(ws.state.selected_session.is_none());
        assert!(ws.state.config.projects.is_empty());
        assert!(ws.state.flash_error.is_none());
        assert!(!ws.state.viewing_help);
        assert!(!ws.state.viewing_settings);
        assert!(ws.state.viewing_diff.is_none());
    });
}

#[gpui::test]
fn test_empty_workspace_sidebar_defaults(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, _window, _cx| {
        assert!(!ws.state.left_sidebar_collapsed);
        assert!(!ws.state.right_sidebar_collapsed);
        assert!(ws.state.bottom_panel_collapsed);
    });
}

// ── Sidebar toggling ───────────────────────────────────────────────────

#[gpui::test]
fn test_toggle_left_sidebar(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, _window, cx| {
        assert!(!ws.state.left_sidebar_collapsed);
        ws.on_toggle_left_sidebar(cx);
        assert!(ws.state.left_sidebar_collapsed);
        ws.on_toggle_left_sidebar(cx);
        assert!(!ws.state.left_sidebar_collapsed);
    });
}

#[gpui::test]
fn test_toggle_right_sidebar(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, _window, cx| {
        assert!(!ws.state.right_sidebar_collapsed);
        ws.on_toggle_right_sidebar(cx);
        assert!(ws.state.right_sidebar_collapsed);
        ws.on_toggle_right_sidebar(cx);
        assert!(!ws.state.right_sidebar_collapsed);
    });
}

#[gpui::test]
fn test_toggle_bottom_panel(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, window, cx| {
        assert!(ws.state.bottom_panel_collapsed);
        ws.on_toggle_bottom_panel(window, cx);
        assert!(!ws.state.bottom_panel_collapsed);
        ws.on_toggle_bottom_panel(window, cx);
        assert!(ws.state.bottom_panel_collapsed);
    });
}

// ── Help and settings views ────────────────────────────────────────────

#[gpui::test]
fn test_toggle_help_view(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, _window, _cx| {
        assert!(!ws.state.viewing_help);
        ws.state.viewing_help = true;
        assert!(ws.state.viewing_help);
        ws.state.viewing_help = false;
        assert!(!ws.state.viewing_help);
    });
}

#[gpui::test]
fn test_toggle_settings_view(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, _window, cx| {
        assert!(!ws.state.viewing_settings);
        ws.on_open_settings(cx);
        assert!(ws.state.viewing_settings);
        ws.on_close_settings(cx);
        assert!(!ws.state.viewing_settings);
    });
}

// ── Config with project ────────────────────────────────────────────────

#[gpui::test]
fn test_workspace_with_project_loads_config(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_workspace_with_project);
    _ = window.update(cx, |ws, _window, _cx| {
        assert_eq!(ws.state.config.projects.len(), 1);
        assert_eq!(ws.state.config.projects[0].sessions.len(), 2);
        assert_eq!(ws.state.left_sidebar_width, 250.0);
        assert_eq!(ws.state.right_sidebar_width, 300.0);
        assert_eq!(ws.state.bottom_panel_height, 200.0);
    });
}

#[gpui::test]
fn test_workspace_invalid_repo_not_selected(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_workspace_with_project);
    _ = window.update(cx, |ws, _window, _cx| {
        // Repo is invalid (last_known_valid=false), so it should not be selected
        assert!(ws.state.selected_repo.is_none());
        assert!(ws.state.selected_session.is_none());
    });
}

// ── Flash error ────────────────────────────────────────────────────────

#[gpui::test]
fn test_flash_error_state(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, _window, _cx| {
        assert!(ws.state.flash_error.is_none());
        ws.state.flash_error = Some("something went wrong".to_string());
        assert_eq!(
            ws.state.flash_error.as_deref(),
            Some("something went wrong")
        );
        ws.state.flash_error = None;
        assert!(ws.state.flash_error.is_none());
    });
}

// ── Diff view state ────────────────────────────────────────────────────

#[gpui::test]
fn test_diff_view_close(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, _window, cx| {
        assert!(ws.state.viewing_diff.is_none());
        ws.on_close_diff(cx);
        assert!(ws.state.viewing_diff.is_none()); // no-op when already closed
    });
}

// ── Checks popover ─────────────────────────────────────────────────────

#[gpui::test]
fn test_checks_popover_toggle(cx: &mut gpui::TestAppContext) {
    let window = cx.add_window(build_empty_workspace);
    _ = window.update(cx, |ws, _window, cx| {
        assert!(!ws.state.checks_popover_open);
        ws.on_toggle_checks_popover(cx);
        assert!(ws.state.checks_popover_open);
        ws.on_close_checks_popover(cx);
        assert!(!ws.state.checks_popover_open);
    });
}
