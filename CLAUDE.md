# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

@AGENTS.md

## Project

Dif is a native macOS Git client built in Rust using [GPUI](https://github.com/zed-industries/zed) (Zed's UI framework). It supports multi-session workflows via git worktrees, an integrated terminal (alacritty_terminal), side-by-side diffs, and GitHub CLI integration.

## Build & Development

```bash
# Build (debug)
cargo build

# Build (release) and bundle macOS .app
scripts/bundle.sh              # outputs build/Dif.app
scripts/bundle.sh --install    # also copies to /Applications

# Check (what CI runs)
cargo check

# Run tests
cargo test
cargo test <test_name>         # run a single test

# Run the app directly
cargo run
```

## Architecture

**Entry flow:** `main.rs` → `app::run()` → creates GPUI `Application` → opens window with `WorkspaceView`

### Key modules

- **`workspace/`** — UI layer. `WorkspaceView` is the root view that composes all panels. Each file in this directory implements methods on `WorkspaceView` for a specific concern (sidebar, diff view, commit input, changes list, git actions, session management, settings, titlebar). Keyboard shortcuts are bound in `app.rs`.
- **`git/`** — All git/gh CLI interactions. `mod.rs` provides `run_git()`/`run_gh()` helpers that resolve binary paths and execute commands. Submodules handle diff computation (`diff.rs` using the `similar` crate), status parsing (`status.rs`), worktree management (`worktree.rs`), and conventional commit validation (`conventional.rs`).
- **`terminal_view/`** — Embedded terminal. `TerminalSession` wraps `alacritty_terminal` for VT parsing; `TerminalView` renders it as a GPUI element. Submodules handle drawing, input, clipboard, mouse events, URL detection, and viewport scrolling.
- **`state/`** — Application state types. `AppState` (top-level) → `ProjectRuntime` (per-project git snapshot, staged files) → `SessionRuntime` (terminals, commit message, branch status). `config.rs` defines the serializable config types; `git.rs` defines diff/change/branch data types; `ui.rs` defines UI state.
- **`storage.rs`** — JSON config persistence to the user's config directory via the `directories` crate.
- **`theme.rs`** — Dark color scheme. Static `Theme` struct accessed via `theme()`. Colors use GPUI's `Hsla`/`rgb`/`rgba`.
- **`components.rs`** — Shared UI primitives (buttons, panels, section headers).
- **`updater.rs`** — Checks GitHub releases for updates.

### GPUI patterns

UI is built with a fluent builder API:
```rust
div().flex().items_center().px_3().bg(theme().bg_surface).child(content)
```

Event handling uses `cx.listener(|this, event, window, cx| { ... })`. Async work is spawned with `window.spawn(cx, async move { ... })`. State changes call `cx.notify()` to trigger re-renders.

### Session model

Each project can have multiple sessions. Sessions map to git worktrees stored under `~/.dif/{project-name}/{short-id}/`, enabling concurrent work on different branches without stashing.

## Conventions

- Conventional Commits format for commit messages (validated in `git/conventional.rs`)
- Release automation via release-please (generates CHANGELOG.md and version bumps in Cargo.toml)
- CI runs `cargo check` on PRs to main
- macOS-only target (min deployment: macOS 13.0)
