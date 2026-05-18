use crate::app::state::{
    context::{App, NavAction, ViewMode},
    file::FileViewState,
    log::LogViewState,
    unit::UnitListState,
};

pub trait Navigable {
    fn navigate(&mut self, action: NavAction, height: u16);
}

impl Navigable for UnitListState {
    fn navigate(&mut self, action: NavAction, height: u16) {
        let half_height = height as i32 / 2;
        match action {
            NavAction::Up => self.move_selection(-1),
            NavAction::Down => self.move_selection(1),
            NavAction::HalfPageUp => self.move_selection(-half_height),
            NavAction::HalfPageDown => self.move_selection(half_height),
            NavAction::PageUp => self.move_selection(-(height as i32)),
            NavAction::PageDown => self.move_selection(height as i32),
            NavAction::Top => self.select_index(Some(0)),
            NavAction::Bottom => {
                if !self.filtered_indices.is_empty() {
                    self.select_index(Some(self.filtered_indices.len().saturating_sub(1)));
                }
            }
        }
    }
}

impl Navigable for LogViewState {
    fn navigate(&mut self, action: NavAction, height: u16) {
        let half_height = height as i32 / 2;
        match action {
            NavAction::Up => self.move_selection(-1),
            NavAction::Down => self.move_selection(1),
            NavAction::HalfPageUp => self.move_selection(-half_height),
            NavAction::HalfPageDown => self.move_selection(half_height),
            NavAction::PageUp => self.move_selection(-(height as i32)),
            NavAction::PageDown => self.move_selection(height as i32),
            NavAction::Top => self.state.select(Some(0)),
            NavAction::Bottom => {
                if !self.logs.is_empty() {
                    self.state.select(Some(self.logs.len().saturating_sub(1)));
                }
            }
        }
    }
}

impl Navigable for FileViewState {
    fn navigate(&mut self, action: NavAction, height: u16) {
        let half_height = height as i32 / 2;
        let total_lines = self.content.lines().count() as i32;
        match action {
            NavAction::Up => self.move_scroll(-1),
            NavAction::Down => self.move_scroll(1),
            NavAction::HalfPageUp => self.move_scroll(-half_height),
            NavAction::HalfPageDown => self.move_scroll(half_height),
            NavAction::PageUp => self.move_scroll(-(height as i32)),
            NavAction::PageDown => self.move_scroll(height as i32),
            NavAction::Top => self.scroll = 0,
            NavAction::Bottom => {
                self.scroll = total_lines.saturating_sub(height as i32).max(0) as u16
            }
        }
    }
}

impl App {
    pub fn perform_nav(&mut self, action: NavAction) {
        let height = self.main_content_height;

        match self.view_mode {
            ViewMode::UnitList => self.unit_list.navigate(action, height),
            ViewMode::LogView => self.log_view.navigate(action, height),
            ViewMode::FileView => self.file_view.navigate(action, height),
        }
    }
}
