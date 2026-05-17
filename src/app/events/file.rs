use crossterm::event::{KeyCode, KeyEvent};

use crate::{
    app::state::context::App,
    models::{PendingAction, UnitEditMode},
};

impl App {
    pub async fn handle_file_view_key(&mut self, key: KeyEvent) -> bool {
        if self.handle_nav_key(key) {
            return false;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.enter_unit_list_view();
            }
            KeyCode::Char('e') if !self.file_view.path.is_empty() => {
                if let Some(request) = self.build_edit_request(UnitEditMode::Override) {
                    self.pending_action = Some(PendingAction::EditFile(request));
                    return true;
                }
            }
            KeyCode::Char('E') if !self.file_view.path.is_empty() => {
                if let Some(request) = self.build_edit_request(UnitEditMode::Full) {
                    self.pending_action = Some(PendingAction::EditFile(request));
                    return true;
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {}
            KeyCode::Char('/') if !self.file_view.content.is_empty() => {
                self.start_search();
            }
            KeyCode::Char('n') if !self.search.query.is_empty() => {
                self.cycle_file_search_match(true);
            }
            KeyCode::Char('N') if !self.search.query.is_empty() => {
                self.cycle_file_search_match(false);
            }
            _ => {}
        }
        false
    }
}
