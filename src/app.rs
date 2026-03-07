use gpui::{App, AppContext, KeyBinding, TitlebarOptions, WindowOptions, point, px};

use crate::assets::Assets;
use terminal::view::{Copy, Paste, SelectAll};
use ui::text_input;
use workspace::{
    CloseDiffView, CloseSideTab, FocusTerminal, HideApp, HideOtherApps, MinimizeWindow, NewSession,
    NewSideTab, OpenSettings, Quit, RefreshGitStatus, RunGitAction, SelectSession1, SelectSession2,
    SelectSession3, SelectSession4, SelectSession5, SelectSession6, SelectSession7, SelectSession8,
    SelectSession9, ToggleHelp, ToggleLeftSidebar, ToggleRightSidebar, WorkspaceView,
};

pub fn run() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            cx.bind_keys(text_input::key_bindings());
            cx.bind_keys([
                // Standard macOS application keybindings
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-h", HideApp, None),
                KeyBinding::new("cmd-alt-h", HideOtherApps, None),
                KeyBinding::new("cmd-m", MinimizeWindow, None),
                KeyBinding::new("cmd-w", CloseSideTab, None),
                // App keybindings
                KeyBinding::new("escape", CloseDiffView, None),
                KeyBinding::new("cmd-t", NewSideTab, None),
                KeyBinding::new("cmd-b", ToggleLeftSidebar, None),
                KeyBinding::new("cmd-shift-b", ToggleRightSidebar, None),
                KeyBinding::new("cmd-r", RefreshGitStatus, None),
                KeyBinding::new("cmd-,", OpenSettings, None),
                KeyBinding::new("cmd-n", NewSession, None),
                KeyBinding::new("cmd-`", FocusTerminal, None),
                KeyBinding::new("cmd-/", ToggleHelp, None),
                KeyBinding::new("cmd-enter", RunGitAction, None),
                KeyBinding::new("cmd-1", SelectSession1, None),
                KeyBinding::new("cmd-2", SelectSession2, None),
                KeyBinding::new("cmd-3", SelectSession3, None),
                KeyBinding::new("cmd-4", SelectSession4, None),
                KeyBinding::new("cmd-5", SelectSession5, None),
                KeyBinding::new("cmd-6", SelectSession6, None),
                KeyBinding::new("cmd-7", SelectSession7, None),
                KeyBinding::new("cmd-8", SelectSession8, None),
                KeyBinding::new("cmd-9", SelectSession9, None),
                KeyBinding::new("cmd-a", SelectAll, None),
                KeyBinding::new("cmd-c", Copy, None),
                KeyBinding::new("cmd-v", Paste, None),
            ]);

            // Global action handlers for standard macOS behaviors
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.on_action(|_: &HideApp, cx| cx.hide());
            cx.on_action(|_: &HideOtherApps, cx| cx.hide_other_apps());

            let config = workspace::storage::load_config().unwrap_or_default();

            let window_options = WindowOptions {
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(9.0), px(9.0))),
                    ..Default::default()
                }),
                ..Default::default()
            };

            cx.open_window(window_options, |window, cx| {
                cx.new(|cx| WorkspaceView::new(config, window, cx))
            })
            .expect("failed to open the main window");

            cx.activate(true);
        });
}
