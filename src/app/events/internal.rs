use tokio::task::spawn_blocking;

use crate::{
    app::state::context::{App, ViewMode},
    models::{AppInternalEvent, PrivilegedAction},
};

impl App {
    pub async fn handle_internal_event(&mut self, event: AppInternalEvent) {
        match event {
            AppInternalEvent::UnitsLoaded(units) => {
                self.unit_list.units = units;
                self.is_loading = false;
                self.update_filter();
            }
            AppInternalEvent::LogsLoaded(logs) => {
                self.log_view.logs = logs;
                self.is_loading = false;
                self.log_view
                    .state
                    .select(Some(self.log_view.logs.len().saturating_sub(1)));
            }
            AppInternalEvent::FileLoaded(content, path) => {
                self.file_view.content = content;
                self.file_view.path = path;
                self.is_loading = false;
            }
            AppInternalEvent::PtyOutput(chunk) => {
                if let Some(flow) = self.embedded_auth.as_mut() {
                    flow.pane.output.push_str(&chunk);
                }
            }
            AppInternalEvent::PtyClosed => {
                self.embedded_auth = None;
            }
            AppInternalEvent::AuthResult(result) => {
                if let Some(mut flow) = self.embedded_auth.take() {
                    spawn_blocking(move || {
                        flow.pane.stop();
                    });
                }
                let action = self.active_privileged_action.take();
                if result.success {
                    match action {
                        Some(PrivilegedAction::UnitCommand { .. }) => {
                            self.refresh_units().await;
                        }
                        Some(PrivilegedAction::ApplyEdit { .. }) => {
                            self.pending_edit_review = None;
                            self.view_mode = ViewMode::UnitList;
                            self.file_view.content.clear();
                            self.file_view.path.clear();
                            self.file_view.scroll = 0;
                            self.refresh_units().await;
                        }
                        None => {}
                    }
                } else {
                    self.error_message = result.error.or(Some("Action failed".to_string()));
                }
            }
            AppInternalEvent::Error(err) => {
                self.is_loading = false;
                self.error_message = Some(err);
            }
        }
    }
}
