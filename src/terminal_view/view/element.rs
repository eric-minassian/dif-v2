use gpui::{
    App, Bounds, Element, ElementId, ElementInputHandler, GlobalElementId, IntoElement, LayoutId,
    PaintQuad, Pixels, SharedString, Style, TextRun, UnderlineStyle, Window, fill, hsla, point, px,
    relative, rgba, size,
};

use super::TerminalView;
use super::drawing::{
    CELL_STYLE_FLAG_BOLD, CELL_STYLE_FLAG_FAINT, CELL_STYLE_FLAG_ITALIC,
    CELL_STYLE_FLAG_STRIKETHROUGH, CELL_STYLE_FLAG_UNDERLINE, TextRunKey, box_drawing_mask,
    box_drawing_quads_for_char, cell_metrics, color_for_key, cursor_color_for_background,
    hsla_from_rgb, text_run_for_key,
};
use super::helpers::byte_index_for_column_in_line;

pub(crate) struct TerminalPrepaintState {
    line_height: Pixels,
    shaped_lines: Vec<gpui::ShapedLine>,
    background_quads: Vec<PaintQuad>,
    selection_quads: Vec<PaintQuad>,
    box_drawing_quads: Vec<PaintQuad>,
    marked_text: Option<(gpui::ShapedLine, gpui::Point<Pixels>)>,
    marked_text_background: Option<PaintQuad>,
    cursor: Option<PaintQuad>,
}

pub(crate) struct TerminalTextElement {
    pub(crate) view: gpui::Entity<TerminalView>,
}

impl IntoElement for TerminalTextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TerminalTextElement {
    type RequestLayoutState = ();
    type PrepaintState = TerminalPrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = relative(1.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let mut style = window.text_style();
        let font = { self.view.read(cx).font.clone() };
        style.font_family = font.family.clone();
        style.font_features = super::super::default_terminal_font_features();
        style.font_fallbacks = font.fallbacks.clone();
        let default_fg = { self.view.read(cx).session.default_foreground() };
        style.color = hsla_from_rgb(default_fg);
        let rem_size = window.rem_size();
        let font_size = style.font_size.to_pixels(rem_size);
        let line_height = style.line_height.to_pixels(style.font_size, rem_size);

        let run_font = style.font();
        let run_color = style.color;

        let cell_width = cell_metrics(window, &font).map(|(w, _)| px(w));

        self.view.update(cx, |view, _cx| {
            if view.viewport_lines.is_empty() {
                view.line_layouts.clear();
                view.line_layout_key = None;
                return;
            }

            if view.line_layout_key != Some((font_size, line_height))
                || view.line_layouts.len() != view.viewport_lines.len()
            {
                view.line_layout_key = Some((font_size, line_height));
                view.line_layouts = vec![None; view.viewport_lines.len()];
            }

            for (idx, line) in view.viewport_lines.iter().enumerate() {
                let Some(slot) = view.line_layouts.get_mut(idx) else {
                    continue;
                };

                if let Some(existing) = slot.as_ref()
                    && existing.text.as_str() == line.as_str()
                {
                    continue;
                }

                let text = SharedString::from(line.clone());
                let mut runs: Vec<TextRun> = Vec::new();

                if let Some(style_runs) = view.viewport_style_runs.get(idx)
                    && !style_runs.is_empty()
                {
                    let mut byte_pos = 0usize;
                    for style in style_runs.iter() {
                        let key = TextRunKey {
                            fg: style.fg,
                            flags: style.flags
                                & (CELL_STYLE_FLAG_BOLD
                                    | CELL_STYLE_FLAG_ITALIC
                                    | CELL_STYLE_FLAG_UNDERLINE
                                    | CELL_STYLE_FLAG_FAINT
                                    | CELL_STYLE_FLAG_STRIKETHROUGH),
                        };

                        let start = byte_index_for_column_in_line(text.as_str(), style.start_col)
                            .min(text.len());
                        let end = byte_index_for_column_in_line(
                            text.as_str(),
                            style.end_col.saturating_add(1),
                        )
                        .min(text.len());

                        if start > byte_pos {
                            runs.push(TextRun {
                                len: start.saturating_sub(byte_pos),
                                font: run_font.clone(),
                                color: run_color,
                                background_color: None,
                                underline: None,
                                strikethrough: None,
                            });
                            byte_pos = start;
                        }

                        if end > start {
                            runs.push(text_run_for_key(&run_font, key, end.saturating_sub(start)));
                            byte_pos = end;
                        }
                    }

                    if byte_pos < text.len() {
                        runs.push(TextRun {
                            len: text.len().saturating_sub(byte_pos),
                            font: run_font.clone(),
                            color: run_color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        });
                    }
                }

                if runs.is_empty() {
                    runs.push(TextRun {
                        len: text.len(),
                        font: run_font.clone(),
                        color: run_color,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    });
                }

                let force_width = cell_width.and_then(|cell_width| {
                    use unicode_width::UnicodeWidthChar as _;
                    let has_wide = text.as_str().chars().any(|ch| ch.width().unwrap_or(0) > 1);
                    (!has_wide).then_some(cell_width)
                });
                let shaped = window
                    .text_system()
                    .shape_line(text, font_size, &runs, force_width);
                *slot = Some(shaped);
            }
        });

        let default_bg = { self.view.read(cx).session.default_background() };
        let background_quads = cell_metrics(window, &font)
            .map(|(cell_width, _)| {
                let origin = bounds.origin;
                let mut quads: Vec<PaintQuad> = Vec::new();

                let view = self.view.read(cx);
                for (row, runs) in view.viewport_style_runs.iter().enumerate() {
                    if runs.is_empty() {
                        continue;
                    }

                    let y = origin.y + line_height * row as f32;
                    for run in runs.iter() {
                        if run.bg == default_bg {
                            continue;
                        }

                        let x =
                            origin.x + px(cell_width * (run.start_col.saturating_sub(1)) as f32);
                        let w = px(cell_width
                            * (run.end_col.saturating_sub(run.start_col).saturating_add(1)) as f32);
                        let color = rgba(
                            (u32::from(run.bg.r) << 24)
                                | (u32::from(run.bg.g) << 16)
                                | (u32::from(run.bg.b) << 8)
                                | 0xFF,
                        );
                        quads.push(fill(Bounds::new(point(x, y), size(w, line_height)), color));
                    }
                }

                quads
            })
            .unwrap_or_default();

        let (shaped_lines, selection, line_offsets) = {
            let view = self.view.read(cx);
            (
                view.line_layouts
                    .iter()
                    .map(|line| line.clone().unwrap_or_default())
                    .collect::<Vec<_>>(),
                view.selection,
                view.viewport_line_offsets.clone(),
            )
        };

        let (marked_text, cursor_position, font) = {
            let view = self.view.read(cx);
            (
                view.marked_text.clone(),
                view.session.cursor_position(),
                view.font.clone(),
            )
        };

        let (marked_text, marked_text_background) = marked_text
            .and_then(|text| {
                if text.is_empty() {
                    return None;
                }
                let (col, row) = cursor_position?;
                let (cell_width, _) = cell_metrics(window, &font)?;

                let origin_x = bounds.left() + px(cell_width * (col.saturating_sub(1)) as f32);
                let origin_y = bounds.top() + line_height * (row.saturating_sub(1)) as f32;
                let origin = point(origin_x, origin_y);

                let run = TextRun {
                    len: text.len(),
                    font: run_font.clone(),
                    color: run_color,
                    background_color: None,
                    underline: Some(UnderlineStyle {
                        color: Some(run_color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    strikethrough: None,
                };
                let force_width = {
                    use unicode_width::UnicodeWidthChar as _;
                    let has_wide = text.as_str().chars().any(|ch| ch.width().unwrap_or(0) > 1);
                    (!has_wide).then_some(px(cell_width))
                };
                let shaped =
                    window
                        .text_system()
                        .shape_line(text.clone(), font_size, &[run], force_width);

                let bg = {
                    let view = self.view.read(cx);
                    let row_index = row.saturating_sub(1) as usize;
                    view.viewport_style_runs
                        .get(row_index)
                        .and_then(|runs| {
                            runs.iter().find_map(|run| {
                                (col >= run.start_col && col <= run.end_col).then_some(run.bg)
                            })
                        })
                        .unwrap_or(default_bg)
                };

                let cell_len = {
                    use unicode_width::UnicodeWidthChar as _;
                    let mut cells = 0usize;
                    for ch in text.as_str().chars() {
                        let w = ch.width().unwrap_or(0);
                        if w > 0 {
                            cells = cells.saturating_add(w);
                        }
                    }
                    cells.max(1)
                };

                let marked_text_background = fill(
                    Bounds::new(origin, size(px(cell_width * cell_len as f32), line_height)),
                    rgba(
                        (u32::from(bg.r) << 24)
                            | (u32::from(bg.g) << 16)
                            | (u32::from(bg.b) << 8)
                            | 0xFF,
                    ),
                );

                Some(((shaped, origin), marked_text_background))
            })
            .map(|(text, bg)| (Some(text), Some(bg)))
            .unwrap_or((None, None));

        let selection_quads = selection
            .map(|sel| sel.range())
            .filter(|range| !range.is_empty())
            .map(|range| {
                let highlight = hsla(0.58, 0.9, 0.55, 0.35);
                let mut quads = Vec::new();

                for (row, line) in shaped_lines.iter().enumerate() {
                    let Some(&line_offset) = line_offsets.get(row) else {
                        continue;
                    };

                    let line_start = line_offset;
                    let line_end = line_offset.saturating_add(line.text.len());

                    let seg_start = range.start.max(line_start).min(line_end);
                    let seg_end = range.end.max(line_start).min(line_end);
                    if seg_start >= seg_end {
                        continue;
                    }

                    let local_start = seg_start.saturating_sub(line_start);
                    let local_end = seg_end.saturating_sub(line_start);

                    let x1 = line.x_for_index(local_start);
                    let x2 = line.x_for_index(local_end);

                    let y1 = bounds.top() + line_height * row as f32;
                    let y2 = y1 + line_height;

                    quads.push(fill(
                        Bounds::from_corners(
                            point(bounds.left() + x1, y1),
                            point(bounds.left() + x2, y2),
                        ),
                        highlight,
                    ));
                }

                quads
            })
            .unwrap_or_default();

        let box_drawing_quads = cell_metrics(window, &font)
            .map(|(cell_width, _)| {
                use unicode_width::UnicodeWidthChar as _;
                let default_fg = run_color;
                let mut quads = Vec::new();

                let view = self.view.read(cx);
                for (row, line) in view.viewport_lines.iter().enumerate() {
                    let y = bounds.top() + line_height * row as f32;
                    let runs = view.viewport_style_runs.get(row).map(|v| v.as_slice());
                    let mut run_idx: usize = 0;

                    let mut col = 1usize;
                    for ch in line.chars() {
                        let width = ch.width().unwrap_or(0);
                        if width == 0 {
                            continue;
                        }

                        if let Some((_, _)) = box_drawing_mask(ch) {
                            let fg = runs
                                .and_then(|runs| {
                                    while let Some(run) = runs.get(run_idx) {
                                        if (col as u16) <= run.end_col {
                                            break;
                                        }
                                        run_idx = run_idx.saturating_add(1);
                                    }
                                    runs.get(run_idx).and_then(|run| {
                                        (col as u16 >= run.start_col && (col as u16) <= run.end_col)
                                            .then_some(run)
                                    })
                                })
                                .map(|run| {
                                    let key = TextRunKey {
                                        fg: run.fg,
                                        flags: run.flags
                                            & (CELL_STYLE_FLAG_FAINT
                                                | CELL_STYLE_FLAG_BOLD
                                                | CELL_STYLE_FLAG_ITALIC
                                                | CELL_STYLE_FLAG_UNDERLINE
                                                | CELL_STYLE_FLAG_STRIKETHROUGH),
                                    };
                                    color_for_key(key)
                                })
                                .unwrap_or(default_fg);

                            let x = bounds.left() + px(cell_width * (col.saturating_sub(1)) as f32);
                            let cell_bounds =
                                Bounds::new(point(x, y), size(px(cell_width), line_height));
                            quads.extend(box_drawing_quads_for_char(
                                cell_bounds,
                                line_height,
                                cell_width,
                                fg,
                                ch,
                            ));
                        }

                        col = col.saturating_add(width);
                    }
                }

                quads
            })
            .unwrap_or_default();

        let cursor = {
            let view = self.view.read(cx);
            view.focus_handle
                .is_focused(window)
                .then(|| view.session.cursor_position())
                .flatten()
        }
        .and_then(|(col, row)| {
            let background = { self.view.read(cx).session.default_background() };
            let cursor_color = cursor_color_for_background(background);
            let y = bounds.top() + line_height * (row.saturating_sub(1)) as f32;
            let row_index = row.saturating_sub(1) as usize;
            let line = shaped_lines.get(row_index)?;
            let byte_index = byte_index_for_column_in_line(line.text.as_str(), col);
            let x = bounds.left() + line.x_for_index(byte_index.min(line.text.len()));

            Some(fill(
                Bounds::new(point(x, y), size(px(2.0), line_height)),
                cursor_color,
            ))
        });

        TerminalPrepaintState {
            line_height,
            shaped_lines,
            background_quads,
            selection_quads,
            box_drawing_quads,
            marked_text,
            marked_text_background,
            cursor,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let bounds_changed = self.view.read(cx).last_bounds != Some(bounds);
        let metrics = cell_metrics(window, &self.view.read(cx).font);
        self.view.update(cx, |view, _cx| {
            view.last_bounds = Some(bounds);
            view.last_cell_metrics = metrics;
        });

        if bounds_changed {
            if let Some((cell_width, cell_height)) = metrics {
                let cols = (f32::from(bounds.size.width) / cell_width)
                    .floor()
                    .max(1.0) as u16;
                let rows = (f32::from(bounds.size.height) / cell_height)
                    .floor()
                    .max(1.0) as u16;
                self.view.update(cx, |view, cx| {
                    if let Some(cb) = view.resize_callback.as_ref() {
                        cb(cols, rows);
                    }
                    view.resize_terminal(cols, rows, cx);
                });
            }
        }

        let focus_handle = { self.view.read(cx).focus_handle.clone() };
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.view.clone()),
            cx,
        );

        window.paint_layer(bounds, |window| {
            let default_bg = { self.view.read(cx).session.default_background() };
            window.paint_quad(fill(bounds, hsla_from_rgb(default_bg)));

            for quad in prepaint.background_quads.drain(..) {
                window.paint_quad(quad);
            }

            for quad in prepaint.selection_quads.drain(..) {
                window.paint_quad(quad);
            }

            let origin = bounds.origin;
            for (row, line) in prepaint.shaped_lines.iter().enumerate() {
                let y = origin.y + prepaint.line_height * row as f32;
                let _ = line.paint(
                    point(origin.x, y),
                    prepaint.line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }

            for quad in prepaint.box_drawing_quads.drain(..) {
                window.paint_quad(quad);
            }

            if let Some(bg) = prepaint.marked_text_background.take() {
                window.paint_quad(bg);
            }

            if let Some((line, origin)) = prepaint.marked_text.as_ref() {
                let _ = line.paint(
                    *origin,
                    prepaint.line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }

            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
    }
}
