use crossterm::event::{KeyCode, KeyEvent};
use nucleo_matcher::{
    Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};

use crate::{
    app::state::context::{App, SearchInputAction},
    models::UnitInfo,
};

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub is_active: bool,
    pub cursor: usize,
}

impl SearchState {
    pub fn start(&mut self) {
        self.is_active = true;
        self.cursor = self.query.chars().count();
    }

    pub fn clear(&mut self) {
        self.is_active = false;
        self.query.clear();
        self.cursor = 0;
    }
}

impl App {
    pub fn start_search(&mut self) {
        self.search.start();
    }

    pub fn clear_search(&mut self) {
        self.search.clear();
    }

    pub fn search_score(&self, unit: &UnitInfo) -> Option<u32> {
        if self.search.query.is_empty() {
            return None;
        }

        let mut matcher = self.matcher.borrow_mut();
        let pattern = Pattern::parse(
            &self.search.query,
            CaseMatching::Ignore,
            Normalization::Smart,
        );
        let target = format!("{} {}", unit.name, unit.description);
        let mut buffer = Vec::new();
        pattern.score(Utf32Str::new(target.as_str(), &mut buffer), &mut matcher)
    }

    pub fn unit_matches_search(&self, unit: &UnitInfo) -> bool {
        self.search.query.is_empty() || self.search_score(unit).is_some()
    }

    pub fn handle_search_key(&mut self, key: KeyEvent) -> Option<SearchInputAction> {
        match key.code {
            KeyCode::Left | KeyCode::Right => {
                edit_search_key_impl(key, &mut self.search.query, &mut self.search.cursor);
                Some(SearchInputAction::Cursor)
            }
            KeyCode::Backspace | KeyCode::Char(_) => {
                edit_search_key_impl(key, &mut self.search.query, &mut self.search.cursor);
                Some(SearchInputAction::Edit)
            }
            _ => None,
        }
    }
}

fn edit_search_key_impl(key: KeyEvent, query: &mut String, cursor: &mut usize) {
    match key.code {
        KeyCode::Left => {
            *cursor = cursor.saturating_sub(1);
        }
        KeyCode::Right => {
            *cursor = (*cursor + 1).min(query.chars().count());
        }
        KeyCode::Backspace if *cursor > 0 => {
            let idx = char_to_byte_index(query, *cursor - 1);
            let end = char_to_byte_index(query, *cursor);
            query.replace_range(idx..end, "");
            *cursor -= 1;
        }
        KeyCode::Backspace => {}
        KeyCode::Char(c) => {
            let idx = char_to_byte_index(query, *cursor);
            query.insert(idx, c);
            *cursor += 1;
        }
        _ => {}
    }
}

fn char_to_byte_index(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or_else(|| value.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use tokio::sync::mpsc;
    use zbus::zvariant::OwnedObjectPath;

    use crate::models::UnitInfo;

    fn test_app(units: Vec<UnitInfo>) -> App {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.unit_list.units = units;
        app.is_loading = false;
        app
    }

    fn unit(
        name: &str,
        description: &str,
        load_state: &str,
        active_state: &str,
        enablement_state: &str,
        path: &str,
    ) -> UnitInfo {
        UnitInfo {
            name: name.to_string(),
            description: description.to_string(),
            scope: "global".to_string(),
            load_state: load_state.to_string(),
            active_state: active_state.to_string(),
            enablement_state: enablement_state.to_string(),
            sub_state: active_state.to_string(),
            path: OwnedObjectPath::try_from(path).unwrap(),
        }
    }

    #[test]
    fn entering_search_mode_preserves_existing_query() {
        let mut app = test_app(vec![unit(
            "alpha.service",
            "Alpha",
            "loaded",
            "active",
            "enabled",
            "/test/unit/alpha",
        )]);

        app.search.query = "foo".to_string();

        app.start_search();

        assert_eq!(app.search.query, "foo");
        assert!(app.search.is_active);
    }

    #[test]
    fn handle_search_key_moves_cursor_and_inserts_text() {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.search.query = "foo".to_string();
        app.search.cursor = 1;

        app.handle_search_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.search.cursor, 0);

        app.handle_search_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(app.search.query, "xfoo");
        assert_eq!(app.search.cursor, 1);
    }
}
