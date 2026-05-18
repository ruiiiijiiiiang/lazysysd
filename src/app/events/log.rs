use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::task::spawn_blocking;

use crate::{
    app::{state::context::App, utils::copy_to_clipboard},
    models::PendingAction,
};

impl App {
    pub async fn handle_log_view_key(&mut self, key: KeyEvent) -> bool {
        if self.log_view.visual_line_select {
            if self.handle_nav_key(key) {
                return false;
            }
            match key.code {
                KeyCode::Esc => {
                    self.log_view.visual_line_select = false;
                    self.log_view.line_marks.clear();
                }
                KeyCode::Char(' ') => {
                    self.toggle_log_line_mark();
                }
                KeyCode::Char('y') | KeyCode::Enter => {
                    if let Some(text) = self.selected_log_lines_text() {
                        let cloned = text.clone();
                        spawn_blocking(move || copy_to_clipboard(&cloned));
                    }
                    self.log_view.visual_line_select = false;
                    self.log_view.line_marks.clear();
                }
                _ => {}
            }
            return false;
        }

        if self.log_view.visual_select {
            if self.handle_nav_key(key) {
                return false;
            }
            match key.code {
                KeyCode::Esc => {
                    self.clear_log_visual_modes();
                }
                KeyCode::Char(' ') => {
                    if let Some(i) = self.log_view.state.selected()
                        && !self.log_view.selected_lines.remove(&i)
                    {
                        self.log_view.selected_lines.insert(i);
                    }
                }
                KeyCode::Char('y') | KeyCode::Enter => {
                    let mut indices: Vec<_> = self.log_view.selected_lines.iter().collect();
                    indices.sort();
                    let selected: Vec<&String> = indices
                        .into_iter()
                        .filter_map(|&i| self.log_view.logs.get(i))
                        .collect();
                    if !selected.is_empty() {
                        let text = selected
                            .into_iter()
                            .map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join("\n");
                        let cloned = text.clone();
                        spawn_blocking(move || copy_to_clipboard(&cloned));
                    }
                    self.clear_log_visual_modes();
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
                self.enter_unit_list_view();
                return false;
            }
            KeyCode::Char('v') if !self.log_view.logs.is_empty() => {
                self.log_view.visual_select = true;
                if self.log_view.state.selected().is_none() {
                    self.log_view.state.select(Some(0));
                }
                return false;
            }
            KeyCode::Char('V') if !self.log_view.logs.is_empty() => {
                self.log_view.visual_line_select = true;
                self.log_view.line_marks.clear();
                if self.log_view.state.selected().is_none() {
                    self.log_view.state.select(Some(0));
                }
                return false;
            }
            KeyCode::Char('v') | KeyCode::Char('V') => {}
            KeyCode::Char('/') if !self.log_view.logs.is_empty() => {
                self.start_search();
                return false;
            }
            KeyCode::Char('n') if !self.search.query.is_empty() => {
                self.cycle_log_search_match(true);
                return false;
            }
            KeyCode::Char('N') if !self.search.query.is_empty() => {
                self.cycle_log_search_match(false);
                return false;
            }
            KeyCode::Char('e') if !self.log_view.logs.is_empty() => {
                self.pending_action = Some(PendingAction::EditText {
                    filename: format!("log-{}.txt", self.unit_list.selected_key.name),
                    content: self.log_view.logs.join("\n"),
                });
                return true;
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(unit) = self.get_selected_unit() {
                    let name = unit.name.clone();
                    let scope = unit.scope.to_string();
                    self.fetch_unit_logs(name, scope).await;
                }
                return false;
            }
            _ => {}
        }

        false
    }
}
