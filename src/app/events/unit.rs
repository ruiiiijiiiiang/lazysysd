use crossterm::event::{KeyCode, KeyEvent};
use tokio::task::spawn_blocking;

use crate::app::{
    events::handler::unit_command_for_key,
    state::context::{App, FilterMenu},
    utils::copy_to_clipboard,
};

impl App {
    pub async fn handle_unit_list_key(&mut self, key: KeyEvent) -> bool {
        if self.handle_nav_key(key) {
            return false;
        }

        match key.code {
            KeyCode::Char('r')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.reset_unit_filters();
                self.notify(
                    "All filters reset".to_string(),
                    crate::models::NotificationType::Success,
                );
                return false;
            }
            KeyCode::Char('/') => {
                self.start_search();
                return false;
            }
            KeyCode::Char('a') => {
                self.unit_list.open_filter_menu = Some(FilterMenu::Active);
                return false;
            }
            KeyCode::Char('n') => {
                self.unit_list.open_filter_menu = Some(FilterMenu::Enablement);
                return false;
            }
            KeyCode::Char('o') => {
                self.unit_list.open_filter_menu = Some(FilterMenu::Load);
                return false;
            }
            KeyCode::Char('y') => {
                self.unit_list.open_filter_menu = Some(FilterMenu::Type);
                return false;
            }
            KeyCode::Char('Y') => {
                if let Some(path) = selected_unit_file_path(self) {
                    spawn_blocking(move || copy_to_clipboard(&path));
                    self.notify(
                        "Path copied to clipboard".to_string(),
                        crate::models::NotificationType::Success,
                    );
                }
                return false;
            }
            KeyCode::Char('p') => {
                self.unit_list.open_filter_menu = Some(FilterMenu::Scope);
                return false;
            }
            KeyCode::Char('l') | KeyCode::Enter => {
                self.enter_log_view().await;
                return false;
            }
            KeyCode::Char('f') => {
                if let Some(unit) = self.get_selected_unit()
                    && !unit.fragment_path.is_empty()
                {
                    self.enter_file_view().await;
                }
                return false;
            }
            _ => {
                if let Some(action) = unit_command_for_key(key) {
                    self.trigger_selected_unit_command(action).await.ok();
                    return false;
                }
            }
        }
        false
    }
}

fn selected_unit_file_path(app: &App) -> Option<String> {
    app.get_selected_unit().map(|unit| {
        if unit.fragment_path.is_empty() {
            unit.path.to_string()
        } else {
            unit.fragment_path.clone()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use zbus::zvariant::OwnedObjectPath;

    use crate::models::{UnitActiveState, UnitEnablementState, UnitInfo, UnitLoadState, UnitScope};

    fn test_app() -> App {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.unit_list.units = vec![UnitInfo {
            name: "ssh.service".to_string(),
            description: "Secure Shell".to_string(),
            scope: UnitScope::Global,
            load_state: UnitLoadState::Loaded,
            active_state: UnitActiveState::Active,
            enablement_state: UnitEnablementState::Enabled,
            sub_state: "running".to_string(),
            path: OwnedObjectPath::try_from("/test/unit/ssh").unwrap(),
            fragment_path: "/etc/systemd/system/ssh.service".to_string(),
        }];
        app.unit_list.filtered_indices = vec![0];
        app.unit_list.select_index(Some(0));
        app.is_loading = false;
        app
    }

    #[test]
    fn selected_unit_file_path_returns_selected_path() {
        let app = test_app();

        assert_eq!(
            selected_unit_file_path(&app),
            Some("/etc/systemd/system/ssh.service".to_string())
        );
    }

    #[tokio::test]
    async fn handle_unit_list_key_accepts_copy_path_key() {
        let mut app = test_app();

        let handled = app
            .handle_unit_list_key(KeyEvent::new(
                KeyCode::Char('Y'),
                crossterm::event::KeyModifiers::NONE,
            ))
            .await;

        assert!(!handled);
    }
}
