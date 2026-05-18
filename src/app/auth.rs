use std::{
    fs,
    io::{Error, Read, Result, Write},
    process,
    sync::Arc,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::{spawn, sync::mpsc, time::sleep};

use crate::{
    app::{state::context::App, utils::AUTH_START_DELAY},
    models::{AppInternalEvent, PrivilegedAction},
    systemd::{dbus::perform_unit_action, edit::perform_unit_edit},
};

pub struct EmbeddedAuthFlow {
    pub pane: EmbeddedAuthPane,
    pub cancel_flag: Arc<AtomicBool>,
}

pub struct EmbeddedAuthPane {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub output: String,
}

impl EmbeddedAuthPane {
    pub fn spawn(
        cols: u16,
        rows: u16,
        internal_tx: mpsc::Sender<AppInternalEvent>,
    ) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                cols,
                rows,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(pty_to_io_error)?;

        let pid = process::id();
        let start_time = current_process_start_time()?;
        let mut cmd = CommandBuilder::new("pkttyagent");
        cmd.arg("--process");
        cmd.arg(format!("{pid},{start_time}"));

        let child = pair.slave.spawn_command(cmd).map_err(pty_to_io_error)?;
        let mut reader = pair.master.try_clone_reader().map_err(pty_to_io_error)?;
        let writer = pair.master.take_writer().map_err(pty_to_io_error)?;
        let master = pair.master;

        thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => {
                        let _ = internal_tx.blocking_send(AppInternalEvent::PtyClosed);
                        break;
                    }
                    Ok(n) => {
                        let chunk = normalize_pty_output(&buffer[..n]);
                        if !chunk.is_empty() {
                            let _ = internal_tx.blocking_send(AppInternalEvent::PtyOutput(chunk));
                        }
                    }
                }
            }
        });

        Ok(Self {
            master,
            writer,
            child,
            output: String::new(),
        })
    }

    pub fn send_key(&mut self, key: KeyEvent) -> Result<()> {
        if let Some(bytes) = key_to_bytes(key) {
            self.writer.write_all(&bytes)?;
            self.writer.flush()?;
        }
        Ok(())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                cols,
                rows,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(pty_to_io_error)
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl App {
    pub async fn start_embedded_auth(&mut self, action: PrivilegedAction) -> Result<()> {
        if self.embedded_auth.is_some() {
            return Ok(());
        }

        let (cols, rows) = self.terminal_size;
        let pane = EmbeddedAuthPane::spawn(cols, rows, self.internal_tx.clone())?;
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel_flag);
        let tx_clone = self.internal_tx.clone();
        let worker_action = action.clone();

        spawn(async move {
            sleep(AUTH_START_DELAY).await;
            if cancel_clone.load(Ordering::SeqCst) {
                return;
            }

            let result = match worker_action {
                PrivilegedAction::UnitCommand {
                    unit_name,
                    scope,
                    action,
                } => perform_unit_action(&unit_name, scope, action).await,
                PrivilegedAction::ApplyEdit {
                    unit_name,
                    scope,
                    mode,
                    content,
                } => perform_unit_edit(&unit_name, scope, mode, content).await,
            };
            let _ = tx_clone.send(AppInternalEvent::AuthResult(result)).await;
        });

        self.active_privileged_action = Some(action);
        self.embedded_auth = Some(EmbeddedAuthFlow { pane, cancel_flag });
        Ok(())
    }

    pub fn cancel_embedded_auth(&mut self, _reason: &str) {
        self.active_privileged_action = None;
        if let Some(mut flow) = self.embedded_auth.take() {
            flow.cancel_flag.store(true, Ordering::SeqCst);
            flow.pane.stop();
        }
    }

    pub fn resize_embedded_auth(&mut self, cols: u16, rows: u16) -> Result<()> {
        if let Some(flow) = self.embedded_auth.as_mut() {
            flow.pane.resize(cols, rows)?;
        }
        Ok(())
    }
}

fn pty_to_io_error(err: impl std::fmt::Display) -> Error {
    Error::other(err.to_string())
}

fn normalize_pty_output(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn current_process_start_time() -> Result<u64> {
    let stat = fs::read_to_string("/proc/self/stat")?;
    let (_prefix, rest) = stat
        .rsplit_once(") ")
        .ok_or_else(|| Error::other("proc parse error"))?;
    let fields: Vec<&str> = rest.split_whitespace().collect();
    fields
        .get(19)
        .ok_or_else(|| Error::other("stat field missing"))?
        .parse::<u64>()
        .map_err(|e| Error::other(e.to_string()))
}

fn key_to_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(vec![0x03]),
            KeyCode::Char('d') => Some(vec![0x04]),
            KeyCode::Char('q') => Some(vec![0x11]),
            _ => None,
        };
    }
    match key.code {
        KeyCode::Enter => Some(vec![b'\r']),
        KeyCode::Tab => Some(vec![b'\t']),
        KeyCode::Backspace => Some(vec![0x7f]),
        KeyCode::Esc => Some(vec![0x1b]),
        KeyCode::Char(c) => {
            let mut buf = [0; 4];
            Some(c.encode_utf8(&mut buf).as_bytes().to_vec())
        }
        _ => None,
    }
}
