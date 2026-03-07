use gpui::Focusable;

use crate::pane_group::{PaneGroup, SplitDirection};
use crate::runtime::TerminalTab;
use ui::empty_state;
use ui::prelude::*;

use crate::WorkspaceView;

impl WorkspaceView {
    /// Add a brand new terminal tab to the bottom panel.
    pub(crate) fn on_add_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(working_dir) = self.bottom_panel_working_dir() else {
            return;
        };
        let Some(rt) = self.selected_session_runtime_mut() else {
            return;
        };

        match terminal::spawn_terminal(window, cx, &working_dir) {
            Ok(view) => {
                let focus = view.focus_handle(cx);
                let tab = TerminalTab {
                    pane_group: PaneGroup::new(view.clone()),
                    active_pane: Some(view),
                    zoomed_pane_group: None,
                };
                rt.tabs.push(tab);
                rt.active_tab_index = rt.tabs.len() - 1;
                focus.focus(window, cx);
            }
            Err(error) => {
                self.state.flash_error = Some(format!("Failed to create terminal: {error}"));
            }
        }

        if self.state.bottom_panel_collapsed {
            self.state.bottom_panel_collapsed = false;
        }
        cx.notify();
    }

    /// Close the active terminal pane within the active tab.
    /// If the tab has no more panes, remove the tab.
    /// If that was the last tab, create a fresh one.
    pub(crate) fn on_close_active_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rt) = self.selected_session_runtime_mut() else {
            return;
        };
        let Some(tab) = rt.active_tab_mut() else {
            return;
        };
        let Some(active) = tab.active_pane.clone() else {
            return;
        };

        if tab.pane_group.remove(&active) {
            // Still has panes — select the first remaining one
            let first = tab.pane_group.first_pane();
            let focus = first.focus_handle(cx);
            tab.active_pane = Some(first);
            tab.zoomed_pane_group = None;
            focus.focus(window, cx);
        } else {
            // Was the only pane — remove the entire tab
            let idx = rt.active_tab_index;
            rt.tabs.remove(idx);
            if rt.tabs.is_empty() {
                // Never leave empty — create a fresh tab
                self.on_add_terminal(window, cx);
                return;
            }
            if rt.active_tab_index >= rt.tabs.len() {
                rt.active_tab_index = rt.tabs.len() - 1;
            }
            // Focus the new active tab's terminal
            if let Some(tab) = rt.active_tab() {
                if let Some(pane) = &tab.active_pane {
                    let focus = pane.focus_handle(cx);
                    focus.focus(window, cx);
                }
            }
        }
        cx.notify();
    }

    /// Close all tabs except the active one (and collapse its splits).
    pub(crate) fn on_close_other_tabs(&mut self, cx: &mut Context<Self>) {
        let Some(rt) = self.selected_session_runtime_mut() else {
            return;
        };
        if rt.tabs.is_empty() {
            return;
        }

        let active = rt.tabs.remove(rt.active_tab_index);
        rt.tabs = vec![active];
        rt.active_tab_index = 0;
        cx.notify();
    }

    /// Close all terminal tabs.
    pub(crate) fn on_close_all_tabs(&mut self, cx: &mut Context<Self>) {
        let Some(rt) = self.selected_session_runtime_mut() else {
            return;
        };
        rt.tabs.clear();
        rt.active_tab_index = 0;
        cx.notify();
    }

    /// Split the active terminal pane in the given direction (within the active tab).
    pub(crate) fn on_split_terminal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        direction: SplitDirection,
    ) {
        let Some(working_dir) = self.bottom_panel_working_dir() else {
            return;
        };
        let Some(rt) = self.selected_session_runtime_mut() else {
            return;
        };

        // If no tabs exist, create one
        if rt.tabs.is_empty() {
            match terminal::spawn_terminal(window, cx, &working_dir) {
                Ok(view) => {
                    let focus = view.focus_handle(cx);
                    rt.tabs.push(TerminalTab {
                        pane_group: PaneGroup::new(view.clone()),
                        active_pane: Some(view),
                        zoomed_pane_group: None,
                    });
                    rt.active_tab_index = 0;
                    focus.focus(window, cx);
                }
                Err(e) => {
                    self.state.flash_error = Some(format!("Failed to create terminal: {e}"));
                }
            }
            if self.state.bottom_panel_collapsed {
                self.state.bottom_panel_collapsed = false;
            }
            cx.notify();
            return;
        }

        let Some(tab) = rt.active_tab_mut() else {
            return;
        };

        // Exit zoom when splitting
        if let Some(zoomed) = tab.zoomed_pane_group.take() {
            tab.pane_group = zoomed;
        }

        match terminal::spawn_terminal(window, cx, &working_dir) {
            Ok(view) => {
                let focus = view.focus_handle(cx);
                let anchor = tab
                    .active_pane
                    .clone()
                    .unwrap_or_else(|| tab.pane_group.first_pane());
                tab.pane_group.split(&anchor, view.clone(), direction);
                tab.active_pane = Some(view);
                focus.focus(window, cx);
            }
            Err(e) => {
                self.state.flash_error = Some(format!("Failed to create terminal: {e}"));
            }
        }

        if self.state.bottom_panel_collapsed {
            self.state.bottom_panel_collapsed = false;
        }
        cx.notify();
    }

    /// Navigate focus to an adjacent split pane within the active tab.
    pub(crate) fn on_activate_pane_in_direction(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        direction: SplitDirection,
    ) {
        let Some(rt) = self.selected_session_runtime() else {
            return;
        };
        let Some(tab) = rt.active_tab() else {
            return;
        };
        let Some(active) = &tab.active_pane else {
            return;
        };
        let Some(target) = tab.pane_group.find_pane_in_direction(active, direction) else {
            return;
        };
        let focus = target.focus_handle(cx);
        let Some(rt) = self.selected_session_runtime_mut() else {
            return;
        };
        let Some(tab) = rt.active_tab_mut() else {
            return;
        };
        tab.active_pane = Some(target);
        focus.focus(window, cx);
        cx.notify();
    }

    /// Zoom: temporarily show only the active pane, stashing the full layout.
    pub(crate) fn on_toggle_zoom_terminal_pane(&mut self, cx: &mut Context<Self>) {
        let Some(rt) = self.selected_session_runtime_mut() else {
            return;
        };
        let Some(tab) = rt.active_tab_mut() else {
            return;
        };

        if tab.zoomed_pane_group.is_some() {
            // Unzoom
            let full = tab.zoomed_pane_group.take();
            if let Some(full) = full {
                tab.pane_group = full;
            }
        } else if let Some(active) = tab.active_pane.clone() {
            if tab.pane_group.is_split() {
                let old = std::mem::replace(&mut tab.pane_group, PaneGroup::new(active));
                tab.zoomed_pane_group = Some(old);
            }
        }
        cx.notify();
    }

    /// Cycle through split panes within the active tab.
    pub(crate) fn on_next_terminal_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cycle_pane(1, window, cx);
    }

    pub(crate) fn on_prev_terminal_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cycle_pane(-1, window, cx);
    }

    fn cycle_pane(&mut self, delta: i32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rt) = self.selected_session_runtime() else {
            return;
        };
        let Some(tab) = rt.active_tab() else {
            return;
        };
        let panes = tab.pane_group.panes();
        if panes.len() < 2 {
            return;
        }

        let current_idx = tab
            .active_pane
            .as_ref()
            .and_then(|active| {
                panes
                    .iter()
                    .position(|p| p.entity_id() == active.entity_id())
            })
            .unwrap_or(0);

        let len = panes.len() as i32;
        let next_idx = ((current_idx as i32 + delta).rem_euclid(len)) as usize;
        let target = panes[next_idx].clone();
        let focus = target.focus_handle(cx);

        let Some(rt) = self.selected_session_runtime_mut() else {
            return;
        };
        let Some(tab) = rt.active_tab_mut() else {
            return;
        };
        tab.active_pane = Some(target);
        focus.focus(window, cx);
        cx.notify();
    }

    /// Toggle focus between the main terminal and the bottom panel.
    pub(crate) fn on_focus_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.bottom_panel_collapsed {
            self.state.bottom_panel_collapsed = false;
            cx.notify();
        }

        let Some(rt) = self.selected_session_runtime() else {
            return;
        };

        let side_handle = rt
            .active_tab()
            .and_then(|tab| tab.active_pane.as_ref())
            .map(|p| p.focus_handle(cx));

        let main_handle = rt.main_terminal.as_ref().map(|m| m.focus_handle(cx));

        let side_focused = side_handle.as_ref().is_some_and(|h| h.is_focused(window));

        if side_focused {
            if let Some(h) = main_handle {
                h.focus(window, cx);
            }
        } else if let Some(h) = side_handle {
            h.focus(window, cx);
        }
    }

    /// Update active_pane based on which terminal currently has focus.
    pub(crate) fn track_focused_terminal_pane(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(rt) = self.selected_session_runtime() else {
            return;
        };
        let Some(tab) = rt.active_tab() else {
            return;
        };

        // Check if the already-active pane is focused — fast path
        if let Some(active) = &tab.active_pane {
            if active.focus_handle(cx).is_focused(window) {
                return;
            }
        }

        // Check all panes in the active tab
        for pane in tab.pane_group.panes() {
            if pane.focus_handle(cx).is_focused(window) {
                if tab
                    .active_pane
                    .as_ref()
                    .is_some_and(|a| a.entity_id() == pane.entity_id())
                {
                    return;
                }
                let Some(rt) = self.selected_session_runtime_mut() else {
                    return;
                };
                let Some(tab) = rt.active_tab_mut() else {
                    return;
                };
                tab.active_pane = Some(pane);
                cx.notify();
                return;
            }
        }
    }

    // ── Rendering ────────────────────────────────────────────────────

    pub(crate) fn render_bottom_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let t = theme();
        let Some(rt) = self.selected_session_runtime() else {
            return div().into_any_element();
        };

        let tab_bar = self.render_terminal_tab_bar(rt, cx);
        let content = self.render_terminal_content(rt);

        v_flex()
            .h(px(self.state.bottom_panel_height))
            .flex_shrink_0()
            .overflow_hidden()
            .border_t_1()
            .border_color(t.border_default)
            .child(tab_bar)
            .child(content)
            .into_any_element()
    }

    fn render_terminal_content(&self, rt: &crate::runtime::SessionRuntime) -> AnyElement {
        if let Some(tab) = rt.active_tab() {
            // Only show the blue focus indicator when the tab has splits
            let active_id = if tab.pane_group.is_split() {
                tab.active_pane.as_ref().map(|p| p.entity_id())
            } else {
                None
            };
            return tab.pane_group.render(active_id);
        }
        empty_state("Click + to add a terminal.").into_any_element()
    }

    fn render_terminal_tab_bar(
        &self,
        rt: &crate::runtime::SessionRuntime,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let t = theme();
        let has_tabs = !rt.tabs.is_empty();
        let is_zoomed = rt
            .active_tab()
            .is_some_and(|tab| tab.zoomed_pane_group.is_some());
        let is_split = rt.active_tab().is_some_and(|tab| tab.pane_group.is_split()) || is_zoomed;

        h_flex()
            .px_2()
            .py(px(5.))
            .flex_shrink_0()
            .bg(t.bg_surface)
            .justify_between()
            .child(
                // Left: tabs
                h_flex()
                    .gap_0p5()
                    .overflow_x_hidden()
                    .children(rt.tabs.iter().enumerate().map(|(ix, _tab)| {
                        let is_active = ix == rt.active_tab_index;
                        div()
                            .id(("terminal-tab", ix))
                            .cursor_pointer()
                            .px_2()
                            .py(px(2.))
                            .text_xs()
                            .flex_shrink_0()
                            .rounded_sm()
                            .text_color(if is_active {
                                t.text_primary
                            } else {
                                t.text_muted
                            })
                            .when(is_active, |el| el.bg(t.bg_base))
                            .hover(|el| el.bg(t.bg_elevated_hover))
                            .child(format!("Terminal {}", ix + 1))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                if let Some(rt) = this.selected_session_runtime_mut() {
                                    rt.active_tab_index = ix;
                                }
                                cx.notify();
                            }))
                    })),
            )
            .child(
                // Right: action buttons
                h_flex()
                    .gap_1()
                    .flex_shrink_0()
                    .when(is_split, |el| {
                        el.child(
                            IconButton::new(
                                "zoom-pane-btn",
                                if is_zoomed {
                                    IconName::Minimize
                                } else {
                                    IconName::Maximize
                                },
                            )
                            .icon_size(px(14.))
                            .on_click(cx.listener(
                                |this, _event, _window, cx| {
                                    this.on_toggle_zoom_terminal_pane(cx);
                                },
                            )),
                        )
                    })
                    .when(has_tabs, |el| {
                        el.child(
                            IconButton::new("split-right-btn", IconName::Columns)
                                .icon_size(px(14.))
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.on_split_terminal(window, cx, SplitDirection::Right);
                                })),
                        )
                    })
                    .when(has_tabs, |el| {
                        el.child(
                            IconButton::new("close-pane-btn", IconName::X)
                                .icon_size(px(14.))
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.on_close_active_terminal(window, cx);
                                })),
                        )
                    })
                    .child(
                        IconButton::new("add-tab-btn", IconName::Plus)
                            .icon_size(px(14.))
                            .on_click(cx.listener(|this, _event, window, cx| {
                                this.on_add_terminal(window, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    // ── Helpers ──────────────────────────────────────────────────────

    fn bottom_panel_working_dir(&self) -> Option<std::path::PathBuf> {
        let repo = self.state.selected_repo.as_ref()?;
        let session_id = self.state.selected_session.as_ref()?;
        Some(self.worktree_or_repo(repo, session_id))
    }
}
