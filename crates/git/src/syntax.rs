use std::sync::LazyLock;

use syntect::highlighting::{self, Highlighter, ThemeSet};
use syntect::parsing::SyntaxSet;

use super::types::SyntaxRun;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

static HIGHLIGHT_THEME: LazyLock<highlighting::Theme> = LazyLock::new(|| {
    let ts = ThemeSet::load_defaults();
    ts.themes["base16-eighties.dark"].clone()
});

fn syntect_color_to_hsla(c: highlighting::Color) -> gpui::Hsla {
    let rgba = gpui::Rgba {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: c.a as f32 / 255.0,
    };
    rgba.into()
}

/// Highlight the full content of a file, returning per-line syntax runs.
///
/// Each inner `Vec<SyntaxRun>` corresponds to one line of the input
/// (split by `\n`). Line indices are 0-based and match the order of
/// lines produced by `str::lines()` / `similar::TextDiff::from_lines()`.
pub(crate) fn highlight_lines(content: &str, file_path: &str) -> Vec<Vec<SyntaxRun>> {
    let extension = file_path.rsplit('.').next().unwrap_or("");
    let syntax = SYNTAX_SET
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());
    let highlighter = Highlighter::new(&HIGHLIGHT_THEME);
    let mut parse_state = syntect::parsing::ParseState::new(syntax);
    let mut highlight_state =
        syntect::highlighting::HighlightState::new(&highlighter, syntect::parsing::ScopeStack::new());
    let mut result = Vec::new();
    let mut line_buf = String::new();

    for line in content.lines() {
        line_buf.clear();
        line_buf.push_str(line);
        line_buf.push('\n');
        let ops = parse_state
            .parse_line(&line_buf, &SYNTAX_SET)
            .unwrap_or_default();
        let regions: Vec<(highlighting::Style, &str)> =
            syntect::highlighting::HighlightIterator::new(
                &mut highlight_state,
                &ops,
                &line_buf,
                &highlighter,
            )
            .collect();

        let mut runs = Vec::new();
        for (style, text) in &regions {
            let len = text.trim_end_matches('\n').len();
            if len == 0 {
                continue;
            }
            runs.push(SyntaxRun {
                len,
                color: syntect_color_to_hsla(style.foreground),
                bold: style
                    .font_style
                    .contains(highlighting::FontStyle::BOLD),
                italic: style
                    .font_style
                    .contains(highlighting::FontStyle::ITALIC),
            });
        }
        result.push(runs);
    }

    result
}
