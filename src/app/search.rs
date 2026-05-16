use crossterm::event::{KeyCode, KeyEvent};

use crate::app::state::{App, ViewMode};

impl App {
    pub fn clear_file_search(&mut self) {
        self.file_search_mode = false;
        self.file_search_query.clear();
        self.file_search_cursor = 0;
        self.file_search_match = None;
    }

    pub fn start_file_search(&mut self) {
        self.file_search_mode = true;
        self.file_search_cursor = self.file_search_query.chars().count();
        self.file_search_match = None;
    }

    pub fn file_search_matches(&self) -> Vec<usize> {
        if self.file_search_query.is_empty() {
            return Vec::new();
        }

        self.unit_file_content
            .lines()
            .enumerate()
            .filter(|(_, line)| line.contains(&self.file_search_query))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn cycle_file_search_match(&mut self, forward: bool) {
        let matches = self.file_search_matches();
        if matches.is_empty() {
            self.file_search_match = None;
            return;
        }

        let current = self.file_search_match.unwrap_or(self.file_scroll as usize);
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

        self.file_search_match = Some(next_index);
        self.file_scroll = next_index as u16;
    }

    pub fn clear_log_visual_modes(&mut self) {
        self.visual_select = false;
        self.visual_line_select = false;
        self.selected_log_lines.clear();
        self.selected_log_line_marks.clear();
    }

    pub fn clear_log_search(&mut self) {
        self.log_search_mode = false;
        self.log_search_query.clear();
        self.log_search_cursor = 0;
    }

    pub fn start_log_search(&mut self) {
        self.log_search_mode = true;
        self.log_search_cursor = self.log_search_query.chars().count();
    }

    pub fn edit_unit_search_key(&mut self, key: KeyEvent) {
        edit_search_key_impl(key, &mut self.search_query, &mut self.search_cursor);
    }

    pub fn edit_log_search_key(&mut self, key: KeyEvent) {
        edit_search_key_impl(key, &mut self.log_search_query, &mut self.log_search_cursor);
    }

    pub fn edit_file_search_key(&mut self, key: KeyEvent) {
        edit_search_key_impl(
            key,
            &mut self.file_search_query,
            &mut self.file_search_cursor,
        );
    }

    pub fn log_search_matches(&self) -> Vec<usize> {
        if self.log_search_query.is_empty() {
            return Vec::new();
        }

        self.unit_logs
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(&self.log_search_query))
            .map(|(index, _)| index)
            .collect()
    }

    pub fn cycle_log_search_match(&mut self, forward: bool) {
        let matches = self.log_search_matches();
        if matches.is_empty() {
            return;
        }

        let current = self.log_state.selected().unwrap_or(0);
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

        self.log_state.select(Some(next_index));
    }

    pub fn toggle_log_line_mark(&mut self) {
        let Some(index) = self.log_state.selected() else {
            return;
        };

        if self.selected_log_line_marks.contains(&index) {
            self.selected_log_line_marks.retain(|&i| i != index);
            return;
        }

        if self.selected_log_line_marks.len() == 2 {
            self.selected_log_line_marks.remove(0);
        }

        self.selected_log_line_marks.push(index);
    }

    pub fn selected_log_line_range(&self) -> Option<(usize, usize)> {
        match self.selected_log_line_marks.as_slice() {
            [only] => Some((*only, *only)),
            [start, end] => Some(((*start).min(*end), (*start).max(*end))),
            _ => None,
        }
    }

    pub fn selected_log_lines_text(&self) -> Option<String> {
        let (start, end) = self.selected_log_line_range()?;
        let lines: Vec<&str> = (start..=end)
            .filter_map(|index| self.unit_logs.get(index).map(|line| line.as_str()))
            .collect();

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    pub fn set_search_cursor_to_end(&mut self) {
        match self.view_mode {
            ViewMode::UnitList => self.search_cursor = self.search_query.chars().count(),
            ViewMode::LogView => self.log_search_cursor = self.log_search_query.chars().count(),
            ViewMode::FileView => self.file_search_cursor = self.file_search_query.chars().count(),
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
