mod clipboard;
pub(crate) mod drawing;
mod element;
pub(crate) mod helpers;
mod input;
mod mouse;
pub(crate) mod url;
mod viewport;

#[cfg(test)]
mod tests;

use super::{StyleRun, TerminalSession};
use gpui::{
    App, Bounds, Context, FocusHandle, Focusable, IntoElement, KeyBinding, KeyContext, MouseButton,
    Pixels, Render, SharedString, Window, actions, div, prelude::*, relative,
};
use std::ops::Range;
use std::sync::Once;

use drawing::hsla_from_rgb;
use element::TerminalTextElement;

actions!(
    terminal_view,
    [
        Copy,
        Paste,
        SelectAll,
        Tab,
        TabPrev,
        EscapeKey,
        Clear,
        ScrollLineUp,
        ScrollLineDown,
        ScrollPageUp,
        ScrollPageDown,
        ScrollToTop,
        ScrollToBottom,
    ]
);

const KEY_CONTEXT: &str = "Terminal";
static KEY_BINDINGS: Once = Once::new();

fn ensure_key_bindings(cx: &mut App) {
    KEY_BINDINGS.call_once(|| {
        cx.bind_keys([
            KeyBinding::new("escape", EscapeKey, Some(KEY_CONTEXT)),
            KeyBinding::new("tab", Tab, Some(KEY_CONTEXT)),
            KeyBinding::new("shift-tab", TabPrev, Some(KEY_CONTEXT)),
        ]);
    });
}

type TerminalSendFn = dyn Fn(&[u8]) + Send + Sync + 'static;

pub struct TerminalInput {
    send: Box<TerminalSendFn>,
}

impl TerminalInput {
    pub fn new(send: impl Fn(&[u8]) + Send + Sync + 'static) -> Self {
        Self {
            send: Box::new(send),
        }
    }

    pub fn send(&self, bytes: &[u8]) {
        (self.send)(bytes);
    }
}

type ResizeCallback = Box<dyn Fn(u16, u16) + 'static>;

pub struct TerminalView {
    pub(crate) session: TerminalSession,
    pub(crate) viewport_lines: Vec<String>,
    pub(crate) viewport_line_offsets: Vec<usize>,
    pub(crate) viewport_total_len: usize,
    pub(crate) viewport_style_runs: Vec<Vec<StyleRun>>,
    pub(crate) line_layouts: Vec<Option<gpui::ShapedLine>>,
    pub(crate) line_layout_key: Option<(Pixels, Pixels)>,
    pub(crate) last_bounds: Option<Bounds<Pixels>>,
    pub(crate) last_cell_metrics: Option<(f32, f32)>,
    pub(crate) resize_callback: Option<ResizeCallback>,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) last_window_title: Option<String>,
    pub(crate) input: Option<TerminalInput>,
    pub(crate) pending_output: Vec<u8>,
    pub(crate) pending_refresh: bool,
    pub(crate) selection: Option<ByteSelection>,
    pub(crate) marked_text: Option<SharedString>,
    pub(crate) marked_selected_range_utf16: Range<usize>,
    pub(crate) font: gpui::Font,
    pub(crate) was_focused: bool,
    pub(crate) scroll_px: f32,
    pub(crate) last_mouse_click: Option<(std::time::Instant, gpui::Point<Pixels>, u8)>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ByteSelection {
    pub(crate) anchor: usize,
    pub(crate) active: usize,
}

impl ByteSelection {
    pub(crate) fn range(self) -> Range<usize> {
        if self.anchor <= self.active {
            self.anchor..self.active
        } else {
            self.active..self.anchor
        }
    }
}

impl TerminalView {
    pub fn new_with_input(
        session: TerminalSession,
        focus_handle: FocusHandle,
        input: TerminalInput,
    ) -> Self {
        Self {
            session,
            viewport_lines: Vec::new(),
            viewport_line_offsets: Vec::new(),
            viewport_total_len: 0,
            viewport_style_runs: Vec::new(),
            line_layouts: Vec::new(),
            line_layout_key: None,
            last_bounds: None,
            last_cell_metrics: None,
            resize_callback: None,
            focus_handle,
            last_window_title: None,
            input: Some(input),
            pending_output: Vec::new(),
            pending_refresh: false,
            selection: None,
            marked_text: None,
            marked_selected_range_utf16: 0..0,
            font: super::default_terminal_font(),
            was_focused: false,
            scroll_px: 0.0,
            last_mouse_click: None,
        }
        .with_refreshed_viewport()
    }

    pub fn with_resize_callback(mut self, callback: impl Fn(u16, u16) + 'static) -> Self {
        self.resize_callback = Some(Box::new(callback));
        self
    }

    fn with_refreshed_viewport(mut self) -> Self {
        self.refresh_viewport();
        self
    }

    fn dispatch_context(&self) -> KeyContext {
        let mut context = KeyContext::new_with_defaults();
        context.add(KEY_CONTEXT);

        if self.session.alt_screen_active() {
            context.set("screen", "alt");
        } else {
            context.set("screen", "normal");
        }

        if self.session.mouse_reporting_enabled() {
            context.add("any_mouse_reporting");
        }

        if self.session.bracketed_paste_enabled() {
            context.add("bracketed_paste");
        }

        if self.session.focus_events_enabled() {
            context.add("report_focus");
        }

        if self.session.alternate_scroll_enabled() {
            context.add("alternate_scroll");
        }

        if self.selection.is_some() {
            context.add("selection");
        }

        context
    }

    fn scroll_by_lines(&mut self, delta: i32, cx: &mut Context<Self>) {
        let _ = self.session.scroll_viewport(delta);
        self.sync_viewport_scroll_tracking();
        self.apply_side_effects(cx);
        self.schedule_viewport_refresh(cx);
    }

    fn scroll_to_bottom_if_needed(&mut self, cx: &mut Context<Self>) {
        if self.session.display_offset() > 0 {
            let _ = self.session.scroll_viewport_bottom();
            self.sync_viewport_scroll_tracking();
            self.apply_side_effects(cx);
            self.schedule_viewport_refresh(cx);
        }
    }

    pub(crate) fn on_clear(
        &mut self,
        _: &Clear,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(input) = self.input.as_ref() {
            // Send form feed (clear) and then redraw prompt
            input.send(b"\x0c");
        }
        self.scroll_px = 0.0;
        let _ = self.session.scroll_viewport_bottom();
        self.sync_viewport_scroll_tracking();
        self.schedule_viewport_refresh(cx);
    }

    pub(crate) fn on_scroll_line_up(
        &mut self,
        _: &ScrollLineUp,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.scroll_by_lines(-1, cx);
    }

    pub(crate) fn on_scroll_line_down(
        &mut self,
        _: &ScrollLineDown,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.scroll_by_lines(1, cx);
    }

    pub(crate) fn on_scroll_page_up(
        &mut self,
        _: &ScrollPageUp,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let step = (self.session.rows() as i32 / 2).max(1);
        self.scroll_by_lines(-step, cx);
    }

    pub(crate) fn on_scroll_page_down(
        &mut self,
        _: &ScrollPageDown,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let step = (self.session.rows() as i32 / 2).max(1);
        self.scroll_by_lines(step, cx);
    }

    pub(crate) fn on_scroll_to_top(
        &mut self,
        _: &ScrollToTop,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.session.scroll_viewport_top();
        self.sync_viewport_scroll_tracking();
        self.apply_side_effects(cx);
        self.schedule_viewport_refresh(cx);
    }

    pub(crate) fn on_scroll_to_bottom(
        &mut self,
        _: &ScrollToBottom,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let _ = self.session.scroll_viewport_bottom();
        self.sync_viewport_scroll_tracking();
        self.apply_side_effects(cx);
        self.schedule_viewport_refresh(cx);
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        ensure_key_bindings(cx);

        if !self.pending_output.is_empty() {
            let bytes = std::mem::take(&mut self.pending_output);
            self.feed_output_bytes_to_session(&bytes);
            self.apply_side_effects(cx);
            self.reconcile_dirty_viewport_after_output();
        }

        if self.pending_refresh {
            self.refresh_viewport();
            self.pending_refresh = false;
        }

        let is_focused = self.focus_handle.is_focused(window);
        if is_focused != self.was_focused {
            self.was_focused = is_focused;
            if self.session.focus_events_enabled() {
                if let Some(input) = self.input.as_ref() {
                    if is_focused {
                        input.send(b"\x1b[I");
                    } else {
                        input.send(b"\x1b[O");
                    }
                }
            }
        }

        if self.session.window_title_updates_enabled() {
            let title = self
                .session
                .title()
                .unwrap_or("Terminal");

            if self.last_window_title.as_deref() != Some(title) {
                window.set_window_title(title);
                self.last_window_title = Some(title.to_string());
            }
        }

        let key_context = self.dispatch_context();

        div()
            .size_full()
            .flex()
            .track_focus(&self.focus_handle)
            .key_context(key_context)
            .on_action(cx.listener(Self::on_copy))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_paste))
            .on_action(cx.listener(Self::on_tab))
            .on_action(cx.listener(Self::on_tab_prev))
            .on_action(cx.listener(Self::on_escape))
            .on_action(cx.listener(Self::on_clear))
            .on_action(cx.listener(Self::on_scroll_line_up))
            .on_action(cx.listener(Self::on_scroll_line_down))
            .on_action(cx.listener(Self::on_scroll_page_up))
            .on_action(cx.listener(Self::on_scroll_page_down))
            .on_action(cx.listener(Self::on_scroll_to_top))
            .on_action(cx.listener(Self::on_scroll_to_bottom))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_scroll_wheel(cx.listener(Self::on_scroll_wheel))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::on_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::on_mouse_up))
            .on_drop(cx.listener(Self::on_file_drop))
            .bg(hsla_from_rgb(self.session.default_background()))
            .text_color(hsla_from_rgb(self.session.default_foreground()))
            .font(self.font.clone())
            .text_sm()
            .line_height(relative(1.0))
            .whitespace_nowrap()
            .child(TerminalTextElement { view: cx.entity() })
    }
}
