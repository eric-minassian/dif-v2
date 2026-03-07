use gpui::{ClipboardItem, Context, ExternalPaths, Window};

use super::{ByteSelection, Copy, SelectAll, TerminalView};

impl TerminalView {
    pub(crate) fn apply_side_effects(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.session.take_clipboard_write() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub(crate) fn on_paste(
        &mut self,
        _: &super::Paste,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };

        // Strip escape characters from pasted text for security (like Zed).
        // Without bracketed paste, a malicious clipboard could inject terminal
        // escape sequences that execute arbitrary commands.
        let text = if self.session.bracketed_paste_enabled() {
            text
        } else {
            text.replace('\x1b', "")
        };

        if self.session.bracketed_paste_enabled() {
            self.send_input_parts(&[b"\x1b[200~", text.as_bytes(), b"\x1b[201~"], cx);
        } else {
            self.send_input_parts(&[text.as_bytes()], cx);
        }
    }

    pub(crate) fn on_file_drop(
        &mut self,
        paths: &ExternalPaths,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let text = paths
            .paths()
            .iter()
            .map(|p| shell_quote(&p.to_string_lossy()))
            .collect::<Vec<_>>()
            .join(" ");

        if text.is_empty() {
            return;
        }

        if self.session.bracketed_paste_enabled() {
            self.send_input_parts(&[b"\x1b[200~", text.as_bytes(), b"\x1b[201~"], cx);
        } else {
            self.send_input_parts(&[text.as_bytes()], cx);
        }
    }

    pub(crate) fn on_copy(&mut self, _: &Copy, _window: &mut Window, cx: &mut Context<Self>) {
        let selection = self
            .selection
            .map(|s| s.range())
            .filter(|range| !range.is_empty())
            .map(|range| self.viewport_slice(range))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| self.viewport_slice(0..self.viewport_total_len));

        let item = ClipboardItem::new_string(selection.to_string());
        cx.write_to_clipboard(item.clone());
        #[cfg(any(target_os = "linux", target_os = "freebsd"))]
        cx.write_to_primary(item);
    }

    pub(crate) fn on_select_all(
        &mut self,
        _: &SelectAll,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.selection = Some(ByteSelection {
            anchor: 0,
            active: self.viewport_total_len,
        });
        self.on_copy(&Copy, window, cx);
        cx.notify();
    }
}

pub(crate) fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    if s.bytes().all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'.' | b'/' | b':' | b'@')) {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}
