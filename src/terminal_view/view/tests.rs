use super::super::Rgb;

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
    assert_eq!(shell_quote("/path/to/my file.png"), "'/path/to/my file.png'");
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
