use std::{
    collections::HashSet,
    io::{Read, Result, Write},
    process::{Command, Stdio},
    sync::Arc,
};

use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use ratatui::widgets::ListState;
use tokio::sync::mpsc;

use crate::{
    models::{
        AppInternalEvent, EditRequest, EditReview, PendingAction, PrivilegedAction, UnitEditMode,
        UnitInfo,
    },
    systemd::{
        auth::EmbeddedAuthFlow,
        dbus::{fetch_all_units, get_unit_fragment_path},
        journal::JournalManager,
    },
};

pub const AUTH_START_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    UnitList,
    LogView,
    FileView,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NavAction {
    Up,
    Down,
    PageUp,
    PageDown,
    HalfPageUp,
    HalfPageDown,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilterMenu {
    Active,
    Enablement,
    Load,
    Scope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterMenuOption {
    pub hotkey: char,
    pub label: String,
    pub value: Option<String>,
    pub selected: bool,
    pub count: usize,
}

pub struct App {
    pub units: Vec<UnitInfo>,
    pub filtered_units: Vec<usize>,
    pub list_state: ListState,
    pub search_query: String,
    pub is_searching: bool,
    pub active_state_filter: Option<String>,
    pub enablement_state_filter: Option<String>,
    pub load_state_filter: Option<String>,
    pub scope_filter: Option<String>,
    pub open_filter_menu: Option<FilterMenu>,
    pub view_mode: ViewMode,
    pub unit_logs: Vec<String>,
    pub log_state: ListState,
    pub last_area_height: u16,
    pub unit_file_content: String,
    pub unit_file_path: String,
    pub file_scroll: u16,
    pub pending_action: Option<PendingAction>,
    pub pending_edit_review: Option<EditReview>,

    pub embedded_auth: Option<EmbeddedAuthFlow>,
    pub active_privileged_action: Option<PrivilegedAction>,
    pub internal_tx: mpsc::Sender<AppInternalEvent>,

    pub matcher: SkimMatcherV2,
    pub is_loading: bool,

    pub visual_select: bool,
    pub selected_log_lines: HashSet<usize>,
    pub pending_nav_prefix: Option<char>,
}

impl App {
    pub fn blank(internal_tx: mpsc::Sender<AppInternalEvent>) -> Self {
        Self {
            units: Vec::new(),
            filtered_units: Vec::new(),
            list_state: ListState::default(),
            search_query: String::new(),
            is_searching: false,
            active_state_filter: None,
            enablement_state_filter: None,
            load_state_filter: None,
            scope_filter: None,
            open_filter_menu: None,
            view_mode: ViewMode::UnitList,
            unit_logs: Vec::new(),
            log_state: ListState::default(),
            last_area_height: 0,
            unit_file_content: String::new(),
            unit_file_path: String::new(),
            file_scroll: 0,
            pending_action: None,
            pending_edit_review: None,
            embedded_auth: None,
            active_privileged_action: None,
            internal_tx,
            matcher: SkimMatcherV2::default(),
            is_loading: true,
            visual_select: false,
            selected_log_lines: HashSet::new(),
            pending_nav_prefix: None,
        }
    }

    pub async fn new(internal_tx: mpsc::Sender<AppInternalEvent>) -> Self {
        let mut app = Self::blank(internal_tx);
        app.refresh_units().await;
        app
    }

    pub async fn refresh_units(&mut self) {
        self.is_loading = true;
        let tx = self.internal_tx.clone();
        tokio::spawn(async move {
            match fetch_all_units().await {
                Ok(units) => {
                    let _ = tx.send(AppInternalEvent::UnitsLoaded(units)).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppInternalEvent::Error(format!(
                            "Failed to load units: {e}"
                        )))
                        .await;
                }
            }
        });
    }

    pub async fn fetch_unit_logs(&mut self, unit_name: String, scope: String) {
        self.is_loading = true;
        let tx = self.internal_tx.clone();
        tokio::spawn(async move {
            let manager = JournalManager::new();
            match manager.fetch_logs(&unit_name, &scope, 100).await {
                Ok(logs) => {
                    let _ = tx.send(AppInternalEvent::LogsLoaded(logs)).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(AppInternalEvent::Error(format!("Failed to load logs: {e}")))
                        .await;
                }
            }
        });
    }

    pub async fn fetch_unit_file(&mut self, unit: UnitInfo) {
        self.is_loading = true;
        let tx = self.internal_tx.clone();
        tokio::spawn(async move {
            match get_unit_fragment_path(&unit.path, &unit.scope).await {
                Ok(path) => {
                    if path.is_empty() || path == "/dev/null" {
                        let _ = tx
                            .send(AppInternalEvent::Error(
                                "Unit file not found (masked or transient)".to_string(),
                            ))
                            .await;
                        return;
                    }
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            let _ = tx.send(AppInternalEvent::FileLoaded(content, path)).await;
                        }
                        Err(e) => {
                            let _ = tx
                                .send(AppInternalEvent::Error(format!(
                                    "Failed to read unit file: {e}"
                                )))
                                .await;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(AppInternalEvent::Error(format!(
                            "Failed to get unit path: {e}"
                        )))
                        .await;
                }
            }
        });
    }

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
        self.list_state.select(Some(i));
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
        self.list_state
            .selected()
            .map(|i| &self.units[self.filtered_units[i]])
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
            let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
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

    pub fn restore_selection(&mut self, selected_unit_name: Option<&str>) {
        if let Some(unit_name) = selected_unit_name
            && let Some(index) = self
                .filtered_units
                .iter()
                .position(|&unit_index| self.units[unit_index].name == unit_name)
        {
            self.list_state.select(Some(index));
            return;
        }

        if self.list_state.selected().is_none() && !self.filtered_units.is_empty() {
            self.list_state.select(Some(0));
        } else if self.filtered_units.is_empty() {
            self.list_state.select(None);
        } else if let Some(selected) = self.list_state.selected()
            && selected >= self.filtered_units.len()
        {
            self.list_state
                .select(Some(self.filtered_units.len().saturating_sub(1)));
        }
    }

    pub fn discard_edit_review(&mut self) {
        if let Some(review) = self.pending_edit_review.take() {
            self.unit_file_content = review.restore_content;
            self.unit_file_path = review.restore_path;
            self.file_scroll = 0;
        }
    }

    pub fn search_score(&self, unit: &UnitInfo) -> Option<i64> {
        let target = format!("{} {}", unit.name, unit.description);
        self.matcher.fuzzy_match(&target, &self.search_query)
    }

    pub fn unit_matches_search(&self, unit: &UnitInfo) -> bool {
        self.search_query.is_empty() || self.search_score(unit).is_some()
    }

    pub fn matches_filter_value(selected: Option<&str>, actual: &str) -> bool {
        match selected {
            Some(expected) => expected == actual,
            None => true,
        }
    }

    pub async fn start_embedded_auth(
        &mut self,
        action: PrivilegedAction,
        cols: u16,
        rows: u16,
    ) -> Result<()> {
        if self.embedded_auth.is_some() {
            return Ok(());
        }

        let pane =
            crate::systemd::auth::EmbeddedAuthPane::spawn(cols, rows, self.internal_tx.clone())?;
        let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel_flag);
        let tx_clone = self.internal_tx.clone();
        let worker_action = action.clone();

        tokio::spawn(async move {
            tokio::time::sleep(AUTH_START_DELAY).await;
            if cancel_clone.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }

            let result = match worker_action {
                PrivilegedAction::UnitCommand {
                    unit_name,
                    scope,
                    action,
                } => crate::systemd::dbus::perform_unit_action(&unit_name, &scope, &action).await,
                PrivilegedAction::ApplyEdit {
                    unit_name,
                    scope,
                    mode,
                    content,
                } => {
                    crate::systemd::edit::perform_unit_edit(&unit_name, &scope, mode, content).await
                }
            };
            let _ = tx_clone.send(AppInternalEvent::AuthResult(result)).await;
        });

        self.active_privileged_action = Some(action);
        self.embedded_auth = Some(crate::systemd::auth::EmbeddedAuthFlow { pane, cancel_flag });
        Ok(())
    }

    pub fn cancel_embedded_auth(&mut self, _reason: &str) {
        self.active_privileged_action = None;
        if let Some(mut flow) = self.embedded_auth.take() {
            flow.cancel_flag
                .store(true, std::sync::atomic::Ordering::SeqCst);
            flow.pane.stop();
        }
    }

    pub fn resize_embedded_auth(&mut self, cols: u16, rows: u16) -> Result<()> {
        if let Some(flow) = self.embedded_auth.as_mut() {
            flow.pane.resize(cols, rows)?;
        }
        Ok(())
    }
}

fn build_override_template(unit_name: &str, source_path: &str) -> String {
    format!(
        "# Drop-in override for {unit_name}\n\
         # Add only the sections and keys you want to override.\n\
         # Source fragment: {source_path}\n\
         # Example:\n\
         # [Service]\n\
         # Environment=KEY=value\n"
    )
}

pub fn copy_to_clipboard(text: &str) -> String {
    let candidates: [(&str, &[&str]); 3] =
        [("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])];
    for (cmd, args) in candidates {
        let mut child = match Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => continue,
        };

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }

        match child.wait() {
            Ok(status) if status.success() => {
                return format!("Copied {} chars to clipboard via {}", text.len(), cmd);
            }
            Ok(_) => {
                let err = child
                    .stderr
                    .take()
                    .and_then(|mut s| {
                        let mut buf = String::new();
                        s.read_to_string(&mut buf).ok().map(|_| buf)
                    })
                    .unwrap_or_default();
                if !err.is_empty() {
                    return format!("Clipboard failed: {} (stderr: {})", cmd, err.trim());
                }
            }
            Err(e) => {
                return format!("Clipboard failed: {} (wait error: {})", cmd, e);
            }
        }
    }
    "Clipboard failed: no clipboard tool found".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{EditRequest, UnitEditMode, UnitInfo};
    use tokio::sync::mpsc;
    use zbus::zvariant::OwnedObjectPath;

    fn test_app(units: Vec<UnitInfo>) -> App {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.units = units;
        app.is_loading = false;
        app.update_filter();
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
    fn finish_edit_request_creates_review_and_discard_restores_preview() {
        let mut app = test_app(vec![unit(
            "ssh.service",
            "Secure Shell",
            "loaded",
            "active",
            "enabled",
            "/test/unit/ssh",
        )]);
        app.unit_file_content = "[Service]\nExecStart=/usr/bin/ssh\n".to_string();
        app.unit_file_path = "/usr/lib/systemd/system/ssh.service".to_string();

        app.finish_edit_request(
            EditRequest {
                unit_name: "ssh.service".to_string(),
                scope: "global".to_string(),
                mode: UnitEditMode::Override,
                initial_content: "# draft\n".to_string(),
                restore_content: app.unit_file_content.clone(),
                restore_path: app.unit_file_path.clone(),
            },
            "[Service]\nEnvironment=DEBUG=1\n".to_string(),
        );

        assert!(app.pending_edit_review.is_some());
        assert_eq!(app.unit_file_path, "Draft Override: ssh.service");

        app.discard_edit_review();

        assert!(app.pending_edit_review.is_none());
        assert_eq!(app.unit_file_path, "/usr/lib/systemd/system/ssh.service");
        assert_eq!(app.unit_file_content, "[Service]\nExecStart=/usr/bin/ssh\n");
    }
}
