use std::io::Result;

use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    terminal,
};

use crate::{
    app::state::{App, FilterMenu, NavAction, ViewMode, copy_to_clipboard},
    models::{AppInternalEvent, PendingAction, PrivilegedAction, UnitEditMode},
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

        if self.view_mode == ViewMode::LogView {
            if self.log_search_mode {
                match key.code {
                    KeyCode::Esc => {
                        self.clear_log_search();
                    }
                    KeyCode::Enter => {
                        self.log_search_mode = false;
                    }
                    KeyCode::Left | KeyCode::Right => {
                        self.edit_log_search_key(key);
                    }
                    KeyCode::Backspace | KeyCode::Char(_) => {
                        self.edit_log_search_key(key);
                        self.cycle_log_search_match(true);
                    }
                    _ => {}
                }
                return Ok(false);
            }

            if self.visual_line_select {
                if self.handle_nav_key(key) {
                    return Ok(false);
                }
                match key.code {
                    KeyCode::Esc => {
                        self.visual_line_select = false;
                        self.selected_log_line_marks.clear();
                    }
                    KeyCode::Char(' ') => {
                        self.toggle_log_line_mark();
                    }
                    KeyCode::Char('y') | KeyCode::Enter => {
                        if let Some(text) = self.selected_log_lines_text() {
                            let cloned = text.clone();
                            tokio::task::spawn_blocking(move || copy_to_clipboard(&cloned));
                        }
                        self.visual_line_select = false;
                        self.selected_log_line_marks.clear();
                    }
                    _ => {}
                }
                return Ok(false);
            }

            if self.visual_select {
                if self.handle_nav_key(key) {
                    return Ok(false);
                }
                match key.code {
                    KeyCode::Esc => {
                        self.clear_log_visual_modes();
                    }
                    KeyCode::Char(' ') => {
                        if let Some(i) = self.log_state.selected()
                            && !self.selected_log_lines.remove(&i)
                        {
                            self.selected_log_lines.insert(i);
                        }
                    }
                    KeyCode::Char('y') | KeyCode::Enter => {
                        let mut indices: Vec<_> = self.selected_log_lines.iter().collect();
                        indices.sort();
                        let selected: Vec<&String> = indices
                            .into_iter()
                            .filter_map(|&i| self.unit_logs.get(i))
                            .collect();
                        if !selected.is_empty() {
                            let text = selected
                                .into_iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join("\n");
                            let cloned = text.clone();
                            tokio::task::spawn_blocking(move || copy_to_clipboard(&cloned));
                        }
                        self.clear_log_visual_modes();
                    }
                    _ => {}
                }
                return Ok(false);
            }

            if self.handle_nav_key(key) {
                return Ok(false);
            }

            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view_mode = ViewMode::UnitList;
                    self.clear_log_visual_modes();
                    self.clear_log_search();
                }
                KeyCode::Char('v') if !self.unit_logs.is_empty() => {
                    self.visual_select = true;
                    if self.log_state.selected().is_none() {
                        self.log_state.select(Some(0));
                    }
                }
                KeyCode::Char('V') if !self.unit_logs.is_empty() => {
                    self.visual_line_select = true;
                    self.selected_log_line_marks.clear();
                    if self.log_state.selected().is_none() {
                        self.log_state.select(Some(0));
                    }
                }
                KeyCode::Char('v') => {}
                KeyCode::Char('V') => {}
                KeyCode::Char('/') if !self.unit_logs.is_empty() => {
                    self.start_log_search();
                }
                KeyCode::Char('n') if !self.log_search_query.is_empty() => {
                    self.cycle_log_search_match(true);
                }
                KeyCode::Char('N') if !self.log_search_query.is_empty() => {
                    self.cycle_log_search_match(false);
                }
                KeyCode::Char('e') if !self.unit_logs.is_empty() => {
                    self.pending_action = Some(PendingAction::EditText {
                        filename: format!("log-{}.txt", self.selected_unit_key.name),
                        content: self.unit_logs.join("\n"),
                    });
                    return Ok(true);
                }
                KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(unit) = self.get_selected_unit() {
                        let name = unit.name.clone();
                        let scope = unit.scope.clone();
                        self.fetch_unit_logs(name, scope).await;
                    }
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.view_mode == ViewMode::FileView {
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
                return Ok(false);
            }

            if self.handle_nav_key(key) {
                return Ok(false);
            }
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view_mode = ViewMode::UnitList;
                    self.clear_file_search();
                }
                KeyCode::Char('e') if !self.unit_file_path.is_empty() => {
                    if let Some(request) = self.build_edit_request(UnitEditMode::Override) {
                        self.pending_action = Some(PendingAction::EditFile(request));
                        return Ok(true);
                    }
                }
                KeyCode::Char('E') if !self.unit_file_path.is_empty() => {
                    if let Some(request) = self.build_edit_request(UnitEditMode::Full) {
                        self.pending_action = Some(PendingAction::EditFile(request));
                        return Ok(true);
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
            return Ok(false);
        }

        if self.is_searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.is_searching = false;
                }
                KeyCode::Left | KeyCode::Right | KeyCode::Backspace | KeyCode::Char(_) => {
                    self.edit_unit_search_key(key);
                    self.update_filter();
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.open_filter_menu.is_some() {
            self.handle_filter_menu_key(key);
            return Ok(false);
        }

        if self.handle_nav_key(key) {
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('q') => Ok(true),
            KeyCode::Char('/') => {
                self.is_searching = true;
                self.set_search_cursor_to_end();
                Ok(false)
            }
            KeyCode::Char('a') => {
                self.open_filter_menu = Some(FilterMenu::Active);
                Ok(false)
            }
            KeyCode::Char('n') => {
                self.open_filter_menu = Some(FilterMenu::Enablement);
                Ok(false)
            }
            KeyCode::Char('o') => {
                self.open_filter_menu = Some(FilterMenu::Load);
                Ok(false)
            }
            KeyCode::Char('p') => {
                self.open_filter_menu = Some(FilterMenu::Scope);
                Ok(false)
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
                Ok(false)
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
                Ok(false)
            }
            _ => {
                if let Some(action) = unit_command_for_key(key) {
                    self.trigger_selected_unit_command(action).await?;
                }
                Ok(false)
            }
        }
    }

    pub fn handle_nav_key(&mut self, key: KeyEvent) -> bool {
        if self.pending_nav_prefix == Some('g') {
            self.pending_nav_prefix = None;
            if key.code == KeyCode::Char('g') {
                self.perform_nav(NavAction::Top);
                return true;
            }
        }

        let action = match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(NavAction::Down),
            KeyCode::Char('k') | KeyCode::Up => Some(NavAction::Up),
            KeyCode::Char('G') => Some(NavAction::Bottom),
            KeyCode::Char('g') => {
                self.pending_nav_prefix = Some('g');
                return true;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(NavAction::HalfPageUp)
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(NavAction::HalfPageDown)
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(NavAction::PageDown)
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(NavAction::PageUp)
            }
            _ => None,
        };

        if let Some(action) = action {
            self.perform_nav(action);
            true
        } else {
            false
        }
    }

    pub fn perform_nav(&mut self, action: NavAction) {
        let height = self.last_area_height as i32;
        let half_height = height / 2;

        match self.view_mode {
            ViewMode::UnitList => match action {
                NavAction::Up => self.move_selection(-1),
                NavAction::Down => self.move_selection(1),
                NavAction::HalfPageUp => self.move_selection(-half_height),
                NavAction::HalfPageDown => self.move_selection(half_height),
                NavAction::PageUp => self.move_selection(-height),
                NavAction::PageDown => self.move_selection(height),
                NavAction::Top => self.select_filtered_unit_index(Some(0)),
                NavAction::Bottom => {
                    if !self.filtered_units.is_empty() {
                        self.select_filtered_unit_index(Some(
                            self.filtered_units.len().saturating_sub(1),
                        ));
                    }
                }
            },
            ViewMode::LogView => match action {
                NavAction::Up => self.move_log_selection(-1),
                NavAction::Down => self.move_log_selection(1),
                NavAction::HalfPageUp => self.move_log_selection(-half_height),
                NavAction::HalfPageDown => self.move_log_selection(half_height),
                NavAction::PageUp => self.move_log_selection(-height),
                NavAction::PageDown => self.move_log_selection(height),
                NavAction::Top => self.log_state.select(Some(0)),
                NavAction::Bottom => {
                    if !self.unit_logs.is_empty() {
                        self.log_state
                            .select(Some(self.unit_logs.len().saturating_sub(1)));
                    }
                }
            },
            ViewMode::FileView => {
                let total_lines = self.unit_file_content.lines().count() as i32;
                match action {
                    NavAction::Up => self.file_scroll = self.file_scroll.saturating_sub(1),
                    NavAction::Down => self.file_scroll = self.file_scroll.saturating_add(1),
                    NavAction::HalfPageUp => {
                        self.file_scroll = self.file_scroll.saturating_sub(half_height as u16)
                    }
                    NavAction::HalfPageDown => {
                        self.file_scroll = self.file_scroll.saturating_add(half_height as u16)
                    }
                    NavAction::PageUp => {
                        self.file_scroll = self.file_scroll.saturating_sub(height as u16)
                    }
                    NavAction::PageDown => {
                        self.file_scroll = self.file_scroll.saturating_add(height as u16)
                    }
                    NavAction::Top => self.file_scroll = 0,
                    NavAction::Bottom => {
                        self.file_scroll = total_lines.saturating_sub(height).max(0) as u16
                    }
                }
                self.file_scroll = self
                    .file_scroll
                    .min(total_lines.saturating_sub(1).max(0) as u16);
            }
        }
    }

    pub fn handle_filter_menu_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.open_filter_menu = None;
            }
            KeyCode::Char(c) => {
                if let Some(menu) = self.open_filter_menu {
                    let selected_hotkey = c.to_ascii_lowercase();
                    if let Some(option) = self
                        .filter_menu_options(menu)
                        .into_iter()
                        .find(|option| option.hotkey == selected_hotkey)
                    {
                        menu.set_selected_value(self, option.value);
                        self.open_filter_menu = None;
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
                    let (cols, rows) = terminal::size().unwrap_or((80, 24));
                    self.start_embedded_auth(
                        PrivilegedAction::ApplyEdit {
                            unit_name: review.unit_name.clone(),
                            scope: review.scope.clone(),
                            mode: review.mode,
                            content: review.edited_content.clone(),
                        },
                        cols,
                        rows,
                    )
                    .await?;
                }
            }
            _ => {}
        }

        Ok(false)
    }

    pub async fn handle_internal_event(&mut self, event: AppInternalEvent) {
        match event {
            AppInternalEvent::UnitsLoaded(units) => {
                self.units = units;
                self.is_loading = false;
                self.update_filter();
            }
            AppInternalEvent::LogsLoaded(logs) => {
                self.unit_logs = logs;
                self.is_loading = false;
                self.log_state
                    .select(Some(self.unit_logs.len().saturating_sub(1)));
            }
            AppInternalEvent::FileLoaded(content, path) => {
                self.unit_file_content = content;
                self.unit_file_path = path;
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
                    tokio::task::spawn_blocking(move || {
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
                            self.unit_file_content.clear();
                            self.unit_file_path.clear();
                            self.file_scroll = 0;
                            self.refresh_units().await;
                        }
                        None => {}
                    }
                }
            }
            AppInternalEvent::Error(_err) => {
                self.is_loading = false;
            }
        }
    }
}

pub fn unit_command_for_key(key: KeyEvent) -> Option<&'static str> {
    if !(key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT) {
        return None;
    }

    match key.code {
        KeyCode::Char('s') => Some("start"),
        KeyCode::Char('t') => Some("stop"),
        KeyCode::Char('r') => Some("restart"),
        KeyCode::Char('R') => Some("reload"),
        KeyCode::Char('e') => Some("enable"),
        KeyCode::Char('d') => Some("disable"),
        KeyCode::Char('m') => Some("mask"),
        KeyCode::Char('u') => Some("unmask"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn unit_command_bindings_match_expected_actions() {
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE)),
            Some("start")
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE)),
            Some("stop")
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
            Some("restart")
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT)),
            Some("reload")
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE)),
            Some("enable")
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE)),
            Some("disable")
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE)),
            Some("mask")
        );
        assert_eq!(
            unit_command_for_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE)),
            Some("unmask")
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
        app.selected_unit_key.name = "ssh.service".to_string();
        app.unit_logs = vec!["line 1".to_string(), "line 2".to_string()];

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
