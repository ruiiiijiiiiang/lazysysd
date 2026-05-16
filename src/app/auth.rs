use std::{
    io::Result,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use tokio::{spawn, time::sleep};

use crate::{
    app::state::{AUTH_START_DELAY, App},
    models::{AppInternalEvent, PrivilegedAction},
    systemd::{
        auth::{EmbeddedAuthFlow, EmbeddedAuthPane},
        dbus::perform_unit_action,
        edit::perform_unit_edit,
    },
};

impl App {
    pub async fn start_embedded_auth(
        &mut self,
        action: PrivilegedAction,
        cols: u16,
        rows: u16,
    ) -> Result<()> {
        if self.embedded_auth.is_some() {
            return Ok(());
        }

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
                } => perform_unit_action(&unit_name, &scope, &action).await,
                PrivilegedAction::ApplyEdit {
                    unit_name,
                    scope,
                    mode,
                    content,
                } => perform_unit_edit(&unit_name, &scope, mode, content).await,
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
