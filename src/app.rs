use gpui::{App, AppContext, KeyBinding, TitlebarOptions, WindowOptions, point, px};

use crate::assets::Assets;
use terminal::view::{Copy, Paste, SelectAll};
use ui::text_input;

pub fn run() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            cx.bind_keys(text_input::key_bindings());

            // Load custom keybindings from file, falling back to defaults
            let entries = workspace::storage::load_keybindings()
                .unwrap_or_else(|_| workspace::keybindings::default_keybindings());
            let mut bindings = workspace::keybindings::to_gpui_keybindings(&entries);

            // Standard clipboard bindings (not customizable)
            bindings.push(KeyBinding::new("cmd-a", SelectAll, None));
            bindings.push(KeyBinding::new("cmd-c", Copy, None));
            bindings.push(KeyBinding::new("cmd-v", Paste, None));

            cx.bind_keys(bindings);

            // Global action handlers for standard macOS behaviors
            cx.on_action(|_: &workspace::Quit, cx| cx.quit());
            cx.on_action(|_: &workspace::HideApp, cx| cx.hide());
            cx.on_action(|_: &workspace::HideOtherApps, cx| cx.hide_other_apps());

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
                cx.new(|cx| workspace::WorkspaceView::new(config, window, cx))
            })
            .expect("failed to open the main window");

            cx.activate(true);
        });
}
