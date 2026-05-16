mod app;
mod models;
mod systemd;
mod ui;

use std::{
    env,
    ffi::OsStr,
    fs,
    io::{Error, Result},
    path::PathBuf,
    process::{Command, ExitCode},
    time::{SystemTime, UNIX_EPOCH},
};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::{FutureExt, StreamExt};
use tokio::{
    sync::mpsc,
    time::{Duration, interval},
};

use crate::{
    app::state::{App, strip_ansi_content},
    models::{EditRequest, PendingAction, PendingAction::EditFile, UnitEditMode},
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
            ["nano", "vim", "vi", "emacs"]
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
                        if let Some(action) = app.pending_action.take() {
                            tui.exit()?;
                            match action {
                                EditFile(request) => {
                                    let result = run_editor_request(
                                        request.initial_content,
                                        request.unit_name,
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

fn run_editor_request(
    initial_content: String,
    unit_name: String,
    mode: UnitEditMode,
    restore_content: String,
    restore_path: String,
) -> Result<(EditRequest, String)> {
    let editor = resolve_editor()?;
    let temp_path = create_temp_edit_path(&unit_name, mode);
    fs::write(&temp_path, &initial_content)
        .map_err(|e| Error::other(format!("Failed to prepare edit draft: {e}")))?;

    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{} \"$0\"", editor))
        .arg(&temp_path)
        .status()
        .map_err(|e| Error::other(format!("Failed to start editor: {e}")))?;

    let edited_content = fs::read_to_string(&temp_path)
        .map_err(|e| Error::other(format!("Failed to read edited draft: {e}")))?;
    let _ = fs::remove_file(&temp_path);

    let request = EditRequest {
        unit_name,
        scope: String::new(),
        mode,
        initial_content,
        restore_content,
        restore_path,
    };

    if !status.success() && edited_content == request.initial_content {
        let initial_content = request.initial_content.clone();
        return Ok((request, initial_content));
    }

    Ok((request, edited_content))
}

fn run_editor_text(filename: String, content: String) -> Result<String> {
    let editor = resolve_editor()?;
    let temp_path = env::temp_dir().join(format!("lazysysd-{filename}-{}.log", unique_suffix()));
    fs::write(&temp_path, strip_ansi_content(&content))
        .map_err(|e| Error::other(format!("Failed to prepare log draft: {e}")))?;

    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{} \"$0\"", editor))
        .arg(&temp_path)
        .status()
        .map_err(|e| Error::other(format!("Failed to start editor: {e}")))?;

    let edited_content = fs::read_to_string(&temp_path)
        .map_err(|e| Error::other(format!("Failed to read edited log draft: {e}")))?;
    let _ = fs::remove_file(&temp_path);

    if !status.success() && edited_content == content {
        return Ok(content);
    }

    Ok(edited_content)
}

fn create_temp_edit_path(unit_name: &str, mode: UnitEditMode) -> PathBuf {
    let mut sanitized_name = unit_name.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    if sanitized_name.is_empty() {
        sanitized_name = "unit".to_string();
    }

    let mode_label = match mode {
        UnitEditMode::Override => "override",
        UnitEditMode::Full => "full",
    };
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    env::temp_dir().join(format!(
        "lazysysd-{sanitized_name}-{mode_label}-{unique}.conf"
    ))
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
