use alacritty_terminal::event::{Event as AlacEvent, EventListener};
use alacritty_terminal::grid::{Dimensions, Scroll};
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::Config as AlacConfig;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::vte::ansi::Processor;
use parking_lot::FairMutex;
use std::sync::{Arc, mpsc};
use unicode_width::UnicodeWidthChar as _;

use super::colors::ANSI_COLORS;
use super::config::{Rgb, StyleRun, TerminalConfig};
use super::view::drawing::{
    CELL_STYLE_FLAG_BOLD, CELL_STYLE_FLAG_FAINT, CELL_STYLE_FLAG_ITALIC,
    CELL_STYLE_FLAG_STRIKETHROUGH, CELL_STYLE_FLAG_UNDERLINE,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CursorShape {
    #[default]
    Bar,
    Block,
    Underline,
}

#[derive(Clone)]
struct Listener {
    event_tx: mpsc::Sender<AlacEvent>,
}

impl EventListener for Listener {
    fn send_event(&self, event: AlacEvent) {
        let _ = self.event_tx.send(event);
    }
}

pub struct TerminalSession {
    config: TerminalConfig,
    term: Arc<FairMutex<Term<Listener>>>,
    processor: Processor,
    event_rx: mpsc::Receiver<AlacEvent>,
    title: Option<String>,
    clipboard_write: Option<String>,
    last_display_offset: usize,
}

struct SizeDimensions {
    cols: usize,
    rows: usize,
}

impl Dimensions for SizeDimensions {
    fn total_lines(&self) -> usize {
        // Provide scrollback so that reflow on resize can preserve content.
        self.rows + 1000
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

impl TerminalSession {
    pub fn new(config: TerminalConfig) -> Result<Self, anyhow::Error> {
        let (event_tx, event_rx) = mpsc::channel();
        let listener = Listener { event_tx };

        let alac_config = AlacConfig::default();
        let dims = SizeDimensions {
            cols: config.cols as usize,
            rows: config.rows as usize,
        };
        let term = Term::new(alac_config, &dims, listener);

        Ok(Self {
            config,
            term: Arc::new(FairMutex::new(term)),
            processor: Processor::new(),
            event_rx,
            title: None,
            clipboard_write: None,
            last_display_offset: 0,
        })
    }

    pub fn cols(&self) -> u16 {
        self.config.cols
    }

    pub fn rows(&self) -> u16 {
        self.config.rows
    }

    pub fn default_foreground(&self) -> Rgb {
        self.config.default_fg
    }

    pub fn default_background(&self) -> Rgb {
        self.config.default_bg
    }

    pub fn bracketed_paste_enabled(&self) -> bool {
        let term = self.term.lock();
        term.mode()
            .contains(alacritty_terminal::term::TermMode::BRACKETED_PASTE)
    }

    pub fn mouse_reporting_enabled(&self) -> bool {
        let term = self.term.lock();
        let mode = term.mode();
        mode.contains(alacritty_terminal::term::TermMode::MOUSE_REPORT_CLICK)
            || mode.contains(alacritty_terminal::term::TermMode::MOUSE_DRAG)
            || mode.contains(alacritty_terminal::term::TermMode::MOUSE_MOTION)
    }

    pub fn mouse_sgr_enabled(&self) -> bool {
        let term = self.term.lock();
        term.mode()
            .contains(alacritty_terminal::term::TermMode::SGR_MOUSE)
    }

    pub fn mouse_button_event_enabled(&self) -> bool {
        let term = self.term.lock();
        term.mode()
            .contains(alacritty_terminal::term::TermMode::MOUSE_DRAG)
    }

    pub fn mouse_any_event_enabled(&self) -> bool {
        let term = self.term.lock();
        term.mode()
            .contains(alacritty_terminal::term::TermMode::MOUSE_MOTION)
    }

    pub fn focus_events_enabled(&self) -> bool {
        let term = self.term.lock();
        term.mode()
            .contains(alacritty_terminal::term::TermMode::FOCUS_IN_OUT)
    }

    pub fn alt_screen_active(&self) -> bool {
        let term = self.term.lock();
        term.mode()
            .contains(alacritty_terminal::term::TermMode::ALT_SCREEN)
    }

    pub fn alternate_scroll_enabled(&self) -> bool {
        let term = self.term.lock();
        term.mode()
            .contains(alacritty_terminal::term::TermMode::ALTERNATE_SCROLL)
    }

    pub fn display_offset(&self) -> usize {
        let term = self.term.lock();
        term.grid().display_offset()
    }

    pub fn cursor_shape(&self) -> CursorShape {
        let term = self.term.lock();
        match term.cursor_style().shape {
            alacritty_terminal::vte::ansi::CursorShape::Block => CursorShape::Block,
            alacritty_terminal::vte::ansi::CursorShape::Underline => CursorShape::Underline,
            alacritty_terminal::vte::ansi::CursorShape::Beam => CursorShape::Bar,
            _ => CursorShape::Bar,
        }
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub(crate) fn window_title_updates_enabled(&self) -> bool {
        self.config.update_window_title
    }

    pub fn hyperlink_at(&self, col: u16, row: u16) -> Option<String> {
        let term = self.term.lock();
        let point = Point::new(
            Line(row.saturating_sub(1) as i32),
            Column(col.saturating_sub(1) as usize),
        );
        let cell = &term.grid()[point];
        cell.hyperlink().map(|h| h.uri().to_string())
    }

    pub fn take_clipboard_write(&mut self) -> Option<String> {
        self.clipboard_write.take()
    }

    fn drain_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AlacEvent::Title(title) => {
                    self.title = Some(title);
                }
                AlacEvent::ResetTitle => {
                    self.title = None;
                }
                AlacEvent::ClipboardStore(_, text) => {
                    self.clipboard_write = Some(text);
                }
                AlacEvent::PtyWrite(text) => {
                    // Alacritty handles DSR/DA/OSC color queries internally
                    // and emits PtyWrite events with the responses.
                    // We store them for feed_with_pty_responses to pick up.
                    if let Some(existing) = self.clipboard_write.as_mut() {
                        // Don't overwrite clipboard with PTY responses
                        let _ = existing;
                    }
                    // PtyWrite responses are handled via the send callback
                    // in feed_with_pty_responses. For plain feed(), they're ignored.
                    let _ = text;
                }
                _ => {}
            }
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), anyhow::Error> {
        {
            let mut term = self.term.lock();
            self.processor.advance(&mut *term, bytes);
        }
        self.drain_events();
        Ok(())
    }

    pub fn feed_with_pty_responses(
        &mut self,
        bytes: &[u8],
        mut send: impl FnMut(&[u8]),
    ) -> Result<(), anyhow::Error> {
        {
            let mut term = self.term.lock();
            self.processor.advance(&mut *term, bytes);
        }
        // Drain events, forwarding PtyWrite responses
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                AlacEvent::Title(title) => {
                    self.title = Some(title);
                }
                AlacEvent::ResetTitle => {
                    self.title = None;
                }
                AlacEvent::ClipboardStore(_, text) => {
                    self.clipboard_write = Some(text);
                }
                AlacEvent::PtyWrite(text) => {
                    send(text.as_bytes());
                }
                AlacEvent::ColorRequest(idx, format_fn) => {
                    let rgb = self.resolve_color_index(idx);
                    let response = format_fn(rgb);
                    send(response.as_bytes());
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn resolve_color_index(&self, idx: usize) -> alacritty_terminal::vte::ansi::Rgb {
        let term = self.term.lock();
        if let Some(rgb) = term.colors()[idx] {
            return rgb;
        }
        // Fall back to our ANSI table for indexed colors
        if idx < 256 {
            let (r, g, b) = ANSI_COLORS[idx];
            return alacritty_terminal::vte::ansi::Rgb { r, g, b };
        }
        // Foreground/background defaults
        match idx {
            256 => alacritty_terminal::vte::ansi::Rgb {
                r: self.config.default_fg.r,
                g: self.config.default_fg.g,
                b: self.config.default_fg.b,
            },
            257 => alacritty_terminal::vte::ansi::Rgb {
                r: self.config.default_bg.r,
                g: self.config.default_bg.g,
                b: self.config.default_bg.b,
            },
            _ => alacritty_terminal::vte::ansi::Rgb {
                r: 0xFF,
                g: 0xFF,
                b: 0xFF,
            },
        }
    }

    fn resolve_cell_color(
        &self,
        color: alacritty_terminal::vte::ansi::Color,
        term: &Term<Listener>,
    ) -> Rgb {
        use alacritty_terminal::vte::ansi::{Color, NamedColor};
        match color {
            Color::Spec(rgb) => Rgb {
                r: rgb.r,
                g: rgb.g,
                b: rgb.b,
            },
            Color::Indexed(idx) => self.resolve_indexed_color(idx as usize, term),
            Color::Named(name) => {
                let fallback = match name {
                    NamedColor::Background => self.config.default_bg,
                    _ => self.config.default_fg,
                };
                self.resolve_indexed_color_with_fallback(name as usize, term, fallback)
            }
        }
    }

    fn resolve_indexed_color(&self, idx: usize, term: &Term<Listener>) -> Rgb {
        self.resolve_indexed_color_with_fallback(idx, term, self.config.default_fg)
    }

    fn resolve_indexed_color_with_fallback(
        &self,
        idx: usize,
        term: &Term<Listener>,
        fallback: Rgb,
    ) -> Rgb {
        if let Some(rgb) = term.colors()[idx] {
            return Rgb {
                r: rgb.r,
                g: rgb.g,
                b: rgb.b,
            };
        }
        if idx < ANSI_COLORS.len() {
            let (r, g, b) = ANSI_COLORS[idx];
            return Rgb { r, g, b };
        }
        fallback
    }

    pub fn dump_viewport(&self) -> Result<String, anyhow::Error> {
        let term = self.term.lock();
        let rows = term.screen_lines();
        let cols = term.columns();
        let display_offset = term.grid().display_offset() as i32;
        let mut output = String::new();

        for row_idx in 0..rows {
            let line = Line(row_idx as i32 - display_offset);
            let mut row_str = String::new();

            for col_idx in 0..cols {
                let point = Point::new(line, Column(col_idx));
                let cell = &term.grid()[point];
                let ch = cell.c;
                if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                    continue;
                }
                if ch == '\0' || ch == ' ' {
                    row_str.push(' ');
                } else {
                    row_str.push(ch);
                }
            }

            output.push_str(&row_str);
            if row_idx + 1 < rows {
                output.push('\n');
            }
        }

        Ok(output)
    }

    pub fn dump_viewport_row(&self, row: u16) -> Result<String, anyhow::Error> {
        let term = self.term.lock();
        let cols = term.columns();
        let display_offset = term.grid().display_offset() as i32;
        let line = Line(row as i32 - display_offset);
        let mut row_str = String::new();

        for col_idx in 0..cols {
            let point = Point::new(line, Column(col_idx));
            let cell = &term.grid()[point];
            if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                continue;
            }
            let ch = cell.c;
            if ch == '\0' || ch == ' ' {
                row_str.push(' ');
            } else {
                row_str.push(ch);
            }
        }

        row_str.push('\n');
        Ok(row_str)
    }

    pub fn dump_viewport_row_style_runs(&self, row: u16) -> Result<Vec<StyleRun>, anyhow::Error> {
        let term = self.term.lock();
        let cols = term.columns();
        let display_offset = term.grid().display_offset() as i32;
        let line = Line(row as i32 - display_offset);

        let mut runs = Vec::new();
        let mut current_run: Option<(Rgb, Rgb, u8, u16)> = None; // (fg, bg, flags, start_col)
        let mut col_pos: u16 = 1; // 1-based

        for col_idx in 0..cols {
            let point = Point::new(line, Column(col_idx));
            let cell = &term.grid()[point];

            if cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                continue;
            }

            let mut fg = self.resolve_cell_color(cell.fg, &term);
            let mut bg = self.resolve_cell_color(cell.bg, &term);

            let mut flags = 0u8;
            if cell.flags.contains(CellFlags::BOLD) {
                flags |= CELL_STYLE_FLAG_BOLD;
            }
            if cell.flags.contains(CellFlags::ITALIC) {
                flags |= CELL_STYLE_FLAG_ITALIC;
            }
            if cell.flags.intersects(
                CellFlags::UNDERLINE
                    | CellFlags::DOUBLE_UNDERLINE
                    | CellFlags::UNDERCURL
                    | CellFlags::DOTTED_UNDERLINE
                    | CellFlags::DASHED_UNDERLINE,
            ) {
                flags |= CELL_STYLE_FLAG_UNDERLINE;
            }
            if cell.flags.contains(CellFlags::DIM) {
                flags |= CELL_STYLE_FLAG_FAINT;
            }
            if cell.flags.contains(CellFlags::STRIKEOUT) {
                flags |= CELL_STYLE_FLAG_STRIKETHROUGH;
            }

            // Handle inverse
            if cell.flags.contains(CellFlags::INVERSE) {
                std::mem::swap(&mut fg, &mut bg);
            }

            if let Some((run_fg, run_bg, run_flags, start)) = current_run {
                if run_fg == fg && run_bg == bg && run_flags == flags {
                    // Continue the current run
                } else {
                    runs.push(StyleRun {
                        start_col: start,
                        end_col: col_pos - 1,
                        fg: run_fg,
                        bg: run_bg,
                        flags: run_flags,
                    });
                    current_run = Some((fg, bg, flags, col_pos));
                }
            } else {
                current_run = Some((fg, bg, flags, col_pos));
            }

            let ch_width = cell.c.width().unwrap_or(1).max(1) as u16;
            col_pos += ch_width;
        }

        if let Some((fg, bg, flags, start)) = current_run {
            runs.push(StyleRun {
                start_col: start,
                end_col: col_pos - 1,
                fg,
                bg,
                flags,
            });
        }

        Ok(runs)
    }

    pub fn cursor_position(&self) -> Option<(u16, u16)> {
        let term = self.term.lock();
        // Hide cursor when scrolled into history
        if term.grid().display_offset() != 0 {
            return None;
        }
        let cursor = term.grid().cursor.point;
        // Convert to 1-based
        Some((cursor.column.0 as u16 + 1, cursor.line.0 as u16 + 1))
    }

    pub fn scroll_viewport(&mut self, delta_lines: i32) -> Result<(), anyhow::Error> {
        let mut term = self.term.lock();
        term.scroll_display(Scroll::Delta(delta_lines));
        Ok(())
    }

    pub fn scroll_viewport_top(&mut self) -> Result<(), anyhow::Error> {
        let mut term = self.term.lock();
        term.scroll_display(Scroll::Top);
        Ok(())
    }

    pub fn scroll_viewport_bottom(&mut self) -> Result<(), anyhow::Error> {
        let mut term = self.term.lock();
        term.scroll_display(Scroll::Bottom);
        Ok(())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), anyhow::Error> {
        self.config.cols = cols;
        self.config.rows = rows;
        let mut term = self.term.lock();
        let dims = SizeDimensions {
            cols: cols as usize,
            rows: rows as usize,
        };
        term.resize(dims);
        Ok(())
    }

    pub(crate) fn take_dirty_viewport_rows(&mut self) -> Vec<u16> {
        use alacritty_terminal::term::TermDamage;
        let mut term = self.term.lock();
        let mut dirty = Vec::new();
        match term.damage() {
            TermDamage::Full => {
                let rows = term.screen_lines();
                for row in 0..rows {
                    dirty.push(row as u16);
                }
            }
            TermDamage::Partial(iter) => {
                for bounds in iter {
                    dirty.push(bounds.line as u16);
                }
            }
        }
        term.reset_damage();
        dirty
    }

    pub(crate) fn take_viewport_scroll_delta(&mut self) -> i32 {
        let term = self.term.lock();
        let current_offset = term.grid().display_offset();
        let delta = self.last_display_offset as i32 - current_offset as i32;
        drop(term);
        self.last_display_offset = self.term.lock().grid().display_offset();
        delta
    }
}
