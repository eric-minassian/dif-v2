use super::super::Rgb;
use gpui::{Bounds, PaintQuad, Pixels, SharedString, TextRun, UnderlineStyle, fill, point, px};
use smallvec::SmallVec;

pub(crate) const CELL_STYLE_FLAG_BOLD: u8 = 0x02;
pub(crate) const CELL_STYLE_FLAG_ITALIC: u8 = 0x04;
pub(crate) const CELL_STYLE_FLAG_UNDERLINE: u8 = 0x08;
pub(crate) const CELL_STYLE_FLAG_FAINT: u8 = 0x10;
pub(crate) const CELL_STYLE_FLAG_STRIKETHROUGH: u8 = 0x40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextRunKey {
    pub fg: Rgb,
    pub flags: u8,
}

pub(crate) fn hsla_from_rgb(rgb: Rgb) -> gpui::Hsla {
    let rgba = gpui::Rgba {
        r: rgb.r as f32 / 255.0,
        g: rgb.g as f32 / 255.0,
        b: rgb.b as f32 / 255.0,
        a: 1.0,
    };
    rgba.into()
}

pub(crate) fn cursor_color_for_background(background: Rgb) -> gpui::Hsla {
    let bg = hsla_from_rgb(background);
    let mut cursor = if bg.l > 0.6 {
        gpui::black()
    } else {
        gpui::white()
    };
    cursor.a = 0.72;
    cursor
}

pub(crate) fn font_for_flags(base: &gpui::Font, flags: u8) -> gpui::Font {
    let mut font = base.clone();
    if flags & CELL_STYLE_FLAG_BOLD != 0 {
        font = font.bold();
    }
    if flags & CELL_STYLE_FLAG_ITALIC != 0 {
        font = font.italic();
    }
    font
}

pub(crate) fn color_for_key(key: TextRunKey) -> gpui::Hsla {
    let mut color = hsla_from_rgb(key.fg);
    if key.flags & CELL_STYLE_FLAG_FAINT != 0 {
        color = color.alpha(0.65);
    }
    color
}

pub(crate) const BOX_DIR_LEFT: u8 = 0x01;
pub(crate) const BOX_DIR_RIGHT: u8 = 0x02;
pub(crate) const BOX_DIR_UP: u8 = 0x04;
pub(crate) const BOX_DIR_DOWN: u8 = 0x08;

pub(crate) fn box_drawing_mask(ch: char) -> Option<(u8, f32)> {
    let light = 1.0;
    let heavy = 1.35;
    let double = 1.15;

    let mask = match ch {
        '─' | '━' | '═' => BOX_DIR_LEFT | BOX_DIR_RIGHT,
        '│' | '┃' | '║' => BOX_DIR_UP | BOX_DIR_DOWN,
        '┌' | '┏' | '╔' | '╭' => BOX_DIR_RIGHT | BOX_DIR_DOWN,
        '┐' | '┓' | '╗' | '╮' => BOX_DIR_LEFT | BOX_DIR_DOWN,
        '└' | '┗' | '╚' | '╰' => BOX_DIR_RIGHT | BOX_DIR_UP,
        '┘' | '┛' | '╝' | '╯' => BOX_DIR_LEFT | BOX_DIR_UP,
        '├' | '┣' | '╠' => BOX_DIR_RIGHT | BOX_DIR_UP | BOX_DIR_DOWN,
        '┤' | '┫' | '╣' => BOX_DIR_LEFT | BOX_DIR_UP | BOX_DIR_DOWN,
        '┬' | '┳' | '╦' => BOX_DIR_LEFT | BOX_DIR_RIGHT | BOX_DIR_DOWN,
        '┴' | '┻' | '╩' => BOX_DIR_LEFT | BOX_DIR_RIGHT | BOX_DIR_UP,
        '┼' | '╋' | '╬' => BOX_DIR_LEFT | BOX_DIR_RIGHT | BOX_DIR_UP | BOX_DIR_DOWN,
        _ => return None,
    };

    let scale = match ch {
        '━' | '┃' | '┏' | '┓' | '┗' | '┛' | '┣' | '┫' | '┳' | '┻' | '╋' => {
            heavy
        }
        '═' | '║' | '╔' | '╗' | '╚' | '╝' | '╠' | '╣' | '╦' | '╩' | '╬' => {
            double
        }
        _ => light,
    };

    Some((mask, scale))
}

pub(crate) fn box_drawing_quads_for_char(
    bounds: Bounds<Pixels>,
    line_height: Pixels,
    cell_width: f32,
    color: gpui::Hsla,
    ch: char,
) -> SmallVec<[PaintQuad; 2]> {
    let Some((mask, scale)) = box_drawing_mask(ch) else {
        return SmallVec::new();
    };

    let x0 = bounds.left();
    let x1 = x0 + px(cell_width);
    let y0 = bounds.top();
    let y1 = y0 + line_height;

    let mid_x = x0 + px(cell_width * 0.5);
    let mid_y = y0 + line_height * 0.5;

    let thickness = px(((f32::from(line_height) / 12.0).max(1.0) * scale).max(1.0));
    let half_t = thickness * 0.5;

    let has_left = mask & BOX_DIR_LEFT != 0;
    let has_right = mask & BOX_DIR_RIGHT != 0;
    let has_up = mask & BOX_DIR_UP != 0;
    let has_down = mask & BOX_DIR_DOWN != 0;

    let mut quads = SmallVec::new();

    if has_left || has_right {
        let (start_x, end_x) = if has_left && has_right {
            (x0, x1)
        } else if has_left {
            (x0, mid_x)
        } else {
            (mid_x, x1)
        };
        quads.push(fill(
            Bounds::from_corners(point(start_x, mid_y - half_t), point(end_x, mid_y + half_t)),
            color,
        ));
    }

    if has_up || has_down {
        let (start_y, end_y) = if has_up && has_down {
            (y0, y1)
        } else if has_up {
            (y0, mid_y)
        } else {
            (mid_y, y1)
        };

        quads.push(fill(
            Bounds::from_corners(point(mid_x - half_t, start_y), point(mid_x + half_t, end_y)),
            color,
        ));
    }

    quads
}

pub(crate) fn text_run_for_key(base_font: &gpui::Font, key: TextRunKey, len: usize) -> TextRun {
    let font = font_for_flags(base_font, key.flags);
    let color = color_for_key(key);

    let underline = (key.flags & CELL_STYLE_FLAG_UNDERLINE != 0).then_some(UnderlineStyle {
        color: Some(color),
        thickness: px(1.0),
        wavy: false,
    });

    let strikethrough =
        (key.flags & CELL_STYLE_FLAG_STRIKETHROUGH != 0).then_some(gpui::StrikethroughStyle {
            color: Some(color),
            thickness: px(1.0),
        });

    TextRun {
        len,
        font,
        color,
        background_color: None,
        underline,
        strikethrough,
    }
}

pub(crate) fn cell_metrics(window: &mut gpui::Window, font: &gpui::Font) -> Option<(f32, f32)> {
    let mut style = window.text_style();
    style.font_family = font.family.clone();
    style.font_features = super::super::default_terminal_font_features();
    style.font_fallbacks = font.fallbacks.clone();

    let rem_size = window.rem_size();
    let font_size = style.font_size.to_pixels(rem_size);
    let line_height = style.line_height.to_pixels(style.font_size, rem_size);

    let run = style.to_run(1);
    let lines = window
        .text_system()
        .shape_text(SharedString::from("M"), font_size, &[run], None, Some(1))
        .ok()?;
    let line = lines.first()?;

    let cell_width = f32::from(line.width()).max(1.0);
    let cell_height = f32::from(line_height).max(1.0);
    Some((cell_width, cell_height))
}
