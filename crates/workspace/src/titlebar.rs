use crate::ui_state::UpdateStatus;
use ui::prelude::*;

use crate::WorkspaceView;

impl WorkspaceView {
    fn render_update_indicator(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let t = theme();
        match &self.state.update_status {
            UpdateStatus::Available {
                version,
                download_url,
            } => {
                let url = download_url.clone();
                Some(
                    h_flex()
                        .id("update-available")
                        .gap_1()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .text_color(t.accent_green)
                        .hover(|style| style.cursor_pointer().bg(t.hover_overlay))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.on_start_update(url.clone(), window, cx);
                        }))
                        .child(format!("Update {version}"))
                        .into_any_element(),
                )
            }
            UpdateStatus::Updating => Some(
                div()
                    .px_2()
                    .py_1()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child("Updating...")
                    .into_any_element(),
            ),
            UpdateStatus::Error(msg) => {
                let _ = msg;
                Some(
                    div()
                        .id("update-error")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .text_color(t.accent_red)
                        .hover(|style| style.cursor_pointer().bg(t.hover_overlay))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.spawn_update_check(window, cx);
                        }))
                        .child("Update failed - retry")
                        .into_any_element(),
                )
            }
            UpdateStatus::Idle | UpdateStatus::Checking => None,
        }
    }

    pub(crate) fn render_titlebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let left_collapsed = self.state.left_sidebar_collapsed;
        let right_collapsed = self.state.right_sidebar_collapsed;

        let project_name = self.state.selected_repo.as_ref().and_then(|repo| {
            self.state
                .config
                .projects
                .iter()
                .find(|p| &p.repo_root == repo)
                .map(|p| p.display_name.clone())
        });
        let branch_name = self
            .selected_project_runtime()
            .and_then(|rt| rt.branch_status.branch_name.clone());

        h_flex()
            .h(px(36.))
            .flex_shrink_0()
            .justify_between()
            .bg(t.bg_titlebar)
            .border_b_1()
            .border_color(t.border_subtle)
            // Left side: traffic light padding + sidebar toggle
            .child(
                h_flex().pl(px(78.)).child(
                    div()
                        .id("toggle-left-sidebar")
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|style| style.bg(t.hover_overlay))
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.on_toggle_left_sidebar(cx);
                        }))
                        .child(Icon::new(IconName::PanelLeft).size(px(14.)).color(
                            if left_collapsed {
                                t.text_dim
                            } else {
                                t.text_muted
                            },
                        )),
                ),
            )
            // Center: project name / branch
            .child(
                h_flex()
                    .flex_1()
                    .justify_center()
                    .gap(px(4.))
                    .overflow_hidden()
                    .text_xs()
                    .when_some(project_name, |el, name| {
                        el.child(div().text_color(t.text_secondary).child(name))
                    })
                    .when_some(branch_name, |el, branch| {
                        el.child(div().text_color(t.text_dim).child("/"))
                            .child(div().text_color(t.text_muted).child(branch))
                    }),
            )
            // Right side: update indicator + sidebar toggle
            .child(
                h_flex()
                    .gap_1()
                    .pr_2()
                    .children(self.render_update_indicator(cx))
                    .child(
                        div()
                            .id("toggle-right-sidebar")
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .hover(|style| style.bg(t.hover_overlay))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.on_toggle_right_sidebar(cx);
                            }))
                            .child(Icon::new(IconName::PanelRight).size(px(14.)).color(
                                if right_collapsed {
                                    t.text_dim
                                } else {
                                    t.text_muted
                                },
                            )),
                    ),
            )
            .into_any_element()
    }
}
