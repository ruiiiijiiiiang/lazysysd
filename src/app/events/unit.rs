use crossterm::event::{KeyCode, KeyEvent};

use crate::app::{
    events::handler::unit_command_for_key,
    state::context::{App, FilterMenu},
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
            KeyCode::Char('p') => {
                self.unit_list.open_filter_menu = Some(FilterMenu::Scope);
                return false;
            }
            KeyCode::Char('l') | KeyCode::Enter => {
                self.enter_log_view().await;
                return false;
            }
            KeyCode::Char('f') => {
                self.enter_file_view().await;
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
