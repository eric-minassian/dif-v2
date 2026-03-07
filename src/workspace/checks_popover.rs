use gpui::Hsla;

use crate::prelude::*;
use crate::state::{BranchStatus, CheckBucket, CiCheck};

use super::WorkspaceView;

impl WorkspaceView {
    pub(crate) fn render_checks_status_icon(
        &self,
        branch_status: &BranchStatus,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let checks = &branch_status.checks;

        if checks.is_empty() {
            return div().into_any_element();
        }

        let fail_count = checks
            .iter()
            .filter(|c| c.bucket == CheckBucket::Fail)
            .count();
        let pending_count = checks
            .iter()
            .filter(|c| c.bucket == CheckBucket::Pending)
            .count();

        let status_icon = if fail_count > 0 {
            Icon::new(IconName::X).size(px(14.)).color(Color::Red).into_any_element()
        } else if pending_count > 0 {
            Icon::new(IconName::CircleDot).size(px(14.)).color(Color::Yellow).into_any_element()
        } else {
            Icon::new(IconName::Check).size(px(14.)).color(Color::Green).into_any_element()
        };

        div()
            .id("checks-status-icon")
            .flex_shrink_0()
            .cursor_pointer()
            .hover(|s| s.opacity(0.7))
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.on_toggle_checks_popover(cx);
            }))
            .child(status_icon)
            .into_any_element()
    }

    pub(crate) fn render_checks_popover(
        &self,
        branch_status: &BranchStatus,
        _cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();

        let checks = &branch_status.checks;
        let mut sorted_checks: Vec<&CiCheck> = checks.iter().collect();
        sorted_checks.sort_by_key(|c| match c.bucket {
            CheckBucket::Fail => 0,
            CheckBucket::Pending => 1,
            CheckBucket::Pass => 2,
            CheckBucket::Skipping => 3,
            CheckBucket::Cancel => 4,
        });

        let pr_url = branch_status.pr_url.clone();

        let mut popover = div()
            .id("checks-popover")
            .absolute()
            .top(px(38.))
            .right(px(4.))
            .w(px(280.))
            .rounded_md()
            .border_1()
            .border_color(t.border_default)
            .bg(gpui::rgb(0x2d2d2d))
            .shadow_lg()
            .overflow_hidden()
            .pb_1()
            .occlude();

        // PR state row at top (if PR exists)
        if let Some(url) = &pr_url {
            let state_str = branch_status
                .pr_state
                .as_deref()
                .unwrap_or("OPEN")
                .to_uppercase();
            let is_merged = state_str == "MERGED";
            let is_closed = state_str == "CLOSED";

            let (badge_bg, badge_text_color): (Hsla, Hsla) = if is_merged {
                (gpui::rgba(0xa371f730).into(), t.accent_purple)
            } else if is_closed {
                (gpui::rgba(0xef292930).into(), t.accent_red)
            } else {
                (gpui::rgba(0x8ae23430).into(), t.accent_green)
            };

            let state_label = match state_str.as_str() {
                "MERGED" => "Merged",
                "CLOSED" => "Closed",
                _ => "Open",
            };

            let pr_label = match branch_status.pr_number {
                Some(n) => format!("#{n}"),
                None => "PR".to_string(),
            };

            let url_owned = url.clone();

            popover = popover.child(
                h_flex()
                    .id("popover-pr-row")
                    .gap(px(8.))
                    .px_2()
                    .py(px(6.))
                    .border_b_1()
                    .border_color(t.border_subtle)
                    .cursor_pointer()
                    .hover(|s| s.bg(gpui::rgba(0xffffff08)))
                    .on_click(move |_event, _window, _cx| {
                        let _ = std::process::Command::new("open")
                            .arg(&url_owned)
                            .spawn();
                    })
                    // State badge pill
                    .child(
                        div()
                            .px(px(8.))
                            .py(px(2.))
                            .rounded(px(12.))
                            .bg(badge_bg)
                            .text_xs()
                            .text_color(badge_text_color)
                            .child(state_label.to_string()),
                    )
                    // PR number
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(t.text_primary)
                            .child(pr_label),
                    )
                    .child(div().flex_1())
                    // External link icon
                    .child(
                        div().flex_shrink_0().child(
                            Icon::new(IconName::ExternalLink).svg()
                                .size(px(12.))
                                .text_color(t.text_dim),
                        ),
                    ),
            );
        }

        // Check rows
        popover = popover.children(
            sorted_checks
                .iter()
                .enumerate()
                .map(|(i, check)| Self::render_popover_check_row(check, i)),
        );

        popover.into_any_element()
    }

    fn render_popover_check_row(check: &CiCheck, index: usize) -> AnyElement {
        let t = theme();

        let (status_icon, status_color) = match check.bucket {
            CheckBucket::Pass => (
                Icon::new(IconName::Check).size(px(12.)).into_any_element(),
                t.accent_green,
            ),
            CheckBucket::Fail => (
                Icon::new(IconName::X).size(px(12.)).into_any_element(),
                t.accent_red,
            ),
            CheckBucket::Pending => (
                Icon::new(IconName::CircleDot).size(px(12.)).into_any_element(),
                t.accent_yellow,
            ),
            CheckBucket::Skipping | CheckBucket::Cancel => (
                Icon::new(IconName::Minus).size(px(12.)).into_any_element(),
                t.text_dim,
            ),
        };

        let row_id = gpui::ElementId::Name(format!("popover-check-{index}").into());
        let link = check.link.clone();
        let has_link = link.is_some();

        h_flex()
            .id(row_id)
            .group("popover-check-row")
            .gap(px(6.))
            .px_2()
            .py(px(4.))
            .when(has_link, |el| {
                el.cursor_pointer()
                    .hover(|s| s.bg(gpui::rgba(0xffffff08)))
            })
            .on_click(move |_event, _window, _cx| {
                if let Some(url) = &link {
                    let _ = std::process::Command::new("open").arg(url).spawn();
                }
            })
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(status_color)
                    .child(status_icon),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_xs()
                    .text_color(t.text_secondary)
                    .child(check.name.clone()),
            )
            .when(has_link, |el| {
                el.child(
                    div()
                        .flex_shrink_0()
                        .text_xs()
                        .text_color(t.text_dim)
                        .invisible()
                        .group_hover("popover-check-row", |s| s.visible())
                        .child("Details"),
                )
            })
            .into_any_element()
    }

    pub(crate) fn on_toggle_checks_popover(&mut self, cx: &mut Context<Self>) {
        self.state.checks_popover_open = !self.state.checks_popover_open;
        cx.notify();
    }

    pub(crate) fn on_close_checks_popover(&mut self, cx: &mut Context<Self>) {
        self.state.checks_popover_open = false;
        cx.notify();
    }
}
