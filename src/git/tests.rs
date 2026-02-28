use std::collections::HashMap;

use crate::state::SplitLineKind;

use super::diff::build_split_diff;
use super::status::{parse_numstat_output, parse_status_entries};

#[test]
fn parses_standard_status_rows() {
    let bytes = b" M src/main.rs\0A  src/lib.rs\0?? Cargo.lock\0";
    let changes = parse_status_entries(bytes);

    assert_eq!(changes.len(), 3);
    assert_eq!(changes[0].status_code, " M");
    assert_eq!(changes[0].path, "src/main.rs");
    assert_eq!(changes[2].status_code, "??");
    assert_eq!(changes[2].path, "Cargo.lock");
}

#[test]
fn parses_rename_rows() {
    let bytes = b"R  old.rs\0new.rs\0";
    let changes = parse_status_entries(bytes);

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "old.rs -> new.rs");
    assert_eq!(changes[0].sort_key, "new.rs");
}

#[test]
fn parses_numstat_output() {
    let mut map = HashMap::new();
    let bytes = b"10\t5\tsrc/main.rs\0-\t-\tbinary.png\0";
    parse_numstat_output(bytes, &mut map);

    assert_eq!(map.get("src/main.rs"), Some(&(Some(10), Some(5))));
    assert_eq!(map.get("binary.png"), Some(&(None, None)));
}

#[test]
fn split_diff_replace() {
    let old = "line one\nold line\nline three\n";
    let new = "line one\nnew line\nline three\n";
    let data = build_split_diff("file.rs", old, new);

    assert_eq!(data.file_path, "file.rs");
    assert_eq!(data.lines.len(), 3);

    assert_eq!(data.lines[0].kind, SplitLineKind::Equal);
    assert_eq!(data.lines[0].old_text, "line one");
    assert_eq!(data.lines[0].new_text, "line one");

    assert_eq!(data.lines[1].kind, SplitLineKind::Replace);
    assert_eq!(data.lines[1].old_text, "old line");
    assert_eq!(data.lines[1].new_text, "new line");

    assert_eq!(data.lines[2].kind, SplitLineKind::Equal);
}

#[test]
fn split_diff_insert_delete() {
    let old = "context\nremoved\nend\n";
    let new = "context\nadded one\nadded two\nend\n";
    let data = build_split_diff("file.rs", old, new);

    assert!(data.additions >= 2);
    assert!(data.deletions >= 1);

    let has_insert = data.lines.iter().any(|l| {
        l.kind == SplitLineKind::Insert || l.kind == SplitLineKind::Replace
    });
    assert!(has_insert);
}

#[test]
fn split_diff_new_file() {
    let data = build_split_diff("new.rs", "", "hello\nworld\n");

    assert_eq!(data.lines.len(), 2);
    assert!(data.lines.iter().all(|l| l.kind == SplitLineKind::Insert));
    assert_eq!(data.additions, 2);
    assert_eq!(data.deletions, 0);
}

#[test]
fn split_diff_deleted_file() {
    let data = build_split_diff("gone.rs", "goodbye\nworld\n", "");

    assert_eq!(data.lines.len(), 2);
    assert!(data.lines.iter().all(|l| l.kind == SplitLineKind::Delete));
    assert_eq!(data.additions, 0);
    assert_eq!(data.deletions, 2);
}
