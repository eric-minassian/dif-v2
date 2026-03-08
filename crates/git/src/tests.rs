use std::collections::HashMap;

use pretty_assertions::assert_eq;

use super::types::SplitLineKind;

use super::diff::build_split_diff;
use super::repo::expand_tilde;
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

    let has_insert = data
        .lines
        .iter()
        .any(|l| l.kind == SplitLineKind::Insert || l.kind == SplitLineKind::Replace);
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

// ── Tilde expansion tests ───────────────────────────────────────────────────

#[test]
fn expand_tilde_home() {
    use std::path::Path;

    let expanded = expand_tilde(Path::new("~"));
    // Should not be "~" anymore (unless HOME is unset)
    if std::env::var_os("HOME").is_some() {
        assert_ne!(expanded, Path::new("~"));
        assert!(expanded.is_absolute());
    }
}

#[test]
fn expand_tilde_with_subpath() {
    use std::path::Path;

    let expanded = expand_tilde(Path::new("~/projects/test"));
    if let Some(home) = std::env::var_os("HOME") {
        let expected = std::path::PathBuf::from(home).join("projects/test");
        assert_eq!(expanded, expected);
    }
}

#[test]
fn expand_tilde_no_tilde() {
    use std::path::Path;

    let path = Path::new("/absolute/path");
    let expanded = expand_tilde(path);
    assert_eq!(expanded, path);
}

// ── Status parsing edge cases ───────────────────────────────────────────────

#[test]
fn parses_empty_status() {
    let changes = parse_status_entries(b"");
    assert_eq!(changes.len(), 0);
}

#[test]
fn parses_staged_and_unstaged_modifications() {
    let bytes = b"MM src/main.rs\0";
    let changes = parse_status_entries(bytes);
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].status_code, "MM");
    assert_eq!(changes[0].path, "src/main.rs");
}

#[test]
fn parses_deleted_files() {
    let bytes = b" D removed.rs\0D  staged_delete.rs\0";
    let changes = parse_status_entries(bytes);
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].status_code, " D");
    assert_eq!(changes[1].status_code, "D ");
}

// ── Diff edge cases ────────────────────────────────────────────────────────

#[test]
fn split_diff_identical_content() {
    let content = "same\ncontent\n";
    let data = build_split_diff("file.rs", content, content);
    assert!(data.lines.iter().all(|l| l.kind == SplitLineKind::Equal));
    assert_eq!(data.additions, 0);
    assert_eq!(data.deletions, 0);
}

#[test]
fn split_diff_both_empty() {
    let data = build_split_diff("empty.rs", "", "");
    assert_eq!(data.lines.len(), 0);
    assert_eq!(data.additions, 0);
    assert_eq!(data.deletions, 0);
}

#[test]
fn split_diff_large_content() {
    let old: String = (0..100).map(|i| format!("line {i}\n")).collect();
    let mut new = old.clone();
    new.push_str("extra line\n");
    let data = build_split_diff("large.rs", &old, &new);
    assert!(data.additions >= 1);
}

// ── Git integration tests (require git on PATH) ─────────────────────────────

mod integration {
    use std::process::Command;

    fn git_init(dir: &std::path::Path) -> bool {
        Command::new("git")
            .arg("init")
            .arg(dir)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn git(dir: &std::path::Path, args: &[&str]) -> bool {
        Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[test]
    fn is_valid_repo_on_real_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!super::super::is_valid_repo(dir.path()));
        assert!(git_init(dir.path()));
        assert!(super::super::is_valid_repo(dir.path()));
    }

    #[test]
    fn collect_changes_on_clean_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git_init(dir.path()));
        git(dir.path(), &["config", "user.email", "test@test.com"]);
        git(dir.path(), &["config", "user.name", "Test"]);

        // Create initial commit so repo is not empty
        std::fs::write(dir.path().join("README.md"), "# test").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "init"]);

        let changes = super::super::collect_changes(dir.path()).unwrap();
        assert!(changes.is_empty(), "clean repo should have no changes");
    }

    #[test]
    fn collect_changes_detects_new_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git_init(dir.path()));
        git(dir.path(), &["config", "user.email", "test@test.com"]);
        git(dir.path(), &["config", "user.name", "Test"]);

        std::fs::write(dir.path().join("README.md"), "# test").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "init"]);

        // Create untracked file
        std::fs::write(dir.path().join("new_file.rs"), "fn main() {}").unwrap();

        let changes = super::super::collect_changes(dir.path()).unwrap();
        assert!(!changes.is_empty(), "should detect untracked file");
        assert!(
            changes.iter().any(|c| c.path == "new_file.rs"),
            "should contain new_file.rs"
        );
    }

    #[test]
    fn collect_changes_detects_modification() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git_init(dir.path()));
        git(dir.path(), &["config", "user.email", "test@test.com"]);
        git(dir.path(), &["config", "user.name", "Test"]);

        std::fs::write(dir.path().join("file.txt"), "original").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "init"]);

        // Modify tracked file
        std::fs::write(dir.path().join("file.txt"), "modified").unwrap();

        let changes = super::super::collect_changes(dir.path()).unwrap();
        assert!(
            changes.iter().any(|c| c.path == "file.txt"),
            "should detect modified file"
        );
    }

    #[test]
    fn get_branch_name_on_fresh_repo() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git_init(dir.path()));
        git(dir.path(), &["config", "user.email", "test@test.com"]);
        git(dir.path(), &["config", "user.name", "Test"]);

        std::fs::write(dir.path().join("init.txt"), "init").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-m", "init"]);

        let branch = super::super::get_branch_name(dir.path()).unwrap();
        // Default branch is usually "main" or "master"
        assert!(
            branch == "main" || branch == "master",
            "unexpected branch: {branch}"
        );
    }

    #[test]
    fn normalize_repo_path_resolves_toplevel() {
        let dir = tempfile::tempdir().unwrap();
        assert!(git_init(dir.path()));

        // Create a subdirectory
        let sub = dir.path().join("subdir");
        std::fs::create_dir(&sub).unwrap();

        let normalized = super::super::normalize_repo_path(&sub).unwrap();
        // Should resolve to the repo root, not the subdirectory
        assert_eq!(
            normalized,
            dir.path().canonicalize().unwrap(),
            "should resolve to repo root"
        );
    }
}
