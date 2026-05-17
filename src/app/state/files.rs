use crate::app::state::context::{App, ViewMode};

#[derive(Default)]
pub struct FileViewState {
    pub content: String,
    pub path: String,
    pub scroll: u16,
    pub search_match: Option<usize>,
}

impl FileViewState {
    pub fn move_scroll(&mut self, delta: i32) {
        let total_lines = self.content.lines().count() as i32;
        let next_scroll = self.scroll as i32 + delta;
        self.scroll = next_scroll.clamp(0, total_lines.saturating_sub(1).max(0)) as u16;
    }
}

impl App {
    pub async fn enter_file_view(&mut self) {
        if let Some(unit) = self.get_selected_unit() {
            let unit_clone = unit.clone();
            self.search.clear();
            self.view_mode = ViewMode::FileView;
            self.file_view.content.clear();
            self.file_view.path.clear();
            self.file_view.scroll = 0;
            self.fetch_unit_file(unit_clone).await;
        }
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

        let current = self
            .file_view
            .search_match
            .unwrap_or(self.file_view.scroll as usize);
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
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn file_search_helpers_match_exact_text_and_cycle() {
        let mut app = test_app(vec![unit(
            "alpha.service",
            "Alpha",
            "loaded",
            "active",
            "enabled",
            "/test/unit/alpha",
        )]);
        app.file_view.content =
            "[Service]\nExecStart=/usr/bin/foo\nExecStartPost=/usr/bin/foo --flag\n".to_string();
        app.search.query = "ExecStart".to_string();

        assert_eq!(app.file_search_matches(), vec![1, 2]);

        app.file_view.scroll = 0;
        app.cycle_file_search_match(true);
        assert_eq!(app.file_view.search_match, Some(1));
        assert_eq!(app.file_view.scroll, 1);

        app.cycle_file_search_match(true);
        assert_eq!(app.file_view.search_match, Some(2));
        assert_eq!(app.file_view.scroll, 2);

        app.cycle_file_search_match(false);
        assert_eq!(app.file_view.search_match, Some(1));
        assert_eq!(app.file_view.scroll, 1);
    }
}
