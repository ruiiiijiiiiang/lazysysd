use crossterm::event::{KeyCode, KeyEvent};

use crate::app::state::{App, SearchInputAction};

impl App {
    pub fn start_search(&mut self) {
        self.search.start();
    }

    pub fn clear_search(&mut self) {
        self.search.clear();
    }

    pub fn file_search_matches(&self) -> Vec<usize> {
        if self.search.query.is_empty() {
            return Vec::new();
        }

        self.file_view
            .content
            .lines()
            .enumerate()
            .filter(|(_, line)| line.contains(&self.search.query))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn cycle_file_search_match(&mut self, forward: bool) {
        let matches = self.file_search_matches();
        if matches.is_empty() {
            self.file_view.search_match = None;
            return;
        }

        let current = self.file_view.search_match.unwrap_or(self.file_view.scroll as usize);
        let next_index = if forward {
            matches
                .iter()
                .copied()
                .find(|&index| index > current)
                .unwrap_or(matches[0])
        } else {
            matches
                .iter()
                .copied()
                .rev()
                .find(|&index| index < current)
                .unwrap_or(*matches.last().unwrap())
        };

        self.file_view.search_match = Some(next_index);
        self.file_view.scroll = next_index as u16;
    }

    pub fn clear_log_visual_modes(&mut self) {
        self.log_view.clear_visual_modes();
    }

    pub fn log_search_matches(&self) -> Vec<usize> {
        if self.search.query.is_empty() {
            return Vec::new();
        }

        self.log_view
            .logs
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(&self.search.query))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn cycle_log_search_match(&mut self, forward: bool) {
        let matches = self.log_search_matches();
        if matches.is_empty() {
            return;
        }

        let current = self.log_view.state.selected().unwrap_or(0);
        let next_index = if forward {
            matches
                .iter()
                .copied()
                .find(|&index| index > current)
                .unwrap_or(matches[0])
        } else {
            matches
                .iter()
                .copied()
                .rev()
                .find(|&index| index < current)
                .unwrap_or(*matches.last().unwrap())
        };

        self.log_view.state.select(Some(next_index));
    }

    pub fn toggle_log_line_mark(&mut self) {
        self.log_view.toggle_line_mark();
    }

    pub fn selected_log_line_range(&self) -> Option<(usize, usize)> {
        self.log_view.selected_line_range()
    }

    pub fn selected_log_lines_text(&self) -> Option<String> {
        self.log_view.selected_lines_text()
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
