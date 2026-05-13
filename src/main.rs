mod app;
mod models;
mod systemd;
mod ui;

use std::{io, process::ExitCode};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::{FutureExt, StreamExt};
use tokio::sync::mpsc;

use crate::{
    app::state::App,
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

async fn run_app() -> io::Result<()> {
    let mut tui = Tui::enter()?;
    let (tx, mut rx) = mpsc::channel(64);
    let mut app = App::new(tx).await;
    let mut reader = EventStream::new();

    loop {
        tui.terminal.draw(|frame| draw(frame, &mut app))?;
        tokio::select! {
        maybe_event = reader.next().fuse() => {
            match maybe_event {
                Some(Ok(Event::Resize(c, r))) => app.resize_embedded_auth(c, r)?,
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press
                    && app.handle_key(key).await? => {
                        if let Some(crate::models::PendingAction::EditFile(path)) = app.pending_action.take() {
                            tui.exit()?;
                            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
                            let mut child = std::process::Command::new("sh")
                                .arg("-c")
                                .arg(format!("{} \"$0\"", editor))
                                .arg(path)
                                .spawn()
                                .map_err(|e| io::Error::other(format!("Failed to start editor: {e}")))?;
                            child.wait()?;
                            tui.resume()?;
                            continue;

                        }
                        break;
                    }
                Some(Err(e)) => return Err(io::Error::other(e)),
                None => break,
                _ => {}
            }
        }

                maybe_internal = rx.recv() => {
                    if let Some(event) = maybe_internal {
                        app.handle_internal_event(event).await;
                    }
                }
            }
    }

    tui.exit()
}
