use std::ops::Range;

use gpui::{
    Bounds, Context, EntityInputHandler, KeyDownEvent, Pixels, SharedString, UTF16Selection,
    Window,
};

use super::helpers::{ctrl_byte_for_keystroke, encode_key_named, should_skip_key_down_for_ime};
use super::{EscapeKey, Tab, TabPrev, TerminalView};

impl TerminalView {
    pub(crate) fn on_escape(
        &mut self,
        _: &EscapeKey,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_input_parts(&[&[0x1b]], cx);
    }

    pub(crate) fn on_tab(&mut self, _: &Tab, _window: &mut Window, cx: &mut Context<Self>) {
        self.send_tab(false, cx);
    }

    pub(crate) fn on_tab_prev(
        &mut self,
        _: &TabPrev,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.send_tab(true, cx);
    }

    fn send_tab(&mut self, reverse: bool, cx: &mut Context<Self>) {
        if reverse {
            self.send_input_parts(&[b"\x1b[Z"], cx);
        } else {
            self.send_input_parts(&[b"\t"], cx);
        }
    }

    pub(crate) fn utf16_len(s: &str) -> usize {
        s.chars().map(|ch| ch.len_utf16()).sum()
    }

    pub(crate) fn utf16_range_to_utf8(
        s: &str,
        range_utf16: Range<usize>,
    ) -> Option<Range<usize>> {
        let mut utf16_count = 0usize;
        let mut start_utf8: Option<usize> = None;
        let mut end_utf8: Option<usize> = None;

        if range_utf16.start == 0 {
            start_utf8 = Some(0);
        }
        if range_utf16.end == 0 {
            end_utf8 = Some(0);
        }

        for (utf8_index, ch) in s.char_indices() {
            if start_utf8.is_none() && utf16_count >= range_utf16.start {
                start_utf8 = Some(utf8_index);
            }
            if end_utf8.is_none() && utf16_count >= range_utf16.end {
                end_utf8 = Some(utf8_index);
            }

            utf16_count = utf16_count.saturating_add(ch.len_utf16());
        }

        if start_utf8.is_none() && utf16_count >= range_utf16.start {
            start_utf8 = Some(s.len());
        }
        if end_utf8.is_none() && utf16_count >= range_utf16.end {
            end_utf8 = Some(s.len());
        }

        Some(start_utf8?..end_utf8?)
    }

    pub(crate) fn cell_offset_for_utf16(text: &str, utf16_offset: usize) -> usize {
        use unicode_width::UnicodeWidthChar as _;

        let mut cells = 0usize;
        let mut utf16_count = 0usize;
        for ch in text.chars() {
            if utf16_count >= utf16_offset {
                break;
            }

            let len_utf16 = ch.len_utf16();
            if utf16_count.saturating_add(len_utf16) > utf16_offset {
                break;
            }
            utf16_count = utf16_count.saturating_add(len_utf16);

            let width = ch.width().unwrap_or(0);
            if width > 0 {
                cells = cells.saturating_add(width);
            }
        }
        cells
    }

    pub(crate) fn clear_marked_text(&mut self, cx: &mut Context<Self>) {
        self.marked_text = None;
        self.marked_selected_range_utf16 = 0..0;
        cx.notify();
    }

    pub(crate) fn set_marked_text(
        &mut self,
        text: String,
        selected_range_utf16: Option<Range<usize>>,
        cx: &mut Context<Self>,
    ) {
        if text.is_empty() {
            self.clear_marked_text(cx);
            return;
        }

        let total_utf16 = Self::utf16_len(&text);
        let selected = selected_range_utf16.unwrap_or(total_utf16..total_utf16);
        let selected = selected.start.min(total_utf16)..selected.end.min(total_utf16);

        self.marked_text = Some(SharedString::from(text));
        self.marked_selected_range_utf16 = selected;
        cx.notify();
    }

    pub(crate) fn commit_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if text.is_empty() {
            return;
        }

        self.send_input_parts(&[text.as_bytes()], cx);
    }

    pub(crate) fn send_input_parts(&mut self, parts: &[&[u8]], cx: &mut Context<Self>) {
        if parts.is_empty() {
            return;
        }

        // Auto-scroll to bottom when sending input (like Zed)
        self.scroll_to_bottom_if_needed(cx);

        if let Some(input) = self.input.as_ref() {
            for bytes in parts {
                input.send(bytes);
            }
            return;
        }

        for bytes in parts {
            let _ = self.session.feed(bytes);
        }
        self.apply_side_effects(cx);
        self.schedule_viewport_refresh(cx);
    }

    pub(crate) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let raw_keystroke = event.keystroke.clone();
        if should_skip_key_down_for_ime(self.input.is_some(), &raw_keystroke) {
            return;
        }
        let keystroke = raw_keystroke.clone().with_simulated_ime();

        if keystroke.modifiers.platform || keystroke.modifiers.function {
            return;
        }

        // Auto-scroll to bottom on any keypress (like Zed)
        self.scroll_to_bottom_if_needed(cx);

        let scroll_step = (self.session.rows() as i32 / 2).max(1);

        if let Some(input) = self.input.as_ref() {
            if keystroke.modifiers.shift {
                match keystroke.key.as_str() {
                    "home" => {
                        let _ = self.session.scroll_viewport_top();
                        self.sync_viewport_scroll_tracking();
                        self.apply_side_effects(cx);
                        self.schedule_viewport_refresh(cx);
                        return;
                    }
                    "end" => {
                        let _ = self.session.scroll_viewport_bottom();
                        self.sync_viewport_scroll_tracking();
                        self.apply_side_effects(cx);
                        self.schedule_viewport_refresh(cx);
                        return;
                    }
                    "pageup" | "page_up" | "page-up" => {
                        let _ = self.session.scroll_viewport(-scroll_step);
                        self.sync_viewport_scroll_tracking();
                        self.apply_side_effects(cx);
                        self.schedule_viewport_refresh(cx);
                        return;
                    }
                    "pagedown" | "page_down" | "page-down" => {
                        let _ = self.session.scroll_viewport(scroll_step);
                        self.sync_viewport_scroll_tracking();
                        self.apply_side_effects(cx);
                        self.schedule_viewport_refresh(cx);
                        return;
                    }
                    _ => {}
                }
            }

            if keystroke.modifiers.control
                && let Some(b) = ctrl_byte_for_keystroke(&keystroke)
            {
                input.send(&[b]);
                return;
            }

            if keystroke.modifiers.alt
                && let Some(text) = raw_keystroke.key_char.as_deref()
            {
                input.send(&[0x1b]);
                input.send(text.as_bytes());
                return;
            }

            if let Some(encoded) = encode_key_named(
                &keystroke.key,
                keystroke.modifiers.shift,
                keystroke.modifiers.control,
                keystroke.modifiers.alt,
            ) {
                input.send(&encoded);
                return;
            }
            return;
        }

        match keystroke.key.as_str() {
            "home" => {
                let _ = self.session.scroll_viewport_top();
                self.sync_viewport_scroll_tracking();
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
                return;
            }
            "end" => {
                let _ = self.session.scroll_viewport_bottom();
                self.sync_viewport_scroll_tracking();
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
                return;
            }
            "pageup" | "page_up" | "page-up" => {
                let _ = self.session.scroll_viewport(-scroll_step);
                self.sync_viewport_scroll_tracking();
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
                return;
            }
            "pagedown" | "page_down" | "page-down" => {
                let _ = self.session.scroll_viewport(scroll_step);
                self.sync_viewport_scroll_tracking();
                self.apply_side_effects(cx);
                self.schedule_viewport_refresh(cx);
                return;
            }
            _ => {}
        }

        if let Some(encoded) = encode_key_named(
            &keystroke.key,
            keystroke.modifiers.shift,
            keystroke.modifiers.control,
            keystroke.modifiers.alt,
        ) {
            let _ = self.session.feed(&encoded);
            self.apply_side_effects(cx);
            self.schedule_viewport_refresh(cx);
            return;
        }

        if keystroke.key == "backspace" {
            if let Some(input) = self.input.as_ref() {
                input.send(&[0x7f]);
                return;
            }
            let _ = self.session.feed(&[0x08]);
            self.apply_side_effects(cx);
            self.schedule_viewport_refresh(cx);
        }
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.marked_text.as_ref()?.as_str();
        let total_utf16 = Self::utf16_len(text);
        let start = range_utf16.start.min(total_utf16);
        let end = range_utf16.end.min(total_utf16);
        let range_utf16 = start..end;
        *adjusted_range = Some(range_utf16.clone());

        let range_utf8 = Self::utf16_range_to_utf8(text, range_utf16)?;
        Some(text.get(range_utf8)?.to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.marked_selected_range_utf16.clone(),
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let text = self.marked_text.as_ref()?.as_str();
        let len = Self::utf16_len(text);
        (len > 0).then_some(0..len)
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.clear_marked_text(cx);
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.clear_marked_text(cx);
        self.commit_text(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_marked_text(new_text.to_string(), new_selected_range, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (col, row) = self.session.cursor_position()?;
        let (cell_width, cell_height) = super::drawing::cell_metrics(window, &self.font)?;

        let base_x = element_bounds.left() + gpui::px(cell_width * (col.saturating_sub(1)) as f32);
        let base_y =
            element_bounds.top() + gpui::px(cell_height * (row.saturating_sub(1)) as f32);

        let offset_cells = self
            .marked_text
            .as_ref()
            .map(|text| Self::cell_offset_for_utf16(text.as_str(), range_utf16.start))
            .unwrap_or(range_utf16.start);
        let x = base_x + gpui::px(cell_width * offset_cells as f32);
        Some(Bounds::new(
            gpui::point(x, base_y),
            gpui::size(gpui::px(cell_width), gpui::px(cell_height)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        _point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
