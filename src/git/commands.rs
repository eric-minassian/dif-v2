use std::path::Path;

use super::repo::get_branch_name;
use super::{run_gh, run_git, run_git_raw};

/// Stage the given files in the worktree.
fn stage_files(worktree: &Path, files: &[String]) -> Result<(), String> {
    let mut args: Vec<&str> = vec!["add", "--"];
    args.extend(files.iter().map(String::as_str));
    run_git_raw(worktree, &args)?;
    Ok(())
}

pub fn commit_selected(worktree: &Path, files: &[String], message: &str) -> Result<(), String> {
    stage_files(worktree, files)?;
    run_git(worktree, &["commit", "-m", message])?;
    Ok(())
}

pub fn amend_selected(worktree: &Path, files: &[String]) -> Result<(), String> {
    stage_files(worktree, files)?;
    run_git(worktree, &["commit", "--amend", "--no-edit"])?;
    Ok(())
}

pub fn push(worktree: &Path) -> Result<(), String> {
    let branch = get_branch_name(worktree)?;
    run_git(worktree, &["push", "-u", "origin", &branch])?;
    Ok(())
}

pub fn force_push(worktree: &Path) -> Result<(), String> {
    let branch = get_branch_name(worktree)?;
    run_git(worktree, &["push", "--force-with-lease", "-u", "origin", &branch])?;
    Ok(())
}

pub fn create_pr(worktree: &Path, title: &str) -> Result<String, String> {
    run_gh(worktree, &["pr", "create", "--title", title, "--body", "", "--fill"])
}

pub fn merge_pr_rebase(worktree: &Path) -> Result<(), String> {
    run_gh(worktree, &["pr", "merge", "--rebase"])?;

    // Delete the remote branch separately — we skip local branch deletion
    // because the branch is checked out in this worktree and `main` is
    // checked out in the main worktree, so `gh --delete-branch` would fail.
    if let Ok(branch) = get_branch_name(worktree) {
        let _ = run_git(worktree, &["push", "origin", "--delete", &branch]);
    }

    Ok(())
}

pub fn enable_auto_merge(worktree: &Path) -> Result<(), String> {
    run_gh(worktree, &["pr", "merge", "--auto", "--rebase"])?;
    Ok(())
}

pub fn disable_auto_merge(worktree: &Path) -> Result<(), String> {
    run_gh(worktree, &["pr", "merge", "--disable-auto"])?;
    Ok(())
}
