use gpui::{Context, MouseButton, MouseMoveEvent, MouseUpEvent, Window};

use crate::state::{
    ResizingSidebar, MAX_SIDEBAR_WIDTH, MIN_SIDEBAR_WIDTH,
};

use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn on_toggle_left_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.left_sidebar_collapsed = !self.state.left_sidebar_collapsed;
        cx.notify();
    }

    pub(crate) fn on_toggle_right_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.right_sidebar_collapsed = !self.state.right_sidebar_collapsed;
        cx.notify();
    }

    pub(crate) fn on_resize_drag(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(side) = self.state.resizing_sidebar else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.state.resizing_sidebar = None;
            self.state.config.left_sidebar_width = Some(self.state.left_sidebar_width);
            self.state.config.right_sidebar_width = Some(self.state.right_sidebar_width);
            self.persist_config();
            cx.notify();
            return;
        }
        let x = f32::from(event.position.x);
        match side {
            ResizingSidebar::Left => {
                let w = x.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
                self.state.left_sidebar_width = w;
            }
            ResizingSidebar::Right => {
                let window_width = f32::from(window.viewport_size().width);
                let w =
                    (window_width - x).clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
                self.state.right_sidebar_width = w;
            }
        }
        cx.notify();
    }

    pub(crate) fn on_resize_end(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.state.resizing_sidebar.is_some() {
            self.state.resizing_sidebar = None;
            self.state.config.left_sidebar_width = Some(self.state.left_sidebar_width);
            self.state.config.right_sidebar_width = Some(self.state.right_sidebar_width);
            self.persist_config();
            cx.notify();
        }
    }
}
