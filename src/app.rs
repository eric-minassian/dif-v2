use gpui::{App, AppContext, Application, WindowOptions};

use crate::storage;
use crate::ui::WorkspaceView;

pub fn run() {
    Application::new().run(|cx: &mut App| {
        let config = storage::load_config().unwrap_or_default();

        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.new(|cx| WorkspaceView::new(config, window, cx))
        })
        .expect("failed to open the main window");

        cx.activate(true);
    });
}
