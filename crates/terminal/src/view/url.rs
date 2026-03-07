use super::TerminalView;
use super::helpers::byte_index_for_column_in_line;

fn is_url_byte(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9')
        || matches!(
            b,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b':'
                | b'/'
                | b'?'
                | b'#'
                | b'['
                | b']'
                | b'@'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b'%'
        )
}

pub(crate) fn url_at_byte_index(text: &str, index: usize) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let mut idx = index.min(bytes.len().saturating_sub(1));

    if !is_url_byte(bytes[idx]) && idx > 0 && is_url_byte(bytes[idx - 1]) {
        idx -= 1;
    }

    if !is_url_byte(bytes[idx]) {
        return None;
    }

    let mut start = idx;
    while start > 0 && is_url_byte(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = idx + 1;
    while end < bytes.len() && is_url_byte(bytes[end]) {
        end += 1;
    }

    while end > start
        && matches!(
            bytes[end - 1],
            b'.' | b',' | b')' | b']' | b'}' | b';' | b':' | b'!' | b'?'
        )
    {
        end -= 1;
    }

    let candidate = std::str::from_utf8(&bytes[start..end]).ok()?;
    if candidate.starts_with("https://") || candidate.starts_with("http://") {
        Some(candidate.to_string())
    } else {
        None
    }
}

pub(crate) fn url_at_column_in_line(line: &str, col: u16) -> Option<String> {
    if line.is_empty() {
        return None;
    }

    let local = byte_index_for_column_in_line(line, col).min(line.len().saturating_sub(1));
    url_at_byte_index(line, local)
}

impl TerminalView {
    pub(crate) fn url_at_viewport_index(&self, index: usize) -> Option<String> {
        if self.viewport_lines.is_empty() {
            return None;
        }

        let idx = index.min(self.viewport_total_len.saturating_sub(1));
        let row = self
            .viewport_line_offsets
            .iter()
            .enumerate()
            .rfind(|(_, offset)| **offset <= idx)
            .map(|(i, _)| i)?;

        let line = self.viewport_lines.get(row)?.as_str();
        let line_start = *self.viewport_line_offsets.get(row).unwrap_or(&0);
        let local = idx
            .saturating_sub(line_start)
            .min(line.len().saturating_sub(1));
        url_at_byte_index(line, local)
    }
}
