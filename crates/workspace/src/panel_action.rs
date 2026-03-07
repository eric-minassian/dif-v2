use gpui::Hsla;

use git::{BranchStatus, CheckBucket, RepoCapabilities};
use ui::prelude::*;
use crate::runtime::ActionPhase;

use crate::WorkspaceView;

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

impl WorkspaceView {
    pub(crate) fn render_header_action_button(
        &self,
        action: &PanelAction,
        phase: &ActionPhase,
        branch_status: &BranchStatus,
        repo_capabilities: &RepoCapabilities,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let is_busy = matches!(phase, ActionPhase::Working(_));

        // For the Rebase action, decide whether to show "Rebase & Merge" or "Auto Merge"
        if *action == PanelAction::Rebase {
            let checks = &branch_status.checks;
            let all_checks_pass = !checks.is_empty()
                && checks
                    .iter()
                    .all(|c| matches!(c.bucket, CheckBucket::Pass | CheckBucket::Skipping));
            let auto_merge_enabled = branch_status.auto_merge_enabled;

            // If auto-merge is already enabled on the PR, show toggle to disable
            if auto_merge_enabled {
                return div()
                    .id("action-auto-merge")
                    .w_full()
                    .px_2()
                    .py(px(4.))
                    .rounded_md()
                    .text_xs()
                    .text_center()
                    .bg(t.bg_elevated)
                    .text_color(t.accent_purple)
                    .when(is_busy, |el| el.opacity(0.5))
                    .when(!is_busy, |el| {
                        el.cursor_pointer()
                            .hover(|style| style.bg(t.bg_elevated_hover))
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.on_toggle_pr_auto_merge(window, cx);
                            }))
                    })
                    .child("Auto Merge")
                    .into_any_element();
            }

            // Checks still pending/failing: show auto-merge toggle if repo supports it
            if !all_checks_pass && repo_capabilities.auto_merge_allowed {
                return div()
                    .id("action-auto-merge")
                    .w_full()
                    .px_2()
                    .py(px(4.))
                    .rounded_md()
                    .text_xs()
                    .text_center()
                    .bg(t.bg_elevated)
                    .text_color(t.accent_green)
                    .when(is_busy, |el| el.opacity(0.5))
                    .when(!is_busy, |el| {
                        el.cursor_pointer()
                            .hover(|style| style.bg(t.bg_elevated_hover))
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.on_toggle_pr_auto_merge(window, cx);
                            }))
                    })
                    .child("Auto Merge")
                    .into_any_element();
            }

            // All checks pass (or no checks): show immediate merge if repo allows rebase
            if !repo_capabilities.rebase_merge_allowed {
                return div().into_any_element();
            }
        }

        let (button_id, label, text_color): (&str, &str, Hsla) = match action {
            PanelAction::Commit => ("action-commit", "Commit", t.text_primary),
            PanelAction::Amend => ("action-amend", "Amend", t.text_primary),
            PanelAction::CreatePR => ("action-create-pr", "Create PR", t.accent_blue),
            PanelAction::Rebase => ("action-rebase", "Rebase & Merge", t.text_primary),
            PanelAction::CloseSession => ("action-close", "Close Session", t.accent_red),
            PanelAction::None => return div().into_any_element(),
        };

        let action_clone = action.clone();

        div()
            .id(button_id)
            .w_full()
            .px_2()
            .py(px(4.))
            .rounded_md()
            .text_xs()
            .text_center()
            .bg(t.bg_elevated)
            .text_color(text_color)
            .when(is_busy, |el| el.opacity(0.5))
            .when(!is_busy, |el| {
                el.cursor_pointer()
                    .hover(|style| style.bg(t.bg_elevated_hover))
                    .on_click(cx.listener(move |this, _event, window, cx| match action_clone {
                        PanelAction::Commit => this.on_commit(window, cx),
                        PanelAction::Amend => this.on_amend(window, cx),
                        PanelAction::CreatePR => this.on_create_pr(window, cx),
                        PanelAction::Rebase => this.on_rebase(window, cx),
                        PanelAction::CloseSession => this.on_close_session(window, cx),
                        PanelAction::None => {}
                    }))
            })
            .child(label)
            .into_any_element()
    }

    pub(crate) fn render_action_or_status(
        &self,
        action: &PanelAction,
        phase: &ActionPhase,
        button: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();

        match phase {
            ActionPhase::Working(label) => div()
                .w_full()
                .px_2()
                .py(px(4.))
                .rounded_md()
                .text_xs()
                .text_center()
                .bg(t.bg_elevated)
                .text_color(t.text_muted)
                .opacity(0.7)
                .child(label.clone())
                .into_any_element(),
            ActionPhase::Error(msg) => h_flex()
                .id("action-error")
                .w_full()
                .px_2()
                .py(px(4.))
                .rounded_md()
                .text_xs()
                .bg(t.error_bg)
                .justify_between()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(t.accent_red)
                        .child(msg.clone()),
                )
                .child(
                    div()
                        .id("dismiss-action-err")
                        .text_color(t.text_dim)
                        .cursor_pointer()
                        .hover(|s| s.text_color(t.text_primary))
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.on_dismiss_action_error(cx);
                        }))
                        .child(Icon::new(IconName::X).size(px(14.)).color(Color::Dim)),
                )
                .into_any_element(),
            ActionPhase::Idle => {
                if *action != PanelAction::None {
                    button
                } else {
                    div().into_any_element()
                }
            }
        }
    }

    pub(crate) fn render_header_pr_link(&self, branch_status: &BranchStatus) -> AnyElement {
        let t = theme();

        let Some(url) = branch_status.pr_url.clone() else {
            return div().into_any_element();
        };

        let pr_label = match branch_status.pr_number {
            Some(n) => format!("#{n}"),
            None => "PR".to_string(),
        };

        h_flex()
            .id("header-pr-link")
            .gap(px(2.))
            .cursor_pointer()
            .hover(|s| s.opacity(0.7))
            .on_click(move |_event, _window, _cx| {
                let _ = std::process::Command::new("open").arg(&url).spawn();
            })
            .child(
                div()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child(pr_label),
            )
            .child(
                Icon::new(IconName::ExternalLink)
                    .size(px(12.))
                    .color(Color::Dim),
            )
            .into_any_element()
    }
}
