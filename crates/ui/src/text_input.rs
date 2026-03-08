use std::ops::Range;

use gpui::{
    Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill,
    point, prelude::*, px, relative, rgba, size,
};
use unicode_segmentation::*;

use crate::theme::theme;

actions!(
    text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Confirm,
        Cancel,
        Paste,
        Cut,
        Copy,
    ]
);

pub fn key_bindings() -> Vec<KeyBinding> {
    let ctx = Some("TextInput");
    vec![
        KeyBinding::new("backspace", Backspace, ctx),
        KeyBinding::new("delete", Delete, ctx),
        KeyBinding::new("left", Left, ctx),
        KeyBinding::new("right", Right, ctx),
        KeyBinding::new("shift-left", SelectLeft, ctx),
        KeyBinding::new("shift-right", SelectRight, ctx),
        KeyBinding::new("cmd-a", SelectAll, ctx),
        KeyBinding::new("home", Home, ctx),
        KeyBinding::new("end", End, ctx),
        KeyBinding::new("enter", Confirm, ctx),
        KeyBinding::new("escape", Cancel, ctx),
        KeyBinding::new("cmd-v", Paste, ctx),
        KeyBinding::new("cmd-c", Copy, ctx),
        KeyBinding::new("cmd-x", Cut, ctx),
    ]
}

#[derive(Clone, Debug)]
pub enum TextInputEvent {
    Confirm(String),
    Cancel,
}

/// Stored layout info for wrapped text, used by mouse/IME handlers.
struct WrappedLayout {
    lines: Vec<ShapedLine>,
    /// Byte offset in the original text where each visual line starts.
    byte_offsets: Vec<usize>,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
}

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    scroll_offset: Pixels,
    is_selecting: bool,
    wrap: bool,
    wrapped_line_count: usize,
    last_wrapped: Option<WrappedLayout>,
}

impl gpui::EventEmitter<TextInputEvent> for TextInput {}

impl TextInput {
    pub fn new(initial_text: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let len = initial_text.len();
        Self {
            focus_handle,
            content: initial_text.into(),
            selected_range: 0..len,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            scroll_offset: px(0.),
            is_selecting: false,
            wrap: false,
            wrapped_line_count: 1,
            last_wrapped: None,
        }
    }

    /// Enable text wrapping (multiline display). The input remains logically
    /// single-line (no newlines) but wraps visually at the container width.
    pub fn wrapping(mut self) -> Self {
        self.wrap = true;
        self
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    fn confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.content.to_string().trim().to_string();
        cx.emit(TextInputEvent::Confirm(text));
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(TextInputEvent::Cancel);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace('\n', " "), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        // Wrapped mode: use wrapped layout
        if let Some(wrapped) = &self.last_wrapped {
            if position.y < wrapped.bounds.top() {
                return 0;
            }
            if position.y > wrapped.bounds.bottom() {
                return self.content.len();
            }
            let relative_y = position.y - wrapped.bounds.top();
            let line_idx =
                ((relative_y / wrapped.line_height) as usize).min(wrapped.lines.len() - 1);
            let local_idx =
                wrapped.lines[line_idx].closest_index_for_x(position.x - wrapped.bounds.left());
            return wrapped.byte_offsets[line_idx] + local_idx;
        }

        // Single-line mode
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left() + self.scroll_offset)
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }

    /// Given a byte index, find which visual line it's on and its x-offset
    /// within that line. Returns `(line_index, x_position)`.
    fn wrapped_cursor_position(&self, byte_index: usize) -> Option<(usize, Pixels)> {
        let wrapped = self.last_wrapped.as_ref()?;
        let line_idx = wrapped
            .byte_offsets
            .iter()
            .rposition(|&off| off <= byte_index)
            .unwrap_or(0);
        let local = byte_index - wrapped.byte_offsets[line_idx];
        let x = wrapped.lines[line_idx].x_for_index(local);
        Some((line_idx, x))
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);

        if let Some((line_idx, x)) = self.wrapped_cursor_position(range.start) {
            let wrapped = self.last_wrapped.as_ref()?;
            let (_end_line, end_x) = self
                .wrapped_cursor_position(range.end)
                .unwrap_or((line_idx, x));
            // Return bounds on the start line (good enough for IME positioning)
            let y = bounds.top() + wrapped.line_height * line_idx as f32;
            return Some(Bounds::from_corners(
                point(bounds.left() + x, y),
                point(bounds.left() + end_x, y + wrapped.line_height),
            ));
        }

        let last_layout = self.last_layout.as_ref()?;
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start) - self.scroll_offset,
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end) - self.scroll_offset,
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if let Some(wrapped) = &self.last_wrapped {
            let relative_y = point.y - wrapped.bounds.top();
            let line_idx = (relative_y / wrapped.line_height) as usize;
            let local_x = point.x - wrapped.bounds.left();
            let utf8_index = wrapped.lines[line_idx].index_for_x(local_x)?;
            return Some(self.offset_to_utf16(wrapped.byte_offsets[line_idx] + utf8_index));
        }

        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x + self.scroll_offset)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

// ── Wrapped line computation ────────────────────────────────────────────────

/// Split `text` into visual lines that fit within `max_width`.
/// Returns `(lines, byte_offsets)` where each entry is a `ShapedLine` and
/// the byte offset in the original text where that visual line starts.
fn compute_wrapped_lines(
    full_line: &ShapedLine,
    text: &str,
    max_width: Pixels,
    font_size: Pixels,
    base_run: &TextRun,
    window: &Window,
) -> (Vec<ShapedLine>, Vec<usize>) {
    let ts = window.text_system();

    if text.is_empty() || max_width <= px(0.) {
        let line = ts.shape_line(" ".into(), font_size, &[base_run.clone()], None);
        return (vec![line], vec![0]);
    }

    let mut lines = Vec::new();
    let mut byte_offsets = Vec::new();
    let mut start: usize = 0;

    for (idx, _) in text.grapheme_indices(true) {
        if idx == start {
            continue;
        }
        let x = full_line.x_for_index(idx) - full_line.x_for_index(start);
        if x > max_width {
            // Must include at least one grapheme per line
            let break_at = if idx > start { idx } else { start + 1 };
            let segment: SharedString = text[start..break_at].to_owned().into();
            let run = TextRun {
                len: segment.len(),
                ..base_run.clone()
            };
            lines.push(ts.shape_line(segment, font_size, &[run], None));
            byte_offsets.push(start);
            start = break_at;
        }
    }

    // Last line
    let segment: SharedString = if start < text.len() {
        text[start..].to_owned().into()
    } else {
        " ".into()
    };
    let run = TextRun {
        len: segment.len(),
        ..base_run.clone()
    };
    lines.push(ts.shape_line(segment, font_size, &[run], None));
    byte_offsets.push(start);

    (lines, byte_offsets)
}

// ── Custom element ──────────────────────────────────────────────────────────

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    scroll_offset: Pixels,
    /// Single-line mode: one line. Wrapped mode: multiple lines.
    lines: Vec<ShapedLine>,
    /// Byte offset where each visual line starts (always same len as `lines`).
    byte_offsets: Vec<usize>,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

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
        cx: &mut gpui::App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let input = self.input.read(cx);
        let line_count = if input.wrap {
            input.wrapped_line_count.max(1)
        } else {
            1
        };
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = (window.line_height() * line_count as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        let t = theme();
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let wrap = input.wrap;
        let mut scroll_offset = if wrap { px(0.) } else { input.scroll_offset };
        let style = window.text_style();

        let base_run = TextRun {
            len: content.len().max(1),
            font: style.font(),
            color: t.text_primary,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let display_text: SharedString = if content.is_empty() {
            " ".into()
        } else {
            content.clone()
        };

        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..base_run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(base_run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..base_run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..base_run.clone()
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![base_run.clone()]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let full_line =
            window
                .text_system()
                .shape_line(display_text.clone(), font_size, &runs, None);
        let line_height = window.line_height();

        if wrap {
            // ── Wrapped mode ────────────────────────────────────────
            let (lines, byte_offsets) = compute_wrapped_lines(
                &full_line,
                &content,
                bounds.size.width,
                font_size,
                &base_run,
                window,
            );

            // Cursor position in wrapped layout
            let cursor_line = byte_offsets
                .iter()
                .rposition(|&off| off <= cursor)
                .unwrap_or(0);
            let cursor_local = cursor - byte_offsets[cursor_line];
            let cursor_x = lines[cursor_line].x_for_index(cursor_local);

            let cursor_quad = if selected_range.is_empty() {
                Some(fill(
                    Bounds::new(
                        point(
                            bounds.left() + cursor_x,
                            bounds.top() + line_height * cursor_line as f32,
                        ),
                        size(px(1.), line_height),
                    ),
                    t.text_primary,
                ))
            } else {
                None
            };

            // Selection quads (may span multiple visual lines)
            let mut selections = Vec::new();
            if !selected_range.is_empty() {
                for (i, &offset) in byte_offsets.iter().enumerate() {
                    let line_end = if i + 1 < byte_offsets.len() {
                        byte_offsets[i + 1]
                    } else {
                        content.len()
                    };
                    // Does selection intersect this visual line?
                    let sel_start = selected_range.start.max(offset);
                    let sel_end = selected_range.end.min(line_end);
                    if sel_start < sel_end {
                        let x_start = lines[i].x_for_index(sel_start - offset);
                        let x_end = lines[i].x_for_index(sel_end - offset);
                        let y = bounds.top() + line_height * i as f32;
                        selections.push(fill(
                            Bounds::from_corners(
                                point(bounds.left() + x_start, y),
                                point(bounds.left() + x_end, y + line_height),
                            ),
                            rgba(0xffffff30),
                        ));
                    }
                }
            }

            // Update wrapped line count for next frame's layout
            let line_count = lines.len();
            self.input.update(cx, |input, _cx| {
                input.wrapped_line_count = line_count;
                input.scroll_offset = px(0.);
            });

            PrepaintState {
                scroll_offset: px(0.),
                lines,
                byte_offsets,
                cursor: cursor_quad,
                selections,
            }
        } else {
            // ── Single-line mode ────────────────────────────────────
            let cursor_pos = full_line.x_for_index(cursor);
            let visible_width = bounds.size.width;
            let padding = px(4.);
            if cursor_pos < scroll_offset + padding {
                scroll_offset = (cursor_pos - padding).max(px(0.));
            } else if cursor_pos > scroll_offset + visible_width - padding {
                scroll_offset = cursor_pos - visible_width + padding;
            }
            if scroll_offset < px(0.) {
                scroll_offset = px(0.);
            }

            self.input.update(cx, |input, _cx| {
                input.scroll_offset = scroll_offset;
            });

            let (selection, cursor_quad) = if selected_range.is_empty() {
                (
                    None,
                    Some(fill(
                        Bounds::new(
                            point(bounds.left() + cursor_pos - scroll_offset, bounds.top()),
                            size(px(1.), bounds.bottom() - bounds.top()),
                        ),
                        t.text_primary,
                    )),
                )
            } else {
                (
                    Some(fill(
                        Bounds::from_corners(
                            point(
                                bounds.left() + full_line.x_for_index(selected_range.start)
                                    - scroll_offset,
                                bounds.top(),
                            ),
                            point(
                                bounds.left() + full_line.x_for_index(selected_range.end)
                                    - scroll_offset,
                                bounds.bottom(),
                            ),
                        ),
                        rgba(0xffffff30),
                    )),
                    None,
                )
            };

            PrepaintState {
                scroll_offset,
                lines: vec![full_line],
                byte_offsets: vec![0],
                cursor: cursor_quad,
                selections: selection.into_iter().collect(),
            }
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
        cx: &mut gpui::App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        let wrap = self.input.read(cx).wrap;

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );

        for sel in prepaint.selections.drain(..) {
            window.paint_quad(sel);
        }

        let line_height = window.line_height();
        let lines: Vec<ShapedLine> = std::mem::take(&mut prepaint.lines);
        let byte_offsets: Vec<usize> = std::mem::take(&mut prepaint.byte_offsets);

        if wrap {
            for (i, line) in lines.iter().enumerate() {
                let origin = point(bounds.origin.x, bounds.origin.y + line_height * i as f32);
                line.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                    .unwrap();
            }
        } else {
            let line = &lines[0];
            line.paint(
                point(bounds.origin.x - prepaint.scroll_offset, bounds.origin.y),
                line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            )
            .unwrap();
        }

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        self.input.update(cx, |input, _cx| {
            if wrap {
                input.last_wrapped = Some(WrappedLayout {
                    lines,
                    byte_offsets,
                    bounds,
                    line_height,
                });
                input.last_layout = None;
                input.last_bounds = None;
            } else {
                input.last_layout = Some(lines.into_iter().next().unwrap());
                input.last_bounds = Some(bounds);
                input.last_wrapped = None;
            }
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme();
        div()
            .flex()
            .key_context("TextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .text_sm()
            .rounded(px(3.))
            .border_1()
            .border_color(t.accent)
            .bg(t.bg_elevated)
            .px(px(4.))
            .child(
                div()
                    .w_full()
                    .overflow_hidden()
                    .child(TextElement { input: cx.entity() }),
            )
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[gpui::test]
    fn test_initial_state(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("hello".to_string(), window, cx));
        _ = window.update(cx, |input, _window, _cx| {
            assert_eq!(input.text(), "hello");
            // new() selects all text
            assert_eq!(input.selected_range, 0..5);
        });
    }

    #[gpui::test]
    fn test_empty_initial_text(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new(String::new(), window, cx));
        _ = window.update(cx, |input, _window, _cx| {
            assert_eq!(input.text(), "");
            assert_eq!(input.selected_range, 0..0);
        });
    }

    #[gpui::test]
    fn test_backspace_deletes_selection(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("hello".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            // All text is selected (0..5), backspace should delete it all
            input.backspace(&Backspace, window, cx);
            assert_eq!(input.text(), "");
            assert_eq!(input.selected_range, 0..0);
        });
    }

    #[gpui::test]
    fn test_backspace_single_char(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("hello".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            // Collapse selection to end
            input.move_to(5, cx);
            input.backspace(&Backspace, window, cx);
            assert_eq!(input.text(), "hell");
            assert_eq!(input.selected_range, 4..4);
        });
    }

    #[gpui::test]
    fn test_delete_forward(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("hello".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            input.move_to(0, cx);
            input.delete(&Delete, window, cx);
            assert_eq!(input.text(), "ello");
            assert_eq!(input.selected_range, 0..0);
        });
    }

    #[gpui::test]
    fn test_left_collapses_selection_to_start(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("hello".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            // selected_range is 0..5
            input.left(&Left, window, cx);
            assert_eq!(input.selected_range, 0..0);
        });
    }

    #[gpui::test]
    fn test_right_collapses_selection_to_end(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("hello".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            input.right(&Right, window, cx);
            assert_eq!(input.selected_range, 5..5);
        });
    }

    #[gpui::test]
    fn test_select_all(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("hello".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            input.move_to(3, cx);
            assert_eq!(input.selected_range, 3..3);
            input.select_all(&SelectAll, window, cx);
            assert_eq!(input.selected_range, 0..5);
        });
    }

    #[gpui::test]
    fn test_home_and_end(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("hello".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            input.move_to(3, cx);
            input.end(&End, window, cx);
            assert_eq!(input.selected_range, 5..5);
            input.home(&Home, window, cx);
            assert_eq!(input.selected_range, 0..0);
        });
    }

    #[gpui::test]
    fn test_replace_text_in_range(cx: &mut gpui::TestAppContext) {
        let window =
            cx.add_window(|window, cx| TextInput::new("hello world".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            // Replace "world" (bytes 6..11) with "rust"
            input.replace_text_in_range(
                Some(6..11), // UTF-16 range (ASCII so same as byte range)
                "rust",
                window,
                cx,
            );
            assert_eq!(input.text(), "hello rust");
        });
    }

    #[gpui::test]
    fn test_select_left_right(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("abc".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            input.move_to(1, cx);
            input.select_right(&SelectRight, window, cx);
            assert_eq!(input.selected_range, 1..2);
            input.select_right(&SelectRight, window, cx);
            assert_eq!(input.selected_range, 1..3);
            input.select_left(&SelectLeft, window, cx);
            assert_eq!(input.selected_range, 1..2);
        });
    }

    #[gpui::test]
    fn test_cursor_navigation(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("abc".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            input.move_to(0, cx);
            input.right(&Right, window, cx);
            assert_eq!(input.selected_range, 1..1);
            input.right(&Right, window, cx);
            assert_eq!(input.selected_range, 2..2);
            input.left(&Left, window, cx);
            assert_eq!(input.selected_range, 1..1);
        });
    }

    #[gpui::test]
    fn test_right_at_end_stays(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("ab".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            input.move_to(2, cx);
            input.right(&Right, window, cx);
            assert_eq!(input.selected_range, 2..2);
        });
    }

    #[gpui::test]
    fn test_left_at_start_stays(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("ab".to_string(), window, cx));
        _ = window.update(cx, |input, window, cx| {
            input.move_to(0, cx);
            input.left(&Left, window, cx);
            assert_eq!(input.selected_range, 0..0);
        });
    }

    #[gpui::test]
    fn test_utf16_offset_roundtrip(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| {
            // Multi-byte char: '€' is 3 bytes UTF-8, 1 code unit UTF-16
            TextInput::new("a€b".to_string(), window, cx)
        });
        _ = window.update(cx, |input, _window, _cx| {
            // 'a' = byte 0, '€' = bytes 1..4, 'b' = byte 4
            assert_eq!(input.offset_to_utf16(0), 0); // start
            assert_eq!(input.offset_to_utf16(1), 1); // after 'a'
            assert_eq!(input.offset_to_utf16(4), 2); // after '€'
            assert_eq!(input.offset_to_utf16(5), 3); // after 'b'

            assert_eq!(input.offset_from_utf16(0), 0);
            assert_eq!(input.offset_from_utf16(1), 1);
            assert_eq!(input.offset_from_utf16(2), 4);
            assert_eq!(input.offset_from_utf16(3), 5);
        });
    }

    #[gpui::test]
    fn test_grapheme_boundaries(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("abc".to_string(), window, cx));
        _ = window.update(cx, |input, _window, _cx| {
            assert_eq!(input.previous_boundary(3), 2);
            assert_eq!(input.previous_boundary(2), 1);
            assert_eq!(input.previous_boundary(1), 0);
            assert_eq!(input.previous_boundary(0), 0);

            assert_eq!(input.next_boundary(0), 1);
            assert_eq!(input.next_boundary(1), 2);
            assert_eq!(input.next_boundary(2), 3);
            assert_eq!(input.next_boundary(3), 3);
        });
    }

    #[gpui::test]
    fn test_confirm_emits_trimmed_text(cx: &mut gpui::TestAppContext) {
        let window =
            cx.add_window(|window, cx| TextInput::new("  hello  ".to_string(), window, cx));
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();
        _ = window.update(cx, |_input, _window, cx| {
            let entity = cx.entity();
            cx.subscribe(
                &entity,
                move |_this, _emitter, event: &TextInputEvent, _cx| {
                    events_clone.lock().unwrap().push(event.clone());
                },
            )
            .detach();
        });
        _ = window.update(cx, |input, window, cx| {
            input.confirm(&Confirm, window, cx);
        });
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            TextInputEvent::Confirm(text) => assert_eq!(text, "hello"),
            other => panic!("expected Confirm, got {:?}", other),
        }
    }

    #[gpui::test]
    fn test_cancel_emits_event(cx: &mut gpui::TestAppContext) {
        let window = cx.add_window(|window, cx| TextInput::new("text".to_string(), window, cx));
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let events_clone = events.clone();
        _ = window.update(cx, |_input, _window, cx| {
            let entity = cx.entity();
            cx.subscribe(
                &entity,
                move |_this, _emitter, event: &TextInputEvent, _cx| {
                    events_clone.lock().unwrap().push(event.clone());
                },
            )
            .detach();
        });
        _ = window.update(cx, |input, window, cx| {
            input.cancel(&Cancel, window, cx);
        });
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], TextInputEvent::Cancel));
    }
}
