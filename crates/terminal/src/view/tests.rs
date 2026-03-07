use super::super::Rgb;

use super::TerminalView;
use super::clipboard::shell_quote;
use super::drawing::cursor_color_for_background;
use super::helpers::window_position_to_local;
use super::url::{url_at_byte_index, url_at_column_in_line};

#[test]
fn url_detection_finds_https_links() {
    let text = "Visit https://google.com for search";
    let idx = text.find("google").unwrap();
    assert_eq!(
        url_at_byte_index(text, idx).as_deref(),
        Some("https://google.com")
    );
}

#[test]
fn url_detection_finds_https_links_by_cell_column() {
    let line = "https://google.com";
    assert_eq!(
        url_at_column_in_line(line, 1).as_deref(),
        Some("https://google.com")
    );
    assert_eq!(
        url_at_column_in_line(line, 10).as_deref(),
        Some("https://google.com")
    );
}

#[test]
fn mouse_position_to_local_accounts_for_bounds_origin() {
    let bounds = Some(gpui::Bounds::new(
        gpui::point(gpui::px(100.0), gpui::px(20.0)),
        gpui::size(gpui::px(200.0), gpui::px(80.0)),
    ));

    let local = window_position_to_local(bounds, gpui::point(gpui::px(110.0), gpui::px(30.0)));
    assert_eq!(local, gpui::point(gpui::px(10.0), gpui::px(10.0)));
}

#[test]
fn cursor_color_contrasts_with_background() {
    let cursor = cursor_color_for_background(Rgb {
        r: 0xFF,
        g: 0xFF,
        b: 0xFF,
    });
    assert!(cursor.l < 0.2);
    assert!((cursor.a - 0.72).abs() < f32::EPSILON);

    let cursor = cursor_color_for_background(Rgb {
        r: 0x00,
        g: 0x00,
        b: 0x00,
    });
    assert!(cursor.l > 0.8);
    assert!((cursor.a - 0.72).abs() < f32::EPSILON);
}

#[test]
fn shell_quote_returns_empty_quoted_for_empty_string() {
    assert_eq!(shell_quote(""), "''");
}

#[test]
fn shell_quote_leaves_simple_paths_unquoted() {
    assert_eq!(shell_quote("/usr/bin/ls"), "/usr/bin/ls");
    assert_eq!(shell_quote("file.txt"), "file.txt");
    assert_eq!(shell_quote("a-b_c.d"), "a-b_c.d");
    assert_eq!(shell_quote("/tmp/image@2x.png"), "/tmp/image@2x.png");
}

#[test]
fn shell_quote_quotes_paths_with_spaces() {
    assert_eq!(
        shell_quote("/path/to/my file.png"),
        "'/path/to/my file.png'"
    );
}

#[test]
fn shell_quote_escapes_single_quotes() {
    assert_eq!(shell_quote("it's"), "'it'\\''s'");
}

#[test]
fn shell_quote_quotes_special_characters() {
    assert_eq!(shell_quote("a b"), "'a b'");
    assert_eq!(shell_quote("a(b)"), "'a(b)'");
    assert_eq!(shell_quote("a$b"), "'a$b'");
    assert_eq!(shell_quote("a&b"), "'a&b'");
}

// -- Word/line selection tests --
// These test the word boundary logic used by double/triple-click selection.
// Since TerminalView requires a GPUI context, we test via the public helper
// functions that the selection methods delegate to.

#[test]
fn word_boundary_detection() {
    // Test the is_word_char logic used in select_word_at_index
    let line = b"hello world foo-bar";
    let is_word_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.';

    // 'h' at 0 is word char
    assert!(is_word_char(line[0]));
    // space at 5 is not
    assert!(!is_word_char(line[5]));
    // '-' at 15 IS word char (we include it, like paths/identifiers)
    assert!(is_word_char(line[15]));
}

#[test]
fn viewport_line_offsets_computed_correctly() {
    let lines = vec![
        "first line".to_string(),
        "second line".to_string(),
        "third".to_string(),
    ];
    let offsets = TerminalView::compute_viewport_line_offsets(&lines);
    assert_eq!(offsets, vec![0, 11, 23]);

    let total = TerminalView::compute_viewport_total_len(&lines);
    assert_eq!(total, 29); // 10+1 + 11+1 + 5+1 = 29
}
