use git::BranchStatus;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PanelAction {
    Commit,
    Amend,
    CreatePR,
    Rebase,
    CloseSession,
    None,
}

pub(crate) fn derive_panel_action(
    has_changes: bool,
    staged_count: usize,
    status: &BranchStatus,
) -> PanelAction {
    if status.pr_merged {
        return PanelAction::CloseSession;
    }
    if has_changes && staged_count > 0 && status.commits_ahead == 0 {
        return PanelAction::Commit;
    }
    if has_changes && staged_count > 0 && status.commits_ahead > 0 {
        return PanelAction::Amend;
    }
    if !has_changes && status.commits_ahead > 0 && status.pr_url.is_none() {
        return PanelAction::CreatePR;
    }
    if !has_changes && status.pr_url.is_some() && !status.pr_merged {
        return PanelAction::Rebase;
    }
    PanelAction::None
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn status() -> BranchStatus {
        BranchStatus::default()
    }

    // ── PR merged → CloseSession (terminal state) ──────────────────────

    #[test]
    fn pr_merged_returns_close_session() {
        let s = BranchStatus {
            pr_merged: true,
            ..status()
        };
        assert_eq!(derive_panel_action(false, 0, &s), PanelAction::CloseSession);
    }

    #[test]
    fn pr_merged_overrides_all_other_state() {
        let s = BranchStatus {
            pr_merged: true,
            commits_ahead: 5,
            pr_url: Some("https://github.com/pr/1".into()),
            ..status()
        };
        // Even with changes and staged files, merged PR wins
        assert_eq!(derive_panel_action(true, 3, &s), PanelAction::CloseSession);
    }

    // ── Commit: has_changes + staged > 0 + commits_ahead == 0 ──────────

    #[test]
    fn commit_when_staged_changes_no_prior_commits() {
        let s = BranchStatus {
            commits_ahead: 0,
            ..status()
        };
        assert_eq!(derive_panel_action(true, 2, &s), PanelAction::Commit);
    }

    // ── Amend: has_changes + staged > 0 + commits_ahead > 0 ────────────

    #[test]
    fn amend_when_staged_changes_with_prior_commits() {
        let s = BranchStatus {
            commits_ahead: 1,
            ..status()
        };
        assert_eq!(derive_panel_action(true, 2, &s), PanelAction::Amend);
    }

    #[test]
    fn amend_with_many_commits_ahead() {
        let s = BranchStatus {
            commits_ahead: 10,
            ..status()
        };
        assert_eq!(derive_panel_action(true, 1, &s), PanelAction::Amend);
    }

    // ── CreatePR: no changes + commits_ahead > 0 + no PR ───────────────

    #[test]
    fn create_pr_when_clean_with_commits_and_no_pr() {
        let s = BranchStatus {
            commits_ahead: 3,
            pr_url: None,
            ..status()
        };
        assert_eq!(derive_panel_action(false, 0, &s), PanelAction::CreatePR);
    }

    // ── Rebase: no changes + has PR + not merged ────────────────────────

    #[test]
    fn rebase_when_clean_with_open_pr() {
        let s = BranchStatus {
            pr_url: Some("https://github.com/pr/1".into()),
            pr_merged: false,
            ..status()
        };
        assert_eq!(derive_panel_action(false, 0, &s), PanelAction::Rebase);
    }

    #[test]
    fn rebase_even_with_commits_ahead_and_pr() {
        let s = BranchStatus {
            commits_ahead: 2,
            pr_url: Some("https://github.com/pr/1".into()),
            pr_merged: false,
            ..status()
        };
        // has_changes=false, commits_ahead>0, pr_url=Some → CreatePR check fails
        // because pr_url.is_none() is false, so falls through to Rebase
        assert_eq!(derive_panel_action(false, 0, &s), PanelAction::Rebase);
    }

    // ── None: no actionable state ──────────────────────────────────────

    #[test]
    fn none_when_completely_clean() {
        assert_eq!(derive_panel_action(false, 0, &status()), PanelAction::None);
    }

    #[test]
    fn none_when_changes_but_nothing_staged() {
        let s = BranchStatus {
            commits_ahead: 0,
            ..status()
        };
        assert_eq!(derive_panel_action(true, 0, &s), PanelAction::None);
    }

    #[test]
    fn none_when_changes_unstaged_with_prior_commits() {
        let s = BranchStatus {
            commits_ahead: 3,
            ..status()
        };
        assert_eq!(derive_panel_action(true, 0, &s), PanelAction::None);
    }
}
