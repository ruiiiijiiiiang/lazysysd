use std::io::Result;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    app::state::context::{App, SearchInputAction, ViewMode},
    models::{PrivilegedAction, UnitAction},
};

impl App {
    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.embedded_auth.is_some() {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
                return Ok(true);
            }
            if matches!(key.code, KeyCode::Esc)
                || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
            {
                self.cancel_embedded_auth("authentication cancelled");
                return Ok(false);
            }
            if let Some(flow) = self.embedded_auth.as_mut() {
                flow.pane.send_key(key)?;
            }
            return Ok(false);
        }

        if self.pending_edit_review.is_some() {
            return self.handle_edit_review_key(key).await;
        }

        if self.search.is_active {
            match key.code {
                KeyCode::Esc => {
                    self.clear_search();
                    if self.view_mode == ViewMode::FileView {
                        self.file_view.search_match = None;
                    }
                }
                KeyCode::Enter => self.search.is_active = false,
                _ => {
                    if self.handle_search_key(key) == Some(SearchInputAction::Edit) {
                        match self.view_mode {
                            ViewMode::UnitList => self.update_filter(),
                            ViewMode::LogView => self.cycle_log_search_match(true),
                            ViewMode::FileView => self.cycle_file_search_match(true),
                        }
                    }
                }
            }
            return Ok(false);
        }

        match self.view_mode {
            ViewMode::LogView => return Ok(self.handle_log_view_key(key).await),
            ViewMode::FileView => return Ok(self.handle_file_view_key(key).await),
            ViewMode::UnitList => {}
        }

        if self.unit_list.open_filter_menu.is_some() {
            self.handle_filter_menu_key(key);
            return Ok(false);
        }

        if key.code == KeyCode::Char('q') {
            return Ok(true);
        }

        Ok(self.handle_unit_list_key(key).await)
    }

    pub fn handle_filter_menu_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.unit_list.open_filter_menu = None;
            }
            KeyCode::Char(c) => {
                if let Some(menu) = self.unit_list.open_filter_menu {
                    let selected_hotkey = c.to_ascii_lowercase();
                    if let Some(option) = self
                        .filter_menu_options(menu)
                        .into_iter()
                        .find(|option| option.hotkey == selected_hotkey)
                    {
                        menu.set_selected_value(self, option.value);
                        self.unit_list.open_filter_menu = None;
                        self.update_filter();
                    }
                }
            }
            _ => {}
        }
    }

    pub async fn handle_edit_review_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('d') => {
                self.discard_edit_review();
            }
            KeyCode::Char('a') | KeyCode::Enter => {
                if let Some(review) = self.pending_edit_review.as_ref() {
                    self.start_embedded_auth(PrivilegedAction::ApplyEdit {
                        unit_name: review.unit_name.clone(),
                        scope: review.scope.clone(),
                        mode: review.mode,
                        content: review.edited_content.clone(),
                    })
                    .await?;
                }
            }
            _ => {}
        }

        Ok(false)
    }
}

pub fn unit_command_for_key(key: KeyEvent) -> Option<UnitAction> {
    if !(key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) {
        return None;
    }

    match key.code {
        KeyCode::Char('s') => Some(UnitAction::Start),
        KeyCode::Char('t') => Some(UnitAction::Stop),
        KeyCode::Char('r') => Some(UnitAction::Restart),
        KeyCode::Char('R') => Some(UnitAction::Reload),
        KeyCode::Char('e') => Some(UnitAction::Enable),
        KeyCode::Char('d') => Some(UnitAction::Disable),
        KeyCode::Char('m') => Some(UnitAction::Mask),
        KeyCode::Char('u') => Some(UnitAction::Unmask),
        KeyCode::Char('x') => Some(UnitAction::ResetFailed),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    use crate::models::PendingAction;

    #[test]
    fn unit_command_bindings_match_expected_actions() {
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
            Some(UnitAction::Start)
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE)),
            Some(UnitAction::Stop)
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
            Some(UnitAction::Restart)
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT)),
            Some(UnitAction::Reload)
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)),
            Some(UnitAction::Enable)
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)),
            Some(UnitAction::Disable)
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE)),
            Some(UnitAction::Mask)
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE)),
            Some(UnitAction::Unmask)
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            None
        );
    }

    #[tokio::test]
    async fn log_view_editor_binding_opens_text_editor() {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.view_mode = ViewMode::LogView;
        app.unit_list.selected_key.name = "ssh.service".to_string();
        app.log_view.logs = vec!["line 1".to_string(), "line 2".to_string()];

        let handled = app
            .handle_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE))
            .await
            .unwrap();

        assert!(handled);
        match app.pending_action {
            Some(PendingAction::EditText { filename, content }) => {
                assert_eq!(filename, "log-ssh.service.txt");
                assert_eq!(content, "line 1\nline 2");
            }
            _ => panic!("expected EditText pending action"),
        }
    }
}
