use gpui::KeyBinding;
use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeybindingEntry {
    pub key: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

pub fn default_keybindings() -> Vec<KeybindingEntry> {
    vec![
        // Standard macOS
        entry("cmd-q", "Quit"),
        entry("cmd-h", "HideApp"),
        entry("cmd-alt-h", "HideOtherApps"),
        entry("cmd-m", "MinimizeWindow"),
        entry("cmd-w", "CloseSideTab"),
        // App
        entry("escape", "CloseDiffView"),
        entry("cmd-t", "NewSideTab"),
        entry("cmd-b", "ToggleLeftSidebar"),
        entry("cmd-r", "ToggleRightSidebar"),
        entry("cmd-,", "OpenSettings"),
        entry("cmd-n", "NewSession"),
        entry("cmd-j", "ToggleBottomPanel"),
        entry("cmd-`", "FocusTerminal"),
        entry("cmd-/", "ToggleHelp"),
        entry("cmd-enter", "RunGitAction"),
        entry("cmd-shift-u", "UpdateFromMain"),
        entry("cmd-1", "SelectSession1"),
        entry("cmd-2", "SelectSession2"),
        entry("cmd-3", "SelectSession3"),
        entry("cmd-4", "SelectSession4"),
        entry("cmd-5", "SelectSession5"),
        entry("cmd-6", "SelectSession6"),
        entry("cmd-7", "SelectSession7"),
        entry("cmd-8", "SelectSession8"),
        entry("cmd-9", "SelectSession9"),
        // Terminal context
        terminal_entry("cmd-d", "SplitTerminalRight"),
        terminal_entry("cmd-shift-d", "SplitTerminalDown"),
        terminal_entry("cmd-shift-]", "NextTerminalTab"),
        terminal_entry("cmd-shift-[", "PrevTerminalTab"),
        terminal_entry("cmd-shift-enter", "ToggleZoomTerminalPane"),
        terminal_entry("cmd-alt-left", "ActivatePaneLeft"),
        terminal_entry("cmd-alt-right", "ActivatePaneRight"),
        terminal_entry("cmd-alt-up", "ActivatePaneUp"),
        terminal_entry("cmd-alt-down", "ActivatePaneDown"),
        terminal_entry("cmd-ctrl-left", "SplitTerminalLeft"),
        terminal_entry("cmd-ctrl-right", "SplitTerminalRight"),
        terminal_entry("cmd-ctrl-up", "SplitTerminalUp"),
        terminal_entry("cmd-ctrl-down", "SplitTerminalDown"),
    ]
}

fn entry(key: &str, action: &str) -> KeybindingEntry {
    KeybindingEntry {
        key: key.to_string(),
        action: action.to_string(),
        context: None,
    }
}

fn terminal_entry(key: &str, action: &str) -> KeybindingEntry {
    KeybindingEntry {
        key: key.to_string(),
        action: action.to_string(),
        context: Some("Terminal".to_string()),
    }
}

pub fn to_gpui_keybindings(entries: &[KeybindingEntry]) -> Vec<KeyBinding> {
    entries
        .iter()
        .filter_map(|e| make_binding(&e.key, &e.action, e.context.as_deref()))
        .collect()
}

fn make_binding(key: &str, action: &str, context: Option<&str>) -> Option<KeyBinding> {
    Some(match action {
        "Quit" => KeyBinding::new(key, Quit, context),
        "HideApp" => KeyBinding::new(key, HideApp, context),
        "HideOtherApps" => KeyBinding::new(key, HideOtherApps, context),
        "MinimizeWindow" => KeyBinding::new(key, MinimizeWindow, context),
        "CloseSideTab" => KeyBinding::new(key, CloseSideTab, context),
        "CloseDiffView" => KeyBinding::new(key, CloseDiffView, context),
        "NewSideTab" => KeyBinding::new(key, NewSideTab, context),
        "ToggleLeftSidebar" => KeyBinding::new(key, ToggleLeftSidebar, context),
        "ToggleRightSidebar" => KeyBinding::new(key, ToggleRightSidebar, context),
        "RefreshGitStatus" => KeyBinding::new(key, RefreshGitStatus, context),
        "OpenSettings" => KeyBinding::new(key, OpenSettings, context),
        "NewSession" => KeyBinding::new(key, NewSession, context),
        "ToggleBottomPanel" => KeyBinding::new(key, ToggleBottomPanel, context),
        "FocusTerminal" => KeyBinding::new(key, FocusTerminal, context),
        "ToggleHelp" => KeyBinding::new(key, ToggleHelp, context),
        "RunGitAction" => KeyBinding::new(key, RunGitAction, context),
        "UpdateFromMain" => KeyBinding::new(key, UpdateFromMain, context),
        "AbortRebase" => KeyBinding::new(key, AbortRebase, context),
        "CopyConflictPrompt" => KeyBinding::new(key, CopyConflictPrompt, context),
        "SelectSession1" => KeyBinding::new(key, SelectSession1, context),
        "SelectSession2" => KeyBinding::new(key, SelectSession2, context),
        "SelectSession3" => KeyBinding::new(key, SelectSession3, context),
        "SelectSession4" => KeyBinding::new(key, SelectSession4, context),
        "SelectSession5" => KeyBinding::new(key, SelectSession5, context),
        "SelectSession6" => KeyBinding::new(key, SelectSession6, context),
        "SelectSession7" => KeyBinding::new(key, SelectSession7, context),
        "SelectSession8" => KeyBinding::new(key, SelectSession8, context),
        "SelectSession9" => KeyBinding::new(key, SelectSession9, context),
        "SplitTerminalRight" => KeyBinding::new(key, SplitTerminalRight, context),
        "SplitTerminalLeft" => KeyBinding::new(key, SplitTerminalLeft, context),
        "SplitTerminalDown" => KeyBinding::new(key, SplitTerminalDown, context),
        "SplitTerminalUp" => KeyBinding::new(key, SplitTerminalUp, context),
        "ActivatePaneLeft" => KeyBinding::new(key, ActivatePaneLeft, context),
        "ActivatePaneRight" => KeyBinding::new(key, ActivatePaneRight, context),
        "ActivatePaneUp" => KeyBinding::new(key, ActivatePaneUp, context),
        "ActivatePaneDown" => KeyBinding::new(key, ActivatePaneDown, context),
        "ToggleZoomTerminalPane" => KeyBinding::new(key, ToggleZoomTerminalPane, context),
        "NextTerminalTab" => KeyBinding::new(key, NextTerminalTab, context),
        "PrevTerminalTab" => KeyBinding::new(key, PrevTerminalTab, context),
        _ => return None,
    })
}

/// Format a key string for display (e.g. "cmd-shift-b" → "Cmd + Shift + B").
pub fn format_key_display(key: &str) -> String {
    key.split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => {
                    let upper = c.to_uppercase().to_string();
                    upper + chars.as_str()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" + ")
}
