use tokio::task::spawn_blocking;

use crate::{
    app::state::context::{App, ViewMode},
    models::{AppInternalEvent, PrivilegedAction},
};

impl App {
    pub async fn handle_internal_event(&mut self, event: AppInternalEvent) {
        match event {
            AppInternalEvent::UnitsLoaded(units, is_manual) => {
                self.unit_list.units = units;
                self.is_loading = false;
                self.update_filter();
                if is_manual {
                    self.notify(
                        "Unit list refreshed".to_string(),
                        crate::models::NotificationType::Success,
                    );
                }
            }
            AppInternalEvent::LogsLoaded(logs, is_manual) => {
                self.log_view.logs = logs;
                self.is_loading = false;
                self.log_view
                    .state
                    .select(Some(self.log_view.logs.len().saturating_sub(1)));
                if is_manual {
                    self.notify(
                        "Logs refreshed".to_string(),
                        crate::models::NotificationType::Success,
                    );
                }
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
                        Some(PrivilegedAction::UnitCommand {
                            unit_name, action, ..
                        }) => {
                            self.notify(
                                format!("{} {}", action.past_tense(), unit_name),
                                crate::models::NotificationType::Success,
                            );
                            self.refresh_units(false).await;
                        }
                        Some(PrivilegedAction::ApplyEdit { unit_name, .. }) => {
                            self.notify(
                                format!("Applied changes to {}", unit_name),
                                crate::models::NotificationType::Success,
                            );
                            self.pending_edit_review = None;
                            self.view_mode = ViewMode::UnitList;
                            self.file_view.content.clear();
                            self.file_view.path.clear();
                            self.file_view.scroll = 0;
                            self.refresh_units(false).await;
                        }
                        None => {}
                    }
                } else {
                    self.error_message = result.error.or(Some("Action failed".to_string()));
                }
            }
            AppInternalEvent::Error(err) => {
                self.is_loading = false;
                self.notify(err, crate::models::NotificationType::Error);
            }
            AppInternalEvent::ClearNotification => {
                self.notification = None;
            }
        }
    }
}
