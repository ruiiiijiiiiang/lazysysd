use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    app::{
        events::unit_command_for_key,
        state::{App, FilterMenu, ViewMode},
    },
    models::{PendingAction, UnitEditMode},
};

impl App {
    pub async fn handle_unit_list_key(&mut self, key: KeyEvent) -> bool {
        if self.handle_nav_key(key) {
            return false;
        }

        match key.code {
            KeyCode::Char('/') => {
                self.is_searching = true;
                self.set_search_cursor_to_end();
                return false;
            }
            KeyCode::Char('a') => {
                self.open_filter_menu = Some(FilterMenu::Active);
                return false;
            }
            KeyCode::Char('n') => {
                self.open_filter_menu = Some(FilterMenu::Enablement);
                return false;
            }
            KeyCode::Char('o') => {
                self.open_filter_menu = Some(FilterMenu::Load);
                return false;
            }
            KeyCode::Char('p') => {
                self.open_filter_menu = Some(FilterMenu::Scope);
                return false;
            }
            KeyCode::Char('l') | KeyCode::Enter => {
                if let Some(unit) = self.get_selected_unit() {
                    let name = unit.name.clone();
                    let scope = unit.scope.clone();
                    self.view_mode = ViewMode::LogView;
                    self.unit_logs.clear();
                    self.log_state.select(None);
                    self.clear_log_visual_modes();
                    self.fetch_unit_logs(name, scope).await;
                }
                return false;
            }
            KeyCode::Char('v') => {
                if let Some(unit) = self.get_selected_unit() {
                    let unit_clone = unit.clone();
                    self.view_mode = ViewMode::FileView;
                    self.unit_file_content.clear();
                    self.unit_file_path.clear();
                    self.file_scroll = 0;
                    self.fetch_unit_file(unit_clone).await;
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

    pub async fn handle_file_view_key(&mut self, key: KeyEvent) -> bool {
        if self.file_search_mode {
            match key.code {
                KeyCode::Esc => {
                    self.clear_file_search();
                }
                KeyCode::Enter => {
                    self.file_search_mode = false;
                }
                KeyCode::Left | KeyCode::Right => {
                    self.edit_file_search_key(key);
                }
                KeyCode::Backspace | KeyCode::Char(_) => {
                    self.edit_file_search_key(key);
                    self.cycle_file_search_match(true);
                }
                _ => {}
            }
            return false;
        }

        if self.handle_nav_key(key) {
            return false;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.view_mode = ViewMode::UnitList;
                self.clear_file_search();
            }
            KeyCode::Char('e') if !self.unit_file_path.is_empty() => {
                if let Some(request) = self.build_edit_request(UnitEditMode::Override) {
                    self.pending_action = Some(PendingAction::EditFile(request));
                    return true;
                }
            }
            KeyCode::Char('E') if !self.unit_file_path.is_empty() => {
                if let Some(request) = self.build_edit_request(UnitEditMode::Full) {
                    self.pending_action = Some(PendingAction::EditFile(request));
                    return true;
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {}
            KeyCode::Char('/') if !self.unit_file_content.is_empty() => {
                self.start_file_search();
            }
            KeyCode::Char('n') if !self.file_search_query.is_empty() => {
                self.cycle_file_search_match(true);
            }
            KeyCode::Char('N') if !self.file_search_query.is_empty() => {
                self.cycle_file_search_match(false);
            }
            _ => {}
        }
        false
    }
}
