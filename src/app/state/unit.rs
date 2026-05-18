use std::io::Result;

use ratatui::widgets::ListState;

use crate::{
    app::{
        state::context::{App, FilterMenu, UnitSelectionKey, ViewMode},
        utils::build_override_template,
    },
    models::{
        EditRequest, EditReview, PrivilegedAction, UnitAction, UnitActiveState, UnitEditMode,
        UnitEnablementState, UnitInfo, UnitLoadState, UnitScope,
    },
};

#[derive(Default)]
pub struct UnitListState {
    pub units: Vec<UnitInfo>,
    pub filtered_indices: Vec<usize>,
    pub state: ListState,
    pub selected_key: UnitSelectionKey,
    pub type_filter: Option<String>,
    pub active_filter: Option<UnitActiveState>,
    pub enablement_filter: Option<UnitEnablementState>,
    pub load_filter: Option<UnitLoadState>,
    pub scope_filter: Option<UnitScope>,
    pub open_filter_menu: Option<FilterMenu>,
}

impl UnitListState {
    pub fn select_index(&mut self, index: Option<usize>) {
        self.state.select(index);
        self.selected_key = index
            .and_then(|i| self.filtered_indices.get(i).copied())
            .and_then(|unit_index| self.units.get(unit_index))
            .map(|unit| UnitSelectionKey {
                name: unit.name.clone(),
                scope: unit.scope,
                path: unit.path.to_string(),
            })
            .unwrap_or_default();
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                let next = i as i32 + delta;
                if next < 0 {
                    0
                } else if next >= self.filtered_indices.len() as i32 {
                    self.filtered_indices.len() - 1
                } else {
                    next as usize
                }
            }
            None => 0,
        };
        self.select_index(Some(i));
    }
}

impl App {
    pub fn enter_unit_list_view(&mut self) {
        self.view_mode = ViewMode::UnitList;
        self.search.clear();
        self.log_view.clear_visual_modes();
        self.file_view.search_match = None;
    }

    pub fn matches_filter_value(selected: Option<&str>, actual: &str) -> bool {
        match selected {
            Some(expected) => expected == actual,
            None => true,
        }
    }

    pub fn finish_edit_request(&mut self, request: EditRequest, edited_content: String) {
        if edited_content == request.initial_content {
            return;
        }

        self.file_view.content = edited_content.clone();
        self.file_view.path = request.mode.draft_label(&request.unit_name);
        self.file_view.scroll = 0;
        self.pending_edit_review = Some(EditReview {
            unit_name: request.unit_name.clone(),
            scope: request.scope,
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
            UnitEditMode::Override => build_override_template(&unit.name, &self.file_view.path),
            UnitEditMode::Full => self.file_view.content.clone(),
        };

        Some(EditRequest {
            unit_name: unit.name.clone(),
            scope: unit.scope,
            mode,
            initial_content,
            restore_content: self.file_view.content.clone(),
            restore_path: self.file_view.path.clone(),
        })
    }

    pub async fn trigger_selected_unit_command(&mut self, action: UnitAction) -> Result<()> {
        if let Some(unit) = self.get_selected_unit() {
            self.start_embedded_auth(PrivilegedAction::UnitCommand {
                unit_name: unit.name.clone(),
                scope: unit.scope,
                action,
            })
            .await?;
        }
        Ok(())
    }

    pub fn restore_selection(&mut self, selected_unit_key: Option<&UnitSelectionKey>) {
        if let Some(unit_key) = selected_unit_key
            && let Some(index) = self
                .unit_list
                .filtered_indices
                .iter()
                .position(|&unit_index| {
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
        self.unit_list
            .state
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use zbus::zvariant::OwnedObjectPath;

    use crate::models::{EditRequest, UnitEditMode, UnitInfo};

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
        load_state: UnitLoadState,
        active_state: UnitActiveState,
        enablement_state: UnitEnablementState,
        path: &str,
    ) -> UnitInfo {
        UnitInfo {
            name: name.to_string(),
            description: description.to_string(),
            scope: UnitScope::Global,
            load_state,
            active_state,
            enablement_state,
            sub_state: active_state.to_string(),
            path: OwnedObjectPath::try_from(path).unwrap(),
            fragment_path: format!("/etc/systemd/system/{name}"),
        }
    }

    #[test]
    fn finish_edit_request_creates_review_and_discard_restores_preview() {
        let mut app = test_app(vec![unit(
            "ssh.service",
            "Secure Shell",
            UnitLoadState::Loaded,
            UnitActiveState::Active,
            UnitEnablementState::Enabled,
            "/test/unit/ssh",
        )]);
        app.file_view.content = "[Service]\nExecStart=/usr/bin/ssh\n".to_string();
        app.file_view.path = "/usr/lib/systemd/system/ssh.service".to_string();

        app.finish_edit_request(
            EditRequest {
                unit_name: "ssh.service".to_string(),
                scope: UnitScope::Global,
                mode: UnitEditMode::Override,
                initial_content: "# draft\n".to_string(),
                restore_content: app.file_view.content.clone(),
                restore_path: app.file_view.path.clone(),
            },
            "[Service]\nEnvironment=DEBUG=1\n".to_string(),
        );

        assert!(app.pending_edit_review.is_some());
        assert_eq!(app.file_view.path, "Draft Override: ssh.service");

        app.discard_edit_review();

        assert!(app.pending_edit_review.is_none());
        assert_eq!(app.file_view.path, "/usr/lib/systemd/system/ssh.service");
        assert_eq!(app.file_view.content, "[Service]\nExecStart=/usr/bin/ssh\n");
    }

    #[test]
    fn select_index_tracks_selected_unit_key() {
        let mut app = test_app(vec![
            unit(
                "alpha.service",
                "Alpha",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/alpha",
            ),
            unit(
                "beta.service",
                "Beta",
                UnitLoadState::Loaded,
                UnitActiveState::Active,
                UnitEnablementState::Enabled,
                "/test/unit/beta",
            ),
        ]);

        app.unit_list.filtered_indices = vec![0, 1];
        app.unit_list.select_index(Some(1));

        assert_eq!(app.unit_list.selected_key.name, "beta.service");
        assert_eq!(app.unit_list.selected_key.scope, UnitScope::Global);
        assert_eq!(app.unit_list.selected_key.path, "/test/unit/beta");
        assert_eq!(
            app.get_selected_unit().map(|unit| unit.name.as_str()),
            Some("beta.service")
        );
    }
}
