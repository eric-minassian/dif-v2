use gpui::{App, AppContext, Application, KeyBinding, TitlebarOptions, WindowOptions, point, px};
use crate::terminal_view::view::{Copy, Paste, SelectAll};

use crate::storage;
use crate::ui::{
    CloseDiffView, NewSideTab, RefreshGitStatus, SelectSideTab1, SelectSideTab2, SelectSideTab3,
    SelectSideTab4, SelectSideTab5, SelectSideTab6, SelectSideTab7, SelectSideTab8, SelectSideTab9,
    ToggleLeftSidebar, ToggleRightSidebar, WorkspaceView,
};

pub fn run() {
    Application::new().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("escape", CloseDiffView, None),
            KeyBinding::new("cmd-t", NewSideTab, None),
            KeyBinding::new("cmd-b", ToggleLeftSidebar, None),
            KeyBinding::new("cmd-shift-b", ToggleRightSidebar, None),
            KeyBinding::new("cmd-r", RefreshGitStatus, None),
            KeyBinding::new("cmd-1", SelectSideTab1, None),
            KeyBinding::new("cmd-2", SelectSideTab2, None),
            KeyBinding::new("cmd-3", SelectSideTab3, None),
            KeyBinding::new("cmd-4", SelectSideTab4, None),
            KeyBinding::new("cmd-5", SelectSideTab5, None),
            KeyBinding::new("cmd-6", SelectSideTab6, None),
            KeyBinding::new("cmd-7", SelectSideTab7, None),
            KeyBinding::new("cmd-8", SelectSideTab8, None),
            KeyBinding::new("cmd-9", SelectSideTab9, None),
            KeyBinding::new("cmd-a", SelectAll, None),
            KeyBinding::new("cmd-c", Copy, None),
            KeyBinding::new("cmd-v", Paste, None),
        ]);

        let config = storage::load_config().unwrap_or_default();

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
