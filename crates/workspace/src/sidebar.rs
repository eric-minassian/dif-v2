use gpui::{MouseButton, MouseMoveEvent, MouseUpEvent};

use crate::config::{
    MAX_BOTTOM_PANEL_HEIGHT, MAX_SIDEBAR_WIDTH, MIN_BOTTOM_PANEL_HEIGHT, MIN_SIDEBAR_WIDTH,
};
use crate::ui_state::ResizingSidebar;
use ui::prelude::*;

use crate::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn on_toggle_left_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.left_sidebar_collapsed = !self.state.left_sidebar_collapsed;
        cx.notify();
    }

    pub(crate) fn on_toggle_right_sidebar(&mut self, cx: &mut Context<Self>) {
        self.state.right_sidebar_collapsed = !self.state.right_sidebar_collapsed;
        cx.notify();
    }

    pub(crate) fn on_toggle_bottom_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui::Focusable;

        self.state.bottom_panel_collapsed = !self.state.bottom_panel_collapsed;

        if !self.state.bottom_panel_collapsed {
            // Opening: focus the active terminal in the active tab
            let side_handle = self
                .selected_session_runtime()
                .and_then(|rt| rt.active_tab())
                .and_then(|tab| tab.active_pane.as_ref())
                .map(|pane| pane.focus_handle(cx));
            if let Some(h) = side_handle {
                h.focus(window, cx);
            }
        } else {
            // Closing: focus the main terminal
            let main_handle = self
                .selected_session_runtime()
                .and_then(|rt| rt.main_terminal.as_ref())
                .map(|main| main.focus_handle(cx));
            if let Some(h) = main_handle {
                h.focus(window, cx);
            }
        }
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
            self.state.config.bottom_panel_height = Some(self.state.bottom_panel_height);
            self.persist_config();
            cx.notify();
            return;
        }
        match side {
            ResizingSidebar::Left => {
                let x = f32::from(event.position.x);
                let w = x.clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
                self.state.left_sidebar_width = w;
            }
            ResizingSidebar::Right => {
                let x = f32::from(event.position.x);
                let window_width = f32::from(window.viewport_size().width);
                let w = (window_width - x).clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
                self.state.right_sidebar_width = w;
            }
            ResizingSidebar::Bottom => {
                let y = f32::from(event.position.y);
                let window_height = f32::from(window.viewport_size().height);
                let h = (window_height - y).clamp(MIN_BOTTOM_PANEL_HEIGHT, MAX_BOTTOM_PANEL_HEIGHT);
                self.state.bottom_panel_height = h;
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
            self.state.config.bottom_panel_height = Some(self.state.bottom_panel_height);
            self.persist_config();
            cx.notify();
        }
    }
}
