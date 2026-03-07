use gpui::{Bounds, Pixels, point, px};

pub(crate) fn split_viewport_lines(viewport: &str) -> Vec<String> {
    let viewport = viewport.strip_suffix('\n').unwrap_or(viewport);
    if viewport.is_empty() {
        return Vec::new();
    }
    viewport.split('\n').map(|line| line.to_string()).collect()
}

pub(crate) fn should_skip_key_down_for_ime(has_input: bool, keystroke: &gpui::Keystroke) -> bool {
    if !has_input || !keystroke.is_ime_in_progress() {
        return false;
    }

    !matches!(
        keystroke.key.as_str(),
        "enter" | "return" | "kp_enter" | "numpad_enter"
    )
}

pub(crate) fn ctrl_byte_for_keystroke(keystroke: &gpui::Keystroke) -> Option<u8> {
    let candidate = keystroke
        .key_char
        .as_deref()
        .or_else(|| (!keystroke.key.is_empty()).then_some(keystroke.key.as_str()))?;

    if candidate == "space" {
        return Some(0x00);
    }

    let bytes = candidate.as_bytes();
    if bytes.len() != 1 {
        return None;
    }

    let b = bytes[0];
    if (b'@'..=b'_').contains(&b) {
        Some(b & 0x1f)
    } else if b.is_ascii_lowercase() {
        Some(b - b'a' + 1)
    } else if b.is_ascii_uppercase() {
        Some(b - b'A' + 1)
    } else {
        // Special Ctrl combinations not in the @.._ or a..z ranges
        match b {
            b'/' => Some(0x1f), // Ctrl+/
            b'2' => Some(0x00), // Ctrl+2 (NUL)
            b'3' => Some(0x1b), // Ctrl+3 (ESC)
            b'4' => Some(0x1c), // Ctrl+4 (FS)
            b'5' => Some(0x1d), // Ctrl+5 (GS)
            b'6' => Some(0x1e), // Ctrl+6 (RS)
            b'7' => Some(0x1f), // Ctrl+7 (US)
            b'8' => Some(0x7f), // Ctrl+8 (DEL)
            _ => None,
        }
    }
}

pub(crate) fn sgr_mouse_button_value(
    base_button: u8,
    motion: bool,
    shift: bool,
    alt: bool,
    control: bool,
) -> u8 {
    let mut value = base_button;
    if motion {
        value = value.saturating_add(32);
    }
    if shift {
        value = value.saturating_add(4);
    }
    if alt {
        value = value.saturating_add(8);
    }
    if control {
        value = value.saturating_add(16);
    }
    value
}

pub(crate) fn window_position_to_local(
    last_bounds: Option<Bounds<Pixels>>,
    position: gpui::Point<gpui::Pixels>,
) -> gpui::Point<gpui::Pixels> {
    let origin = last_bounds
        .map(|bounds| bounds.origin)
        .unwrap_or_else(|| point(px(0.0), px(0.0)));
    point(position.x - origin.x, position.y - origin.y)
}

pub(crate) fn sgr_mouse_sequence(button_value: u8, col: u16, row: u16, pressed: bool) -> String {
    let suffix = if pressed { 'M' } else { 'm' };
    format!("\x1b[<{};{};{}{}", button_value, col, row, suffix)
}

/// Normal (X10) mouse encoding: ESC [ M Cb Cx Cy
/// Values are offset by 32. Coordinates capped at 223 (255-32).
pub(crate) fn normal_mouse_sequence(button_value: u8, col: u16, row: u16) -> Vec<u8> {
    let cb = button_value.saturating_add(32);
    let cx = (col.min(223) as u8).saturating_add(32);
    let cy = (row.min(223) as u8).saturating_add(32);
    vec![0x1b, b'[', b'M', cb, cx, cy]
}

pub(crate) fn byte_index_for_column_in_line(line: &str, col: u16) -> usize {
    use unicode_width::UnicodeWidthChar as _;

    let col = col.max(1) as usize;
    if col == 1 {
        return 0;
    }

    let mut current_col = 1usize;
    for (byte_index, ch) in line.char_indices() {
        let width = ch.width().unwrap_or(0);
        if width == 0 {
            continue;
        }

        if current_col == col {
            return byte_index;
        }

        let next_col = current_col.saturating_add(width);
        if col < next_col {
            return byte_index;
        }

        current_col = next_col;
    }

    line.len()
}

/// Encode a named key (arrow, home, end, etc.) with modifiers into escape bytes.
pub(crate) fn encode_key_named(
    name: &str,
    shift: bool,
    control: bool,
    alt: bool,
) -> Option<Vec<u8>> {
    // Modifier parameter for CSI sequences: 1 + (shift*1 + alt*2 + ctrl*4)
    let modifier_param =
        1 + if shift { 1 } else { 0 } + if alt { 2 } else { 0 } + if control { 4 } else { 0 };
    let has_modifiers = modifier_param > 1;

    // Helper: CSI sequence with optional modifier
    // Format: ESC [ 1 ; <mod> <final> or ESC [ <final>
    let csi_key = |final_byte: u8| -> Vec<u8> {
        if has_modifiers {
            format!("\x1b[1;{}{}", modifier_param, final_byte as char).into_bytes()
        } else {
            vec![0x1b, b'[', final_byte]
        }
    };

    // Helper: CSI ~ sequence with number
    // Format: ESC [ <num> ; <mod> ~ or ESC [ <num> ~
    let csi_tilde = |num: u8| -> Vec<u8> {
        if has_modifiers {
            format!("\x1b[{};{}~", num, modifier_param).into_bytes()
        } else {
            format!("\x1b[{}~", num).into_bytes()
        }
    };

    // Helper: SS3 sequence (for F1-F4 without modifiers)
    // Format: ESC O <final> or CSI 1 ; <mod> <final>
    let ss3_or_csi = |final_byte: u8| -> Vec<u8> {
        if has_modifiers {
            format!("\x1b[1;{}{}", modifier_param, final_byte as char).into_bytes()
        } else {
            vec![0x1b, b'O', final_byte]
        }
    };

    let result = match name {
        "up" => csi_key(b'A'),
        "down" => csi_key(b'B'),
        "right" => csi_key(b'C'),
        "left" => csi_key(b'D'),
        "home" => csi_key(b'H'),
        "end" => csi_key(b'F'),
        "insert" => csi_tilde(2),
        "delete" => csi_tilde(3),
        "pageup" | "page_up" => csi_tilde(5),
        "pagedown" | "page_down" => csi_tilde(6),
        "f1" => ss3_or_csi(b'P'),
        "f2" => ss3_or_csi(b'Q'),
        "f3" => ss3_or_csi(b'R'),
        "f4" => ss3_or_csi(b'S'),
        "f5" => csi_tilde(15),
        "f6" => csi_tilde(17),
        "f7" => csi_tilde(18),
        "f8" => csi_tilde(19),
        "f9" => csi_tilde(20),
        "f10" => csi_tilde(21),
        "f11" => csi_tilde(23),
        "f12" => csi_tilde(24),
        "tab" if !has_modifiers => b"\t".to_vec(),
        "tab" if shift && !control && !alt => b"\x1b[Z".to_vec(),
        "enter" | "return" if !has_modifiers => b"\r".to_vec(),
        "backspace" if !has_modifiers => vec![0x7f],
        "escape" if !has_modifiers => vec![0x1b],
        _ => return None,
    };

    Some(result)
}
