use gpui::{
    ClipboardItem, Context, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ScrollDelta, ScrollWheelEvent, Window,
};

use super::drawing::cell_metrics;
use super::helpers::{
    byte_index_for_column_in_line, sgr_mouse_button_value, sgr_mouse_sequence,
    window_position_to_local,
};
use super::url::url_at_column_in_line;
use super::ByteSelection;
use super::TerminalView;

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

        if event.button == MouseButton::Left && event.modifiers.platform {
            if let Some((col, row)) = self.mouse_position_to_cell(event.position, window) {
                if let Some(link) = self.session.hyperlink_at(col, row) {
                    let item = ClipboardItem::new_string(link);
                    cx.write_to_clipboard(item.clone());
                    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                    cx.write_to_primary(item);
                    return;
                }

                if let Some(line) = self.viewport_lines.get(row.saturating_sub(1) as usize)
                    && let Some(url) = url_at_column_in_line(line, col)
                {
                    let item = ClipboardItem::new_string(url);
                    cx.write_to_clipboard(item.clone());
                    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                    cx.write_to_primary(item);
                    return;
                }
            }

            if let Some(index) = self.mouse_position_to_viewport_index(event.position, window)
                && let Some(url) = self.url_at_viewport_index(index)
            {
                let item = ClipboardItem::new_string(url);
                cx.write_to_clipboard(item.clone());
                #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                cx.write_to_primary(item);
                return;
            }
        }

        if event.modifiers.shift
            || self.input.is_none()
            || !self.session.mouse_reporting_enabled()
            || !self.session.mouse_sgr_enabled()
        {
            if event.button == MouseButton::Left
                && let Some(index) = self.mouse_position_to_viewport_index(event.position, window)
            {
                self.selection = Some(ByteSelection {
                    anchor: index,
                    active: index,
                });
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
            let seq = sgr_mouse_sequence(button_value, col, row, true);
            input.send(seq.as_bytes());
        }
    }

    pub(crate) fn on_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.modifiers.shift
            || self.input.is_none()
            || !self.session.mouse_reporting_enabled()
            || !self.session.mouse_sgr_enabled()
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
            let seq = sgr_mouse_sequence(button_value, col, row, false);
            input.send(seq.as_bytes());
        }
    }

    pub(crate) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.modifiers.shift
            && self.input.is_some()
            && self.session.mouse_reporting_enabled()
            && self.session.mouse_sgr_enabled()
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
                    let seq = sgr_mouse_sequence(button_value, col, row, true);
                    input.send(seq.as_bytes());
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
        let dy_lines: f32 = match event.delta {
            ScrollDelta::Lines(p) => p.y,
            ScrollDelta::Pixels(p) => f32::from(p.y) / 16.0,
        };

        let delta_lines = (-dy_lines).round() as i32;
        if delta_lines == 0 {
            return;
        }

        if let Some(input) = self.input.as_ref()
            && !event.modifiers.shift
            && self.session.mouse_reporting_enabled()
            && self.session.mouse_sgr_enabled()
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
                let seq = sgr_mouse_sequence(button_value, col, row, true);
                input.send(seq.as_bytes());
            }
            return;
        }

        let _ = self.session.scroll_viewport(delta_lines);
        self.sync_viewport_scroll_tracking();
        self.apply_side_effects(cx);
        self.schedule_viewport_refresh(cx);
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
