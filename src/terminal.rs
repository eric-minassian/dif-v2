use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::{Context as AnyhowContext, Result};
use gpui::{AppContext, Context, Entity, Window, px};
use gpui_terminal::{ColorPalette, TerminalConfig, TerminalView};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

pub fn spawn_terminal<T: 'static>(
    window: &mut Window,
    cx: &mut Context<T>,
    working_directory: &Path,
) -> Result<Entity<TerminalView>> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    spawn_terminal_inner(window, cx, working_directory, &shell, &["-l"])
}

fn spawn_terminal_inner<T: 'static>(
    _window: &mut Window,
    cx: &mut Context<T>,
    working_directory: &Path,
    command: &str,
    args: &[&str],
) -> Result<Entity<TerminalView>> {
    let config = TerminalConfig {
        cols: 80,
        rows: 24,
        font_family: "Menlo".into(),
        font_size: px(13.0),
        line_height_multiplier: 1.0,
        scrollback: 10_000,
        padding: gpui::Edges::all(px(6.0)),
        colors: ColorPalette::default(),
    };

    let pty_system = native_pty_system();
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: config.rows.try_into().unwrap_or(u16::MAX),
            cols: config.cols.try_into().unwrap_or(u16::MAX),
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("failed to create pty")?;

    let mut cmd = CommandBuilder::new(command);
    for arg in args {
        cmd.arg(*arg);
    }
    cmd.cwd(working_directory);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "dif");
    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .context("failed to spawn command")?;

    thread::spawn(move || {
        let _ = child.wait();
    });

    let master = pty_pair.master;
    let pty_reader = master
        .try_clone_reader()
        .context("failed to clone pty reader")?;
    let pty_writer = master.take_writer().context("failed to take pty writer")?;
    let resize_master = Arc::new(Mutex::new(master));

    Ok(cx.new(|cx| {
        TerminalView::new(pty_writer, pty_reader, config, cx).with_resize_callback(
            move |cols, rows| {
                let Ok(master) = resize_master.lock() else {
                    return;
                };

                let _ = master.resize(PtySize {
                    rows: rows.try_into().unwrap_or(u16::MAX),
                    cols: cols.try_into().unwrap_or(u16::MAX),
                    pixel_width: 0,
                    pixel_height: 0,
                });
            },
        )
    }))
}
