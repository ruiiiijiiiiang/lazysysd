mod app;
mod models;
mod systemd;
mod ui;

use std::{
    env,
    ffi::OsStr,
    io::{Error, Result},
    process::{Command, ExitCode},
};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::{FutureExt, StreamExt};
use tokio::{
    sync::mpsc,
    time::{Duration, interval},
};

use crate::{
    app::state::App,
    models::PendingAction::EditFile,
    ui::{render::draw, utils::Tui},
};

#[tokio::main]
async fn main() -> ExitCode {
    match run_app().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn resolve_editor() -> Result<String> {
    let candidates = env::var("VISUAL")
        .ok()
        .filter(|e| !e.is_empty())
        .or_else(|| env::var("EDITOR").ok().filter(|e| !e.is_empty()))
        .map(|e| vec![e])
        .unwrap_or_else(|| {
            ["nano", "vim", "vi", "emacs", "pico"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        });

    for editor in &candidates {
        if Command::new("which")
            .arg(OsStr::new(editor))
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return Ok(editor.clone());
        }
    }

    Err(Error::other(
        "No text editor found. Set $VISUAL or $EDITOR, or install one of: nano, vim, vi, emacs, pico",
    ))
}

async fn run_app() -> Result<()> {
    let mut tui = Tui::enter()?;
    let (tx, mut rx) = mpsc::channel(64);
    let mut app = App::new(tx).await;
    let mut reader = EventStream::new();
    let mut tick = interval(Duration::from_secs(2));

    loop {
        tui.terminal.draw(|frame| draw(frame, &mut app))?;
        tokio::select! {
        maybe_event = reader.next().fuse() => {
            match maybe_event {
                Some(Ok(Event::Resize(c, r))) => app.resize_embedded_auth(c, r)?,
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press
                    && app.handle_key(key).await? => {
                        if let Some(EditFile(path)) = app.pending_action.take() {
                            tui.exit()?;
                            let editor = resolve_editor()?;
                            let mut child = Command::new("sh")
                                .arg("-c")
                                .arg(format!("{} \"$0\"", editor))
                                .arg(path)
                                .spawn()
                                .map_err(|e| Error::other(format!("Failed to start editor: {e}")))?;
                            child.wait()?;
                            tui.resume()?;
                            continue;

                        }
                        break;
                    }
                Some(Err(e)) => return Err(Error::other(e)),
                None => break,
                _ => {}
            }
        }

        maybe_internal = rx.recv() => {
            if let Some(event) = maybe_internal {
                app.handle_internal_event(event).await;
            }
        }

        _ = tick.tick() => {
            app.refresh_units().await;
        }
        }
    }

    tui.exit()
}
