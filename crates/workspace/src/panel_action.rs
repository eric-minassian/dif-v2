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
