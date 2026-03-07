# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is Dif

Dif is a native macOS Git GUI built with Rust and Zed's [GPUI](https://github.com/zed-industries/zed) framework. It manages multiple Git repositories and sessions (branches/worktrees), each with an integrated terminal, a file-changes sidebar, split diff viewer, and GitHub PR workflow (commit, push, create PR, auto-merge, rebase merge) via `gh` CLI.

## Build Commands

```bash
cargo check           # Fast type-check (CI uses this)
cargo build           # Debug build
cargo build --release # Release build
cargo run             # Run debug build
cargo test            # Run all tests
cargo test git::      # Run tests in the git module
cargo test terminal_view::  # Run tests in terminal_view
./scripts/bundle.sh   # Build release + create Dif.app bundle
./scripts/bundle.sh --install  # Build + install to /Applications
```

Note: `.cargo/config.toml` sets a custom target dir (`/Users/eric/.cargo-targets/dif`) and a `ZIG` env var. CI overrides both (`CARGO_TARGET_DIR=target`, `ZIG=zig`).

## Architecture

### Layers

**App entry** (`main.rs` → `app.rs`): Creates a GPUI `Application`, registers global keybindings, opens a single window containing `WorkspaceView`.

**WorkspaceView** (`workspace/mod.rs`): The root view. Owns all application state (`AppState`) and renders the three-panel layout (left sidebar, center, right sidebar). Action handlers are registered in `register_actions()`. Sub-files in `workspace/` each implement a slice of WorkspaceView:
- `left_panel.rs` / `right_panel.rs` / `sidebar.rs` — panel rendering
- `git_actions.rs` — commit, amend, push, create PR, rebase merge, auto-merge
- `git_poll.rs` — async polling loop (2s interval) for git status and branch info
- `session.rs` — session lifecycle (create, rename, delete, activate, worktrees)
- `diff_view.rs` — split diff rendering with syntax highlighting
- `changes_list.rs` — staged/unstaged file list
- `panel_action.rs` — derives which action button to show based on git state
- `settings.rs` / `help.rs` / `titlebar.rs` / `tab_bar.rs` / `checks_popover.rs`

**State** (`state/`): Pure data types split across:
- `config.rs` — persisted config (`AppConfig`, `SavedProject`, `SavedSession`, `ProjectSettings`)
- `runtime.rs` — ephemeral runtime state (`AppState`, `ProjectRuntime`, `SessionRuntime`, `TerminalTab`)
- `git.rs` — git data types (`GitSnapshot`, `BranchStatus`, `DiffData`, `RepoCapabilities`, `CiCheck`)
- `ui.rs` — UI enums (`ResizingSidebar`, `UpdateStatus`)

**Git** (`git/`): Shells out to `git` and `gh` CLI. Resolves binary paths at startup to handle macOS GUI PATH issues. Key public functions: `collect_changes`, `collect_branch_status`, `check_repo_capabilities`, `commit_selected`, `push`, `force_push`, `create_pr`, `merge_pr_rebase`, `create_worktree`, `remove_worktree`, `compute_file_diff`.

**Terminal** (`terminal.rs` + `terminal_view/`): Spawns real PTY sessions using `portable-pty` + `alacritty_terminal` for VT parsing. `terminal.rs` handles PTY spawn and 16ms polling. `terminal_view/` contains the `TerminalView` GPUI view with custom `Element` rendering, input handling, mouse selection, clipboard, URL detection, and viewport management.

**Shared UI** (`components.rs`, `icons.rs`, `theme.rs`, `text_input.rs`, `picker.rs`):
- `theme.rs` — static `Theme` struct (dark theme, `LazyLock`), accessed via `theme()`
- `components.rs` — reusable builders: `button()`, `section_header()`, `panel()`, `empty_state()`
- `icons.rs` — SVG icon helpers wrapping `assets/icons/`
- `text_input.rs` — `TextInput` view implementing `EntityInputHandler` and `Focusable`
- `picker.rs` — macOS `osascript` folder picker

### GPUI Reference: Zed Codebase

Zed is the canonical large-scale GPUI codebase and should be used as the reference for best practices. When implementing GPUI patterns (views, elements, actions, focus, rendering), consult the Zed source for idiomatic usage:

- **General GPUI patterns**: https://github.com/zed-industries/zed/tree/main/crates
- **Terminal implementation** (most relevant to our `terminal_view/`): https://github.com/zed-industries/zed/tree/main/crates/terminal_view/src

Prefer Zed's patterns over inventing new ones — if Zed has an established way to handle focus, keybindings, element rendering, testing, or async work, follow it.

### GPUI Patterns Used

- All views with focus implement `Focusable` trait
- Actions defined via `actions!()` macro, registered on elements via `.on_action(cx.listener(...))`
- Keybindings scoped by key context strings (`"TextInput"`, `"Terminal"`)
- Async work: `window.spawn(cx, ...)` for UI-updating tasks, `cx.background_executor().spawn(...)` for pure computation
- Subscriptions stored as `_event_sub`/`_blur_sub` fields (underscore prefix = retained for lifetime, not unused)
- Named structs for complex state (e.g., `InlineEdit`, `SessionRename`, `SessionCreate`)
- `Entity<T>` for GPUI entity handles (e.g., `Entity<TerminalView>`, `Entity<TextInput>`)
- Generation tracking (`git_poll_generation`) for cancelling stale async polls

### Config Persistence

Config is stored as JSON at the platform's config directory (via `directories` crate). Loaded at startup in `storage.rs` using a lenient `Raw*` deserialization layer that tolerates missing/extra fields.
