use std::ops::Range;

use gpui::Context;

use super::helpers::split_viewport_lines;
use super::TerminalView;

impl TerminalView {
    pub(crate) fn feed_output_bytes_to_session(&mut self, bytes: &[u8]) {
        if let Some(input) = self.input.as_ref() {
            let _ = self
                .session
                .feed_with_pty_responses(bytes, |resp| input.send(resp));
        } else {
            let _ = self.session.feed(bytes);
        }
    }

    pub(crate) fn sync_viewport_scroll_tracking(&mut self) {
        let _ = self.session.take_viewport_scroll_delta();
    }

    pub(crate) fn apply_viewport_scroll_delta(&mut self, delta: i32) {
        if delta == 0 {
            return;
        }

        let rows = self.session.rows() as usize;
        if rows == 0 {
            return;
        }

        if self.viewport_lines.len() != rows || self.viewport_style_runs.len() != rows {
            self.refresh_viewport();
            return;
        }

        let delta_abs: usize = delta.unsigned_abs() as usize;
        if delta_abs == 0 {
            return;
        }
        if delta_abs >= rows {
            self.refresh_viewport();
            return;
        }

        let has_layouts = self.line_layouts.len() == rows;

        if delta > 0 {
            self.viewport_lines.rotate_left(delta_abs);
            self.viewport_style_runs.rotate_left(delta_abs);
            if has_layouts {
                self.line_layouts.rotate_left(delta_abs);
            }

            for idx in rows - delta_abs..rows {
                self.viewport_lines[idx].clear();
                self.viewport_style_runs[idx].clear();
                if has_layouts {
                    self.line_layouts[idx] = None;
                }
            }

            let dirty_rows: Vec<u16> = (rows - delta_abs..rows).map(|row| row as u16).collect();
            let _ = self.apply_dirty_viewport_rows(&dirty_rows);
            return;
        }

        self.viewport_lines.rotate_right(delta_abs);
        self.viewport_style_runs.rotate_right(delta_abs);
        if has_layouts {
            self.line_layouts.rotate_right(delta_abs);
        }

        for idx in 0..delta_abs {
            self.viewport_lines[idx].clear();
            self.viewport_style_runs[idx].clear();
            if has_layouts {
                self.line_layouts[idx] = None;
            }
        }

        let dirty_rows: Vec<u16> = (0..delta_abs).map(|row| row as u16).collect();
        let _ = self.apply_dirty_viewport_rows(&dirty_rows);
    }

    pub(crate) fn reconcile_dirty_viewport_after_output(&mut self) {
        let delta = self.session.take_viewport_scroll_delta();
        self.apply_viewport_scroll_delta(delta);

        let dirty = self.session.take_dirty_viewport_rows();
        if !dirty.is_empty() && !self.apply_dirty_viewport_rows(&dirty) {
            self.pending_refresh = true;
        }
    }

    pub(crate) fn refresh_viewport(&mut self) {
        let viewport = self.session.dump_viewport().unwrap_or_default();
        self.viewport_lines = split_viewport_lines(&viewport);
        self.viewport_line_offsets = Self::compute_viewport_line_offsets(&self.viewport_lines);
        self.viewport_total_len = Self::compute_viewport_total_len(&self.viewport_lines);
        self.viewport_style_runs = (0..self.session.rows())
            .map(|row| {
                self.session
                    .dump_viewport_row_style_runs(row)
                    .unwrap_or_default()
            })
            .collect();
        self.line_layouts.clear();
        self.line_layout_key = None;
        self.selection = None;
    }

    pub(crate) fn compute_viewport_line_offsets(lines: &[String]) -> Vec<usize> {
        let mut offsets = Vec::with_capacity(lines.len());
        let mut offset = 0usize;
        for line in lines {
            offsets.push(offset);
            offset = offset.saturating_add(line.len() + 1);
        }
        offsets
    }

    pub(crate) fn compute_viewport_total_len(lines: &[String]) -> usize {
        lines
            .iter()
            .fold(0usize, |acc, line| acc.saturating_add(line.len() + 1))
    }

    pub(crate) fn viewport_slice(&self, range: Range<usize>) -> String {
        if range.is_empty() || self.viewport_lines.is_empty() {
            return String::new();
        }

        let start = range.start.min(self.viewport_total_len);
        let end = range.end.min(self.viewport_total_len);
        if start >= end {
            return String::new();
        }

        let mut out = String::new();
        let mut i = 0usize;
        while i < self.viewport_lines.len() {
            let line_start = *self.viewport_line_offsets.get(i).unwrap_or(&0);
            let line = &self.viewport_lines[i];
            let line_end = line_start.saturating_add(line.len());
            let newline_pos = line_end;

            let seg_start = start.max(line_start);
            let seg_end = end.min(newline_pos.saturating_add(1));
            if seg_start < seg_end {
                let local_start = seg_start.saturating_sub(line_start);
                let local_end = seg_end.saturating_sub(line_start);
                let local_end = local_end.min(line.len().saturating_add(1));

                if local_start < line.len() {
                    let text_end = local_end.min(line.len());
                    if let Some(seg) = line.get(local_start..text_end) {
                        out.push_str(seg);
                    }
                }
                if local_end > line.len() {
                    out.push('\n');
                }
            }

            i += 1;
        }

        out
    }

    pub(crate) fn apply_dirty_viewport_rows(&mut self, dirty_rows: &[u16]) -> bool {
        if dirty_rows.is_empty() {
            return false;
        }

        let expected_rows = self.session.rows() as usize;
        if self.viewport_lines.len() != expected_rows {
            self.refresh_viewport();
            return true;
        }
        if self.viewport_style_runs.len() != expected_rows {
            self.refresh_viewport();
            return true;
        }

        for &row in dirty_rows {
            let row = row as usize;
            if row >= self.viewport_lines.len() {
                continue;
            }

            let line = match self.session.dump_viewport_row(row as u16) {
                Ok(s) => s,
                Err(_) => {
                    self.refresh_viewport();
                    return true;
                }
            };

            let line = line.strip_suffix('\n').unwrap_or(line.as_str());
            self.viewport_lines[row].clear();
            self.viewport_lines[row].push_str(line);
            self.viewport_style_runs[row] = self
                .session
                .dump_viewport_row_style_runs(row as u16)
                .unwrap_or_default();
            if row < self.line_layouts.len() {
                self.line_layouts[row] = None;
            }
        }

        self.viewport_line_offsets = Self::compute_viewport_line_offsets(&self.viewport_lines);
        self.viewport_total_len = Self::compute_viewport_total_len(&self.viewport_lines);
        self.selection = None;
        true
    }

    pub(crate) fn schedule_viewport_refresh(&mut self, cx: &mut Context<Self>) {
        self.pending_refresh = true;
        cx.notify();
    }

    pub fn queue_output_bytes(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        const MAX_PENDING_OUTPUT_BYTES: usize = 256 * 1024;

        if self.pending_output.len().saturating_add(bytes.len()) <= MAX_PENDING_OUTPUT_BYTES {
            self.pending_output.extend_from_slice(bytes);
            cx.notify();
            return;
        }

        if !self.pending_output.is_empty() {
            let pending = std::mem::take(&mut self.pending_output);
            self.feed_output_bytes_to_session(&pending);
            self.apply_side_effects(cx);
            self.reconcile_dirty_viewport_after_output();
        }

        if bytes.len() > MAX_PENDING_OUTPUT_BYTES {
            let mut offset = 0usize;
            while offset < bytes.len() {
                let end = (offset + MAX_PENDING_OUTPUT_BYTES).min(bytes.len());
                self.feed_output_bytes_to_session(&bytes[offset..end]);
                offset = end;
            }
            self.apply_side_effects(cx);
            self.reconcile_dirty_viewport_after_output();
            cx.notify();
            return;
        }

        self.pending_output.extend_from_slice(bytes);
        cx.notify();
    }

    pub fn resize_terminal(&mut self, cols: u16, rows: u16, cx: &mut Context<Self>) {
        if cols == self.session.cols() && rows == self.session.rows() {
            return;
        }
        let _ = self.session.resize(cols, rows);
        self.sync_viewport_scroll_tracking();
        self.pending_refresh = true;
        cx.notify();
    }
}
