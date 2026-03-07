use std::time::Instant;

use gpui::{
    Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ScrollWheelEvent,
    TouchPhase, Window,
};

use super::ByteSelection;
use super::TerminalView;
use super::drawing::cell_metrics;
use super::helpers::{
    byte_index_for_column_in_line, normal_mouse_sequence, sgr_mouse_button_value,
    sgr_mouse_sequence, window_position_to_local,
};
use super::url::url_at_column_in_line;

const DOUBLE_CLICK_INTERVAL_MS: u128 = 400;
const DOUBLE_CLICK_DISTANCE_PX: f32 = 5.0;

impl TerminalView {
    pub(crate) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window, cx);

        if event.first_mouse {
            return;
        }

        // Cmd+click: open URLs in browser (not just copy)
        if event.button == MouseButton::Left && event.modifiers.platform {
            if let Some((col, row)) = self.mouse_position_to_cell(event.position, window) {
                if let Some(link) = self.session.hyperlink_at(col, row) {
                    cx.open_url(&link);
                    return;
                }

                if let Some(line) = self.viewport_lines.get(row.saturating_sub(1) as usize)
                    && let Some(url) = url_at_column_in_line(line, col)
                {
                    cx.open_url(&url);
                    return;
                }
            }

            if let Some(index) = self.mouse_position_to_viewport_index(event.position, window)
                && let Some(url) = self.url_at_viewport_index(index)
            {
                cx.open_url(&url);
                return;
            }
        }

        // Selection mode (shift held, no input, or no mouse reporting)
        if event.modifiers.shift || self.input.is_none() || !self.session.mouse_reporting_enabled()
        {
            if event.button == MouseButton::Left {
                let click_count = self.click_count_for_event(event);

                if let Some(index) = self.mouse_position_to_viewport_index(event.position, window) {
                    match click_count {
                        2 => {
                            // Double-click: select word
                            if let Some(sel) = self.select_word_at_index(index) {
                                self.selection = Some(sel);
                            }
                        }
                        3 => {
                            // Triple-click: select line
                            if let Some(sel) = self.select_line_at_index(index) {
                                self.selection = Some(sel);
                            }
                        }
                        _ => {
                            // Single click: start new selection
                            self.selection = Some(ByteSelection {
                                anchor: index,
                                active: index,
                            });
                        }
                    }
                    cx.notify();
                }
            }
            return;
        }

        // Forward mouse events to the terminal application
        let Some((col, row)) = self.mouse_position_to_cell(event.position, window) else {
            return;
        };

        if let Some(input) = self.input.as_ref() {
            let base_button = match event.button {
                MouseButton::Left => 0,
                MouseButton::Middle => 1,
                MouseButton::Right => 2,
                _ => return,
            };

            let button_value = sgr_mouse_button_value(
                base_button,
                false,
                false,
                event.modifiers.alt,
                event.modifiers.control,
            );
            if self.session.mouse_sgr_enabled() {
                let seq = sgr_mouse_sequence(button_value, col, row, true);
                input.send(seq.as_bytes());
            } else {
                let seq = normal_mouse_sequence(button_value, col, row);
                input.send(&seq);
            }
        }
    }

    pub(crate) fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.modifiers.shift || self.input.is_none() || !self.session.mouse_reporting_enabled()
        {
            if let Some(selection) = self.selection {
                if selection.range().is_empty() {
                    self.selection = None;
                }
                cx.notify();
            }
            return;
        }

        let Some((col, row)) = self.mouse_position_to_cell(event.position, window) else {
            return;
        };

        if let Some(input) = self.input.as_ref() {
            let base_button = match event.button {
                MouseButton::Left => 0,
                MouseButton::Middle => 1,
                MouseButton::Right => 2,
                _ => return,
            };

            let button_value = sgr_mouse_button_value(
                base_button,
                false,
                false,
                event.modifiers.alt,
                event.modifiers.control,
            );
            if self.session.mouse_sgr_enabled() {
                let seq = sgr_mouse_sequence(button_value, col, row, false);
                input.send(seq.as_bytes());
            } else {
                // Normal encoding has no release event; send button 3 (release)
                let release_value = sgr_mouse_button_value(
                    3,
                    false,
                    false,
                    event.modifiers.alt,
                    event.modifiers.control,
                );
                let seq = normal_mouse_sequence(release_value, col, row);
                input.send(&seq);
            }
        }
    }

    pub(crate) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.modifiers.shift && self.input.is_some() && self.session.mouse_reporting_enabled()
        {
            let send_motion = if self.session.mouse_any_event_enabled() {
                true
            } else if self.session.mouse_button_event_enabled() {
                event.pressed_button.is_some()
            } else {
                false
            };

            if send_motion {
                let Some((col, row)) = self.mouse_position_to_cell(event.position, window) else {
                    return;
                };

                let base_button = match event.pressed_button {
                    Some(MouseButton::Left) => 0,
                    Some(MouseButton::Middle) => 1,
                    Some(MouseButton::Right) => 2,
                    Some(_) => 3,
                    None => 3,
                };

                let button_value = sgr_mouse_button_value(
                    base_button,
                    true,
                    false,
                    event.modifiers.alt,
                    event.modifiers.control,
                );
                if let Some(input) = self.input.as_ref() {
                    if self.session.mouse_sgr_enabled() {
                        let seq = sgr_mouse_sequence(button_value, col, row, true);
                        input.send(seq.as_bytes());
                    } else {
                        let seq = normal_mouse_sequence(button_value, col, row);
                        input.send(&seq);
                    }
                }
                return;
            }
        }

        if !event.dragging() {
            return;
        }

        if self.selection.is_none() {
            return;
        }

        let Some(index) = self.mouse_position_to_viewport_index(event.position, window) else {
            return;
        };

        if let Some(selection) = self.selection.as_mut()
            && selection.active != index
        {
            selection.active = index;
            cx.notify();
        }
    }

    pub(crate) fn on_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Accumulate sub-pixel scroll for smooth scrolling (like Zed)
        let line_height = self.last_cell_metrics.map(|(_, h)| h).unwrap_or(16.0);

        let delta_lines = match event.touch_phase {
            TouchPhase::Started => {
                self.scroll_px = 0.0;
                return;
            }
            TouchPhase::Moved => {
                let old_offset = (self.scroll_px / line_height) as i32;
                let pixel_delta_y = f32::from(event.delta.pixel_delta(gpui::px(line_height)).y);
                self.scroll_px += pixel_delta_y;
                let new_offset = (self.scroll_px / line_height) as i32;
                let delta = new_offset - old_offset;
                if delta == 0 {
                    return;
                }
                delta
            }
            TouchPhase::Ended => return,
        };

        // Mouse reporting mode: forward scroll events to the application
        if let Some(input) = self.input.as_ref()
            && !event.modifiers.shift
            && self.session.mouse_reporting_enabled()
        {
            let Some((col, row)) = self.mouse_position_to_cell(event.position, window) else {
                return;
            };

            let button = if delta_lines < 0 { 64 } else { 65 };
            let button_value = sgr_mouse_button_value(
                button,
                false,
                false,
                event.modifiers.alt,
                event.modifiers.control,
            );
            let steps = delta_lines.unsigned_abs().min(10);
            for _ in 0..steps {
                if self.session.mouse_sgr_enabled() {
                    let seq = sgr_mouse_sequence(button_value, col, row, true);
                    input.send(seq.as_bytes());
                } else {
                    let seq = normal_mouse_sequence(button_value, col, row);
                    input.send(&seq);
                }
            }
            return;
        }

        // Alt screen + alternate scroll mode: convert scroll to arrow keys
        // This is what makes scrolling work in vim, less, man, etc.
        if self.session.alt_screen_active()
            && let Some(input) = self.input.as_ref()
        {
            let key = if delta_lines > 0 {
                b"\x1b[B" // Down arrow
            } else {
                b"\x1b[A" // Up arrow
            };
            let steps = delta_lines.unsigned_abs().min(10);
            for _ in 0..steps {
                input.send(key);
            }
            return;
        }

        // Normal mode: scroll the viewport buffer
        let _ = self.session.scroll_viewport(delta_lines);
        self.sync_viewport_scroll_tracking();
        self.apply_side_effects(cx);
        self.schedule_viewport_refresh(cx);
    }

    fn click_count_for_event(&mut self, event: &MouseDownEvent) -> u8 {
        let now = Instant::now();
        if let Some((last_time, last_pos, last_count)) = self.last_mouse_click {
            let elapsed = now.duration_since(last_time).as_millis();
            let dx = f32::from(event.position.x - last_pos.x).abs();
            let dy = f32::from(event.position.y - last_pos.y).abs();
            if elapsed < DOUBLE_CLICK_INTERVAL_MS
                && dx < DOUBLE_CLICK_DISTANCE_PX
                && dy < DOUBLE_CLICK_DISTANCE_PX
            {
                let count = if last_count >= 3 { 1 } else { last_count + 1 };
                self.last_mouse_click = Some((now, event.position, count));
                return count;
            }
        }
        self.last_mouse_click = Some((now, event.position, 1));
        1
    }

    pub(crate) fn select_word_at_index(&mut self, index: usize) -> Option<ByteSelection> {
        let (_row, line, line_offset) = self.viewport_row_for_index(index)?;
        let local = index.saturating_sub(line_offset);
        let bytes = line.as_bytes();

        if local >= bytes.len() {
            return None;
        }

        let is_word_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.';
        let target_is_word = is_word_char(bytes[local]);

        let mut start = local;
        let mut end = local;

        if target_is_word {
            while start > 0 && is_word_char(bytes[start - 1]) {
                start -= 1;
            }
            while end < bytes.len() && is_word_char(bytes[end]) {
                end += 1;
            }
        } else if bytes[local] == b' ' {
            while start > 0 && bytes[start - 1] == b' ' {
                start -= 1;
            }
            while end < bytes.len() && bytes[end] == b' ' {
                end += 1;
            }
        } else {
            end += 1;
        }

        Some(ByteSelection {
            anchor: line_offset + start,
            active: line_offset + end,
        })
    }

    pub(crate) fn select_line_at_index(&mut self, index: usize) -> Option<ByteSelection> {
        let (_row, line, line_offset) = self.viewport_row_for_index(index)?;
        Some(ByteSelection {
            anchor: line_offset,
            active: line_offset + line.len(),
        })
    }

    pub(crate) fn viewport_row_for_index(&self, index: usize) -> Option<(usize, &str, usize)> {
        let row = self
            .viewport_line_offsets
            .iter()
            .enumerate()
            .rfind(|(_, offset)| **offset <= index)
            .map(|(i, _)| i)?;
        let line = self.viewport_lines.get(row)?.as_str();
        let offset = *self.viewport_line_offsets.get(row).unwrap_or(&0);
        Some((row, line, offset))
    }

    pub(crate) fn mouse_position_to_viewport_index(
        &self,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
    ) -> Option<usize> {
        let rows = self.session.rows() as usize;
        if rows == 0 {
            return None;
        }

        let local = self.mouse_position_to_local(position);
        let (_, cell_height) = self
            .last_cell_metrics
            .or_else(|| cell_metrics(window, &self.font))?;
        let y = f32::from(local.y);
        let mut row_index = (y / cell_height).floor() as i32;
        if row_index < 0 {
            row_index = 0;
        }
        if row_index >= rows as i32 {
            row_index = rows as i32 - 1;
        }
        let row_index = row_index as usize;

        if let Some(Some(line)) = self.line_layouts.get(row_index) {
            let byte_index = line
                .closest_index_for_x(gpui::px(f32::from(local.x)))
                .min(line.text.len());
            let offset = *self.viewport_line_offsets.get(row_index).unwrap_or(&0);
            return Some(offset.saturating_add(byte_index));
        }

        let (col, row) = self.mouse_position_to_cell(position, window)?;
        let row_index = row.saturating_sub(1) as usize;
        let line = self.viewport_lines.get(row_index)?.as_str();
        let byte_index = byte_index_for_column_in_line(line, col).min(line.len());
        let offset = *self.viewport_line_offsets.get(row_index).unwrap_or(&0);
        Some(offset.saturating_add(byte_index))
    }

    pub(crate) fn mouse_position_to_cell(
        &self,
        position: gpui::Point<gpui::Pixels>,
        window: &mut Window,
    ) -> Option<(u16, u16)> {
        let cols = self.session.cols();
        let rows = self.session.rows();

        let position = self.mouse_position_to_local(position);
        let (cell_width, cell_height) = self
            .last_cell_metrics
            .or_else(|| cell_metrics(window, &self.font))?;
        let x = f32::from(position.x);
        let y = f32::from(position.y);

        let mut col = (x / cell_width).floor() as i32 + 1;
        let mut row = (y / cell_height).floor() as i32 + 1;

        if col < 1 {
            col = 1;
        }
        if row < 1 {
            row = 1;
        }
        if col > cols as i32 {
            col = cols as i32;
        }
        if row > rows as i32 {
            row = rows as i32;
        }

        Some((col as u16, row as u16))
    }

    fn mouse_position_to_local(
        &self,
        position: gpui::Point<gpui::Pixels>,
    ) -> gpui::Point<gpui::Pixels> {
        window_position_to_local(self.last_bounds, position)
    }
}
