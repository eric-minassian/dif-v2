use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::{Context as AnyhowContext, Result};
use gpui::{AppContext, Context, Entity, Window};
use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use crate::view::{TerminalInput, TerminalView};
use crate::{TerminalConfig, TerminalSession};

pub fn spawn_terminal<T: 'static>(
    window: &mut Window,
    cx: &mut Context<T>,
    working_directory: &Path,
) -> Result<Entity<TerminalView>> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    spawn_terminal_inner(window, cx, working_directory, &shell, &["-l"])
}

fn spawn_terminal_inner<T: 'static>(
    window: &mut Window,
    cx: &mut Context<T>,
    working_directory: &Path,
    command: &str,
    args: &[&str],
) -> Result<Entity<TerminalView>> {
    let config = TerminalConfig::default();

    let pty_system = native_pty_system();
    let pty_pair = pty_system
        .openpty(PtySize {
            rows: config.rows,
            cols: config.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("failed to create pty")?;

    let master: Arc<dyn portable_pty::MasterPty + Send> = Arc::from(pty_pair.master);

    let mut cmd = CommandBuilder::new(command);
    for arg in args {
        cmd.arg(*arg);
    }
    cmd.cwd(working_directory);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("TERM_PROGRAM", "dif");
    if std::env::var("LANG").is_err() {
        cmd.env("LANG", "en_US.UTF-8");
    }

    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .context("failed to spawn command")?;

    thread::spawn(move || {
        let _ = child.wait();
    });

    let mut pty_reader = master
        .try_clone_reader()
        .context("failed to clone pty reader")?;
    let mut pty_writer = master.take_writer().context("failed to take pty writer")?;

    let (stdin_tx, stdin_rx) = mpsc::channel::<Vec<u8>>();
    let (stdout_tx, stdout_rx) = mpsc::channel::<Vec<u8>>();

    // Stdin writer thread: reads from channel, writes to PTY
    thread::spawn(move || {
        while let Ok(bytes) = stdin_rx.recv() {
            if pty_writer.write_all(&bytes).is_err() {
                break;
            }
            let _ = pty_writer.flush();
        }
    });

    // Stdout reader thread: reads from PTY, sends to channel
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            let n = match pty_reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };
            let _ = stdout_tx.send(buf[..n].to_vec());
        }
    });

    let master_for_resize = master.clone();
    let view = cx.new(|cx: &mut Context<TerminalView>| {
        let focus_handle = cx.focus_handle();

        let session = TerminalSession::new(config).expect("terminal init");
        let stdin_tx_for_input = stdin_tx.clone();
        let input = TerminalInput::new(move |bytes| {
            let _ = stdin_tx_for_input.send(bytes.to_vec());
        });

        TerminalView::new_with_input(session, focus_handle, input).with_resize_callback(
            move |cols, rows| {
                let _ = master_for_resize.resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                });
            },
        )
    });

    // 16ms polling task: drain stdout channel → queue_output_bytes
    let view_for_task = view.clone();
    window
        .spawn(cx, async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let mut batch = Vec::new();
                while let Ok(chunk) = stdout_rx.try_recv() {
                    batch.extend_from_slice(&chunk);
                }
                if batch.is_empty() {
                    continue;
                }

                cx.update(|_, cx| {
                    view_for_task.update(cx, |this: &mut TerminalView, cx| {
                        this.queue_output_bytes(&batch, cx);
                    });
                })
                .ok();
            }
        })
        .detach();

    Ok(view)
}
