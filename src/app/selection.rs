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

        self.file_view.content = edited_content.clone();
        self.file_view.path = request.mode.draft_label(&request.unit_name);
        self.file_view.scroll = 0;
        self.pending_edit_review = Some(EditReview {
            unit_name: request.unit_name.clone(),
            scope: request.scope.clone(),
            mode: request.mode,
            edited_content,
            restore_content: request.restore_content,
            restore_path: request.restore_path,
        });
    }

    pub fn get_selected_unit(&self) -> Option<&UnitInfo> {
        self.selected_unit_index()
            .map(|i| &self.unit_list.units[self.unit_list.filtered_indices[i]])
    }

    pub fn build_edit_request(&self, mode: UnitEditMode) -> Option<EditRequest> {
        let unit = self.get_selected_unit()?;
        let initial_content = match mode {
            UnitEditMode::Override => {
                build_override_template(&unit.name, &self.file_view.path)
            }
            UnitEditMode::Full => self.file_view.content.clone(),
        };

        Some(EditRequest {
            unit_name: unit.name.clone(),
            scope: unit.scope.clone(),
            mode,
            initial_content,
            restore_content: self.file_view.content.clone(),
            restore_path: self.file_view.path.clone(),
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
            && let Some(index) = self.unit_list.filtered_indices.iter().position(|&unit_index| {
                let unit = &self.unit_list.units[unit_index];
                unit.name == unit_key.name
                    && unit.scope == unit_key.scope
                    && unit.path.to_string() == unit_key.path
            })
        {
            self.unit_list.select_index(Some(index));
            return;
        }

        if self.unit_list.filtered_indices.is_empty() {
            self.unit_list.select_index(None);
        } else {
            self.unit_list.select_index(Some(0));
        }
    }

    pub fn selected_unit_index(&self) -> Option<usize> {
        self.unit_list.state
            .selected()
            .filter(|&index| index < self.unit_list.filtered_indices.len())
    }

    pub fn discard_edit_review(&mut self) {
        if let Some(review) = self.pending_edit_review.take() {
            self.file_view.content = review.restore_content;
            self.file_view.path = review.restore_path;
            self.file_view.scroll = 0;
        }
    }
}
