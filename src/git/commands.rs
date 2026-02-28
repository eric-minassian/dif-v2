use std::path::Path;
use std::process::Command;

use super::repo::get_branch_name;
use super::{gh, git};

pub fn commit_selected(worktree: &Path, files: &[String], message: &str) -> Result<(), String> {
    let mut cmd = Command::new(git());
    cmd.arg("-C").arg(worktree).arg("add").arg("--");
    for file in files {
        cmd.arg(file);
    }
    let output = cmd.output().map_err(|e| format!("git add failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let output = Command::new(git())
        .arg("-C")
        .arg(worktree)
        .args(["commit", "-m", message])
        .output()
        .map_err(|e| format!("git commit failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn amend_selected(worktree: &Path, files: &[String]) -> Result<(), String> {
    let mut cmd = Command::new(git());
    cmd.arg("-C").arg(worktree).arg("add").arg("--");
    for file in files {
        cmd.arg(file);
    }
    let output = cmd.output().map_err(|e| format!("git add failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let output = Command::new(git())
        .arg("-C")
        .arg(worktree)
        .args(["commit", "--amend", "--no-edit"])
        .output()
        .map_err(|e| format!("git commit --amend failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git commit --amend failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn push(worktree: &Path) -> Result<(), String> {
    let branch = get_branch_name(worktree)?;

    let output = Command::new(git())
        .arg("-C")
        .arg(worktree)
        .args(["push", "-u", "origin", &branch])
        .output()
        .map_err(|e| format!("git push failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git push failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn force_push(worktree: &Path) -> Result<(), String> {
    let branch = get_branch_name(worktree)?;

    let output = Command::new(git())
        .arg("-C")
        .arg(worktree)
        .args(["push", "--force-with-lease", "-u", "origin", &branch])
        .output()
        .map_err(|e| format!("git push failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git push failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn create_pr(worktree: &Path, title: &str) -> Result<String, String> {
    let output = Command::new(gh())
        .current_dir(worktree)
        .args(["pr", "create", "--title", title, "--body", "", "--fill"])
        .output()
        .map_err(|e| format!("gh pr create failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "gh pr create failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn merge_pr_rebase(worktree: &Path) -> Result<(), String> {
    let output = Command::new(gh())
        .current_dir(worktree)
        .args(["pr", "merge", "--rebase"])
        .output()
        .map_err(|e| format!("gh pr merge failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "gh pr merge failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    // Delete the remote branch separately — we skip local branch deletion
    // because the branch is checked out in this worktree and `main` is
    // checked out in the main worktree, so `gh --delete-branch` would fail.
    let branch = get_branch_name(worktree).unwrap_or_default();
    if !branch.is_empty() {
        let _ = Command::new(git())
            .arg("-C")
            .arg(worktree)
            .args(["push", "origin", "--delete", &branch])
            .output();
    }

    Ok(())
}

pub fn enable_auto_merge(worktree: &Path) -> Result<(), String> {
    let output = Command::new(gh())
        .current_dir(worktree)
        .args(["pr", "merge", "--auto", "--rebase"])
        .output()
        .map_err(|e| format!("gh pr merge --auto failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "gh pr merge --auto failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}

pub fn disable_auto_merge(worktree: &Path) -> Result<(), String> {
    let output = Command::new(gh())
        .current_dir(worktree)
        .args(["pr", "merge", "--disable-auto"])
        .output()
        .map_err(|e| format!("gh pr merge --disable-auto failed: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "gh pr merge --disable-auto failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    Ok(())
}
