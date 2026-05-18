use std::io::{Error, Result};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::{FutureExt, StreamExt};
use tokio::{
    sync::mpsc,
    time::{Duration, interval},
};

use crate::{
    app::{
        editor::{run_editor_request, run_editor_text},
        state::context::App,
    },
    models::PendingAction,
    ui::{render::draw, utils::Tui},
};

pub async fn run_app() -> Result<()> {
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
                            if let Some(action) = app.pending_action.take() {
                                tui.exit()?;
                                match action {
                                    PendingAction::EditFile(request) => {
                                        let result = run_editor_request(
                                            request.initial_content,
                                            request.unit_name,
                                            request.scope,
                                            request.mode,
                                            request.restore_content,
                                            request.restore_path,
                                        );
                                        tui.resume()?;
                                        let (request, edited_content) = result?;
                                        app.finish_edit_request(request, edited_content);
                                    }
                                    PendingAction::EditText { filename, content } => {
                                        let result = run_editor_text(filename, content);
                                        tui.resume()?;
                                        let _ = result?;
                                    }
                                }
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
