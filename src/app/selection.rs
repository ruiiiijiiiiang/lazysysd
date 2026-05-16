use std::io::Result;

use crossterm::terminal;

use crate::{
    app::state::{App, UnitSelectionKey, build_override_template},
    models::{EditRequest, EditReview, PrivilegedAction, UnitEditMode, UnitInfo},
};

impl App {
    pub fn finish_edit_request(&mut self, request: EditRequest, edited_content: String) {
        if edited_content == request.initial_content {
            return;
        }

        self.unit_file_content = edited_content.clone();
        self.unit_file_path = request.mode.draft_label(&request.unit_name);
        self.file_scroll = 0;
        self.pending_edit_review = Some(EditReview {
            unit_name: request.unit_name.clone(),
            scope: request.scope.clone(),
            mode: request.mode,
            edited_content,
            restore_content: request.restore_content,
            restore_path: request.restore_path,
        });
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.filtered_units.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                let next = i as i32 + delta;
                if next < 0 {
                    0
                } else if next >= self.filtered_units.len() as i32 {
                    self.filtered_units.len() - 1
                } else {
                    next as usize
                }
            }
            None => 0,
        };
        self.select_filtered_unit_index(Some(i));
    }

    pub fn move_log_selection(&mut self, delta: i32) {
        if self.unit_logs.is_empty() {
            return;
        }
        let i = match self.log_state.selected() {
            Some(i) => {
                let next = i as i32 + delta;
                if next < 0 {
                    0
                } else if next >= self.unit_logs.len() as i32 {
                    self.unit_logs.len() - 1
                } else {
                    next as usize
                }
            }
            None => {
                if delta > 0 {
                    0
                } else {
                    self.unit_logs.len().saturating_sub(1)
                }
            }
        };
        self.log_state.select(Some(i));
    }

    pub fn get_selected_unit(&self) -> Option<&UnitInfo> {
        self.selected_unit_index()
            .map(|i| &self.units[self.filtered_units[i]])
    }

    pub fn selected_unit_key_for(unit: &UnitInfo) -> UnitSelectionKey {
        UnitSelectionKey {
            name: unit.name.clone(),
            scope: unit.scope.clone(),
            path: unit.path.to_string(),
        }
    }

    pub fn build_edit_request(&self, mode: UnitEditMode) -> Option<EditRequest> {
        let unit = self.get_selected_unit()?;
        let initial_content = match mode {
            UnitEditMode::Override => build_override_template(&unit.name, &self.unit_file_path),
            UnitEditMode::Full => self.unit_file_content.clone(),
        };

        Some(EditRequest {
            unit_name: unit.name.clone(),
            scope: unit.scope.clone(),
            mode,
            initial_content,
            restore_content: self.unit_file_content.clone(),
            restore_path: self.unit_file_path.clone(),
        })
    }

    pub async fn trigger_selected_unit_command(&mut self, action: &str) -> Result<()> {
        if let Some(unit) = self.get_selected_unit() {
            let (cols, rows) = terminal::size().unwrap_or((80, 24));
            self.start_embedded_auth(
                PrivilegedAction::UnitCommand {
                    unit_name: unit.name.clone(),
                    scope: unit.scope.clone(),
                    action: action.to_string(),
                },
                cols,
                rows,
            )
            .await?;
        }
        Ok(())
    }

    pub fn restore_selection(&mut self, selected_unit_key: Option<&UnitSelectionKey>) {
        if let Some(unit_key) = selected_unit_key
            && let Some(index) = self.filtered_units.iter().position(|&unit_index| {
                let unit = &self.units[unit_index];
                unit.name == unit_key.name
                    && unit.scope == unit_key.scope
                    && unit.path.to_string() == unit_key.path
            })
        {
            self.select_filtered_unit_index(Some(index));
            return;
        }

        if self.filtered_units.is_empty() {
            self.select_filtered_unit_index(None);
        } else {
            self.select_filtered_unit_index(Some(0));
        }
    }

    pub fn selected_unit_index(&self) -> Option<usize> {
        self.list_state
            .selected()
            .filter(|&index| index < self.filtered_units.len())
    }

    pub fn select_filtered_unit_index(&mut self, index: Option<usize>) {
        self.list_state.select(index);
        self.selected_unit_key = index
            .and_then(|i| self.filtered_units.get(i).copied())
            .and_then(|unit_index| self.units.get(unit_index))
            .map(Self::selected_unit_key_for)
            .unwrap_or_default();
    }

    pub fn discard_edit_review(&mut self) {
        if let Some(review) = self.pending_edit_review.take() {
            self.unit_file_content = review.restore_content;
            self.unit_file_path = review.restore_path;
            self.file_scroll = 0;
        }
    }
}
