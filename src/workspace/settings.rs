use std::path::PathBuf;

use crate::prelude::*;
use crate::state::UpdateStatus;
use crate::text_input::{TextInput, TextInputEvent};
use crate::updater;

use super::{SettingsEdit, WorkspaceView};

impl WorkspaceView {
    pub(crate) fn on_toggle_conventional_commits(
        &mut self,
        repo_root: PathBuf,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        {
            project.settings.enforce_conventional_commits =
                !project.settings.enforce_conventional_commits;
        }
        self.persist_config();
        cx.notify();
    }

    pub(crate) fn on_open_settings(&mut self, cx: &mut Context<Self>) {
        self.state.viewing_settings = true;
        cx.notify();
    }

    pub(crate) fn on_close_settings(&mut self, cx: &mut Context<Self>) {
        self.state.viewing_settings = false;
        self.settings_input = None;
        cx.notify();
    }

    pub(crate) fn on_add_init_command(
        &mut self,
        repo_root: PathBuf,
        command: String,
        cx: &mut Context<Self>,
    ) {
        if command.is_empty() {
            return;
        }
        if let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        {
            project.settings.workspace_init_commands.push(command);
        }
        self.settings_input = None;
        self.persist_config();
        cx.notify();
    }

    pub(crate) fn on_remove_init_command(
        &mut self,
        repo_root: PathBuf,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(project) = self
            .state
            .config
            .projects
            .iter_mut()
            .find(|p| p.repo_root == repo_root)
        {
            if index < project.settings.workspace_init_commands.len() {
                project.settings.workspace_init_commands.remove(index);
            }
        }
        self.persist_config();
        cx.notify();
    }

    pub(crate) fn on_start_add_init_command(
        &mut self,
        repo_root: PathBuf,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| TextInput::new(String::new(), window, cx));
        let repo = repo_root.clone();
        let event_sub = cx.subscribe(&input, move |this, _input, event, cx| match event {
            TextInputEvent::Confirm(text) => {
                this.on_add_init_command(repo.clone(), text.clone(), cx);
            }
            TextInputEvent::Cancel => {
                this.settings_input = None;
                cx.notify();
            }
        });
        self.settings_input = Some(SettingsEdit {
            repo_root,
            input,
            _event_sub: event_sub,
        });
        cx.notify();
    }

    pub(crate) fn render_settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();

        let header = h_flex()
            .justify_between()
            .px_3()
            .py_2()
            .bg(t.bg_panel)
            .border_b_1()
            .border_color(t.border_default)
            .child(
                div()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_sm()
                    .child("Settings"),
            )
            .child(
                div()
                    .id("close-settings")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_xs()
                    .bg(t.bg_elevated)
                    .text_color(t.text_muted)
                    .hover(|style| style.bg(t.bg_elevated_hover).text_color(t.text_primary))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.on_close_settings(cx);
                    }))
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(Icon::new(IconName::X).size(px(12.)).color(Color::Muted))
                    .child("Esc"),
            );

        let mut content = v_flex()
            .id("settings-content")
            .flex_1()
            .min_h_0()
            .overflow_scroll()
            .p_4()
            .gap_4();

        // About / Version section
        let mut about_section = v_flex()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(t.text_muted)
                    .child("ABOUT"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(t.text_primary)
                    .child(format!("Dif v{}", updater::current_version())),
            );

        match &self.state.update_status {
            UpdateStatus::Available { version, download_url } => {
                let url = download_url.clone();
                let ver = version.clone();
                about_section = about_section.child(
                    h_flex()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(t.accent_green)
                                .child(format!("{ver} available")),
                        )
                        .child(
                            div()
                                .id("settings-update-btn")
                                .cursor_pointer()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_xs()
                                .bg(t.bg_elevated)
                                .text_color(t.accent_green)
                                .hover(|style| style.bg(t.bg_elevated_hover))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.on_start_update(url.clone(), window, cx);
                                }))
                                .child("Install update"),
                        ),
                );
            }
            UpdateStatus::Updating => {
                about_section = about_section.child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("Updating..."),
                );
            }
            UpdateStatus::Error(msg) => {
                about_section = about_section
                    .child(
                        div()
                            .text_xs()
                            .text_color(t.accent_red)
                            .child(format!("Update failed: {msg}")),
                    )
                    .child(
                        div()
                            .id("settings-retry-btn")
                            .cursor_pointer()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .bg(t.bg_elevated)
                            .text_color(t.text_secondary)
                            .hover(|style| style.bg(t.bg_elevated_hover))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.spawn_update_check(window, cx);
                            }))
                            .child("Retry"),
                    );
            }
            UpdateStatus::Checking => {
                about_section = about_section.child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("Checking for updates..."),
                );
            }
            UpdateStatus::Idle => {
                about_section = about_section.child(
                    div()
                        .id("settings-check-btn")
                        .cursor_pointer()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .text_xs()
                        .bg(t.bg_elevated)
                        .text_color(t.text_secondary)
                        .hover(|style| style.bg(t.bg_elevated_hover))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.spawn_update_check(window, cx);
                        }))
                        .child("Check for updates"),
                );
            }
        }

        content = content.child(about_section);

        // Per-project settings
        for project in &self.state.config.projects {
            let repo_root = project.repo_root.clone();

            let mut project_section = v_flex()
                .gap_2()
                .p_3()
                .rounded_md()
                .bg(t.bg_elevated)
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(t.text_primary)
                        .child(project.display_name.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_dim)
                        .child(project.repo_root.display().to_string()),
                );

            // Conventional commits toggle
            {
                let toggle_repo = repo_root.clone();
                let is_enabled = project.settings.enforce_conventional_commits;
                let toggle_id = gpui::ElementId::Name(
                    format!("cc-toggle-{}", project.display_name).into(),
                );
                project_section = project_section.child(
                    h_flex()
                        .mt_2()
                        .justify_between()
                        .child(
                            v_flex()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(t.text_secondary)
                                        .child("Enforce conventional commits"),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(t.text_dim)
                                        .child("Reject commits not matching type[(scope)]: description"),
                                ),
                        )
                        .child(
                            div()
                                .id(toggle_id)
                                .cursor_pointer()
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .text_xs()
                                .bg(if is_enabled { t.accent_blue } else { t.bg_surface })
                                .text_color(if is_enabled { t.bg_panel } else { t.text_muted })
                                .hover(|style| style.bg(t.bg_elevated_hover))
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.on_toggle_conventional_commits(toggle_repo.clone(), cx);
                                }))
                                .child(if is_enabled { "On" } else { "Off" }),
                        ),
                );
            }

            // Workspace init commands subsection
            project_section = project_section
                .child(
                    div()
                        .mt_2()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(t.text_secondary)
                        .child("Workspace init commands"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(t.text_dim)
                        .child("Run after worktree creation. Available env vars: $DIF_WORKTREE_DIR, $DIF_REPO_DIR"),
                );

            if project.settings.workspace_init_commands.is_empty() {
                project_section = project_section.child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("No init commands configured."),
                );
            } else {
                for (i, cmd) in project.settings.workspace_init_commands.iter().enumerate() {
                    let remove_repo = repo_root.clone();
                    let cmd_index = i;
                    let cmd_row_id =
                        gpui::ElementId::Name(format!("cmd-{}-{}", project.display_name, i).into());

                    project_section = project_section.child(
                        h_flex()
                            .id(cmd_row_id)
                            .group("cmd-row")
                            .justify_between()
                            .px_2()
                            .py_1()
                            .rounded(px(3.))
                            .bg(t.bg_surface)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_xs()
                                    .text_color(t.text_secondary)
                                    .overflow_hidden()
                                    .child(cmd.clone()),
                            )
                            .child(
                                div()
                                    .id("remove-cmd-btn")
                                    .cursor_pointer()
                                    .px_1()
                                    .text_xs()
                                    .text_color(t.text_dim)
                                    .invisible()
                                    .group_hover("cmd-row", |style| style.visible())
                                    .hover(|style| style.text_color(t.accent_red))
                                    .on_click(cx.listener(move |this, _event, _window, cx| {
                                        this.on_remove_init_command(
                                            remove_repo.clone(),
                                            cmd_index,
                                            cx,
                                        );
                                    }))
                                    .child(Icon::new(IconName::X).size(px(14.)).color(Color::Dim)),
                            ),
                    );
                }
            }

            // Add command input or button
            let is_editing = self
                .settings_input
                .as_ref()
                .is_some_and(|s| s.repo_root == repo_root);

            if is_editing {
                let input = self.settings_input.as_ref().unwrap().input.clone();
                project_section = project_section.child(
                    h_flex()
                        .mt_1()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(input)),
                );
            } else {
                let add_repo = repo_root.clone();
                let add_btn_id = gpui::ElementId::Name(
                    format!("add-cmd-{}", project.display_name).into(),
                );
                project_section = project_section.child(
                    div()
                        .id(add_btn_id)
                        .mt_1()
                        .cursor_pointer()
                        .text_xs()
                        .text_color(t.text_dim)
                        .hover(|style| style.text_color(t.text_primary))
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.on_start_add_init_command(add_repo.clone(), window, cx);
                        }))
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(Icon::new(IconName::Plus).size(px(12.)).color(Color::Dim))
                        .child("Add command"),
                );
            }

            content = content.child(project_section);
        }

        v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(header)
            .child(content)
            .into_any_element()
    }
}
