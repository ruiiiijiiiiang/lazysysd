use std::{
    collections::{BTreeSet, HashSet, VecDeque},
    io::{Read, Result, Write},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    terminal,
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
        auth::{EmbeddedAuthFlow, EmbeddedAuthPane},
        dbus::{fetch_all_units, get_unit_fragment_path, perform_unit_action},
        edit::perform_unit_edit,
        journal::JournalManager,
    },
};

const LOG_CAPACITY: usize = 10;
const AUTH_START_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilterMenuOption {
    pub hotkey: char,
    pub label: String,
    pub value: Option<String>,
    pub selected: bool,
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

    pub logs: VecDeque<String>,
    pub embedded_auth: Option<EmbeddedAuthFlow>,
    pub active_privileged_action: Option<PrivilegedAction>,
    pub internal_tx: mpsc::Sender<AppInternalEvent>,

    pub matcher: SkimMatcherV2,
    pub is_loading: bool,

    pub visual_select: bool,
    pub selected_log_lines: HashSet<usize>,
    pub pending_nav_prefix: Option<char>,
}

impl FilterMenu {
    pub fn title(self) -> &'static str {
        match self {
            Self::Active => "Active State",
            Self::Enablement => "Enablement State",
            Self::Load => "Load State",
        }
    }

    pub fn segment_title(self) -> &'static str {
        match self {
            Self::Active => " Active (a) ",
            Self::Enablement => " Enablement (n) ",
            Self::Load => " Load (o) ",
        }
    }

    fn unit_value(self, unit: &UnitInfo) -> &str {
        match self {
            Self::Active => &unit.active_state,
            Self::Enablement => &unit.enablement_state,
            Self::Load => &unit.load_state,
        }
    }

    fn selected_value(self, app: &App) -> Option<&str> {
        match self {
            Self::Active => app.active_state_filter.as_deref(),
            Self::Enablement => app.enablement_state_filter.as_deref(),
            Self::Load => app.load_state_filter.as_deref(),
        }
    }

    fn set_selected_value(self, app: &mut App, value: Option<String>) {
        match self {
            Self::Active => app.active_state_filter = value,
            Self::Enablement => app.enablement_state_filter = value,
            Self::Load => app.load_state_filter = value,
        }
    }

    fn preferred_order(self) -> &'static [&'static str] {
        match self {
            // Based on the documented high-level systemd unit states.
            Self::Active => &[
                "active",
                "inactive",
                "failed",
                "activating",
                "deactivating",
                "maintenance",
                "reloading",
            ],
            Self::Enablement => &[
                "enabled",
                "enabled-runtime",
                "linked",
                "linked-runtime",
                "masked",
                "masked-runtime",
                "static",
                "disabled",
                "invalid",
                "indirect",
                "alias",
                "generated",
                "transient",
                "unknown",
            ],
            // Includes the documented systemctl LOAD values; any observed extras are appended.
            Self::Load => &["loaded", "not-found", "bad-setting", "error", "masked"],
        }
    }

    fn preferred_hotkeys(self, value: &str) -> Vec<char> {
        match self {
            Self::Active => match value {
                "active" => vec!['t', 'v', 'c'],
                "inactive" => vec!['i'],
                "failed" => vec!['f'],
                "activating" => vec!['g'],
                "deactivating" => vec!['d'],
                "maintenance" => vec!['m'],
                "reloading" => vec!['r'],
                "unknown" => vec!['u'],
                _ => Vec::new(),
            },
            Self::Enablement => match value {
                "enabled" => vec!['e'],
                "disabled" => vec!['d'],
                "static" => vec!['s'],
                "masked" => vec!['m'],
                "indirect" => vec!['i'],
                "alias" => vec!['l'],
                "generated" => vec!['g'],
                "linked" => vec!['k'],
                "enabled-runtime" => vec!['r'],
                "disabled-runtime" => vec!['u'],
                "masked-runtime" => vec!['x'],
                "linked-runtime" => vec!['y'],
                "transient" => vec!['t'],
                "unknown" => vec!['w'],
                _ => Vec::new(),
            },
            Self::Load => match value {
                "loaded" => vec!['l'],
                "not-found" => vec!['n'],
                "masked" => vec!['m'],
                "error" => vec!['e'],
                "bad-setting" => vec!['b'],
                "merged" => vec!['g'],
                "stub" => vec!['s'],
                "unknown" => vec!['u'],
                _ => Vec::new(),
            },
        }
    }
}

impl App {
    fn blank(internal_tx: mpsc::Sender<AppInternalEvent>) -> Self {
        Self {
            units: Vec::new(),
            filtered_units: Vec::new(),
            list_state: ListState::default(),
            search_query: String::new(),
            is_searching: false,
            active_state_filter: None,
            enablement_state_filter: None,
            load_state_filter: None,
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
            logs: VecDeque::new(),
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
        app.push_log("lazysysd started");
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

    pub async fn fetch_unit_logs(&mut self, unit_name: String) {
        self.is_loading = true;
        let tx = self.internal_tx.clone();
        tokio::spawn(async move {
            let manager = JournalManager::new();
            match manager.fetch_logs(&unit_name, 100).await {
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
            match get_unit_fragment_path(&unit.path).await {
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

    pub fn update_filter(&mut self) {
        let selected_unit_name = self.get_selected_unit().map(|unit| unit.name.clone());

        if self.search_query.is_empty() {
            self.filtered_units = self
                .units
                .iter()
                .enumerate()
                .filter(|(_, unit)| self.unit_matches_state_filters(unit))
                .map(|(index, _)| index)
                .collect();
        } else {
            let mut scored: Vec<(usize, i64)> = self
                .units
                .iter()
                .enumerate()
                .filter(|(_, unit)| self.unit_matches_state_filters(unit))
                .filter_map(|(index, unit)| self.search_score(unit).map(|score| (index, score)))
                .collect();
            scored.sort_by_key(|&(_, score)| -score);
            self.filtered_units = scored.into_iter().map(|(index, _)| index).collect();
        }

        self.restore_selection(selected_unit_name.as_deref());
    }

    pub fn filter_summary(&self, menu: FilterMenu) -> &str {
        menu.selected_value(self).unwrap_or("all")
    }

    pub fn filter_menu_options(&self, menu: FilterMenu) -> Vec<FilterMenuOption> {
        let values = self.comprehensive_filter_values(menu);

        let mut options = vec![FilterMenuOption {
            hotkey: 'a',
            label: "all".to_string(),
            value: None,
            selected: menu.selected_value(self).is_none(),
        }];
        let mut used_hotkeys = HashSet::from(['a']);

        for label in self.sort_filter_values(menu, values) {
            let hotkey = self.assign_filter_hotkey(menu, &label, &mut used_hotkeys);
            options.push(FilterMenuOption {
                hotkey,
                selected: menu.selected_value(self) == Some(label.as_str()),
                value: Some(label.clone()),
                label,
            });
        }

        options
    }

    pub fn finish_edit_request(&mut self, request: EditRequest, edited_content: String) {
        if edited_content == request.initial_content {
            self.push_log(format!(
                "{} edit cancelled for {}",
                request.mode.action_label(),
                request.unit_name
            ));
            return;
        }

        self.unit_file_content = edited_content.clone();
        self.unit_file_path = request.mode.draft_label(&request.unit_name);
        self.file_scroll = 0;
        self.pending_edit_review = Some(EditReview {
            unit_name: request.unit_name.clone(),
            mode: request.mode,
            edited_content,
            restore_content: request.restore_content,
            restore_path: request.restore_path,
        });
        self.push_log(format!(
            "{} draft ready for {}",
            request.mode.action_label(),
            request.unit_name
        ));
    }

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
            if self.visual_select {
                if self.handle_nav_key(key) {
                    return Ok(false);
                }
                match key.code {
                    KeyCode::Esc => {
                        self.visual_select = false;
                        self.selected_log_lines.clear();
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
                            let msg =
                                tokio::task::spawn_blocking(move || copy_to_clipboard(&cloned))
                                    .await
                                    .unwrap_or_else(|e| format!("Clipboard task panic: {e}"));
                            self.push_log(msg);
                        }
                        self.visual_select = false;
                        self.selected_log_lines.clear();
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
                    self.visual_select = false;
                    self.selected_log_lines.clear();
                }
                KeyCode::Char('v') if !self.unit_logs.is_empty() => {
                    self.visual_select = true;
                    if self.log_state.selected().is_none() {
                        self.log_state.select(Some(0));
                    }
                }
                KeyCode::Char('v') => {}
                KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(unit) = self.get_selected_unit() {
                        let name = unit.name.clone();
                        self.fetch_unit_logs(name).await;
                    }
                }
                _ => {}
            }
            return Ok(false);
        }

        if self.view_mode == ViewMode::FileView {
            if self.handle_nav_key(key) {
                return Ok(false);
            }
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view_mode = ViewMode::UnitList;
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
                _ => {}
            }
            return Ok(false);
        }

        if self.is_searching {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.is_searching = false;
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.update_filter();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
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
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.refresh_units().await;
                Ok(false)
            }
            KeyCode::Char('l') | KeyCode::Enter => {
                if let Some(unit) = self.get_selected_unit() {
                    let name = unit.name.clone();
                    self.view_mode = ViewMode::LogView;
                    self.unit_logs.clear();
                    self.log_state.select(None);
                    self.visual_select = false;
                    self.selected_log_lines.clear();
                    self.fetch_unit_logs(name).await;
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

    fn move_selection(&mut self, delta: i32) {
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

    fn move_log_selection(&mut self, delta: i32) {
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

    fn get_selected_unit(&self) -> Option<&UnitInfo> {
        self.list_state
            .selected()
            .map(|i| &self.units[self.filtered_units[i]])
    }

    fn build_edit_request(&self, mode: UnitEditMode) -> Option<EditRequest> {
        let unit = self.get_selected_unit()?;
        let initial_content = match mode {
            UnitEditMode::Override => build_override_template(&unit.name, &self.unit_file_path),
            UnitEditMode::Full => self.unit_file_content.clone(),
        };

        Some(EditRequest {
            unit_name: unit.name.clone(),
            mode,
            initial_content,
            restore_content: self.unit_file_content.clone(),
            restore_path: self.unit_file_path.clone(),
        })
    }

    async fn trigger_selected_unit_command(&mut self, action: &str) -> Result<()> {
        if let Some(unit) = self.get_selected_unit() {
            let (cols, rows) = terminal::size().unwrap_or((80, 24));
            self.start_embedded_auth(
                PrivilegedAction::UnitCommand {
                    unit_name: unit.name.clone(),
                    action: action.to_string(),
                },
                cols,
                rows,
            )
            .await?;
        }
        Ok(())
    }

    fn restore_selection(&mut self, selected_unit_name: Option<&str>) {
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

    async fn handle_edit_review_key(&mut self, key: KeyEvent) -> Result<bool> {
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

    fn discard_edit_review(&mut self) {
        if let Some(review) = self.pending_edit_review.take() {
            self.unit_file_content = review.restore_content;
            self.unit_file_path = review.restore_path;
            self.file_scroll = 0;
            self.push_log(format!(
                "{} draft discarded for {}",
                review.mode.action_label(),
                review.unit_name
            ));
        }
    }

    fn search_score(&self, unit: &UnitInfo) -> Option<i64> {
        let target = format!("{} {}", unit.name, unit.description);
        self.matcher.fuzzy_match(&target, &self.search_query)
    }

    fn unit_matches_search(&self, unit: &UnitInfo) -> bool {
        self.search_query.is_empty() || self.search_score(unit).is_some()
    }

    fn unit_matches_state_filters(&self, unit: &UnitInfo) -> bool {
        Self::matches_filter_value(self.active_state_filter.as_deref(), &unit.active_state)
            && Self::matches_filter_value(
                self.enablement_state_filter.as_deref(),
                &unit.enablement_state,
            )
            && Self::matches_filter_value(self.load_state_filter.as_deref(), &unit.load_state)
    }

    fn unit_matches_scope_for_menu(&self, unit: &UnitInfo, menu: FilterMenu) -> bool {
        self.unit_matches_search(unit)
            && (menu == FilterMenu::Active
                || Self::matches_filter_value(
                    self.active_state_filter.as_deref(),
                    &unit.active_state,
                ))
            && (menu == FilterMenu::Enablement
                || Self::matches_filter_value(
                    self.enablement_state_filter.as_deref(),
                    &unit.enablement_state,
                ))
            && (menu == FilterMenu::Load
                || Self::matches_filter_value(self.load_state_filter.as_deref(), &unit.load_state))
    }

    fn available_filter_values(&self, menu: FilterMenu, scoped: bool) -> BTreeSet<String> {
        self.units
            .iter()
            .filter(|unit| !scoped || self.unit_matches_scope_for_menu(unit, menu))
            .map(|unit| menu.unit_value(unit).to_string())
            .filter(|value| !value.is_empty())
            .collect()
    }

    fn comprehensive_filter_values(&self, menu: FilterMenu) -> BTreeSet<String> {
        let mut values = self.available_filter_values(menu, false);
        values.extend(
            menu.preferred_order()
                .iter()
                .map(|value| (*value).to_string()),
        );
        values
    }

    fn sort_filter_values(&self, menu: FilterMenu, values: BTreeSet<String>) -> Vec<String> {
        let mut remaining: Vec<String> = values.into_iter().collect();
        let mut ordered = Vec::with_capacity(remaining.len());

        for preferred in menu.preferred_order() {
            if let Some(index) = remaining.iter().position(|value| value == preferred) {
                ordered.push(remaining.remove(index));
            }
        }

        ordered.extend(remaining);
        ordered
    }

    fn assign_filter_hotkey(
        &self,
        menu: FilterMenu,
        label: &str,
        used_hotkeys: &mut HashSet<char>,
    ) -> char {
        let fallbacks = label
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase());

        for candidate in menu
            .preferred_hotkeys(label)
            .into_iter()
            .chain(fallbacks)
            .chain('0'..='9')
        {
            let normalized = candidate.to_ascii_lowercase();
            if used_hotkeys.insert(normalized) {
                return normalized;
            }
        }

        '?'
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

        fn perform_nav(&mut self, action: NavAction) {
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
                NavAction::Top => self.list_state.select(Some(0)),
                NavAction::Bottom => {
                    if !self.filtered_units.is_empty() {
                        self.list_state
                            .select(Some(self.filtered_units.len().saturating_sub(1)));
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
                self.file_scroll = self.file_scroll.min(total_lines.saturating_sub(1).max(0) as u16);
            }
        }
        }

        pub fn matches_filter_value(selected: Option<&str>, actual: &str) -> bool {
        match selected {
            Some(expected) => expected == actual,
            None => true,
        }
    }

    fn handle_filter_menu_key(&mut self, key: KeyEvent) {
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

    async fn start_embedded_auth(
        &mut self,
        action: PrivilegedAction,
        cols: u16,
        rows: u16,
    ) -> Result<()> {
        if self.embedded_auth.is_some() {
            return Ok(());
        }

        let pane = EmbeddedAuthPane::spawn(cols, rows, self.internal_tx.clone())?;
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel_flag);
        let tx_clone = self.internal_tx.clone();
        let worker_action = action.clone();

        tokio::spawn(async move {
            tokio::time::sleep(AUTH_START_DELAY).await;
            if cancel_clone.load(Ordering::SeqCst) {
                return;
            }

            let result = match worker_action {
                PrivilegedAction::UnitCommand { unit_name, action } => {
                    perform_unit_action(&unit_name, &action).await
                }
                PrivilegedAction::ApplyEdit {
                    unit_name,
                    mode,
                    content,
                } => perform_unit_edit(&unit_name, mode, content).await,
            };
            let _ = tx_clone.send(AppInternalEvent::AuthResult(result)).await;
        });

        self.push_log(privileged_action_log(&action));
        self.active_privileged_action = Some(action);
        self.embedded_auth = Some(EmbeddedAuthFlow { pane, cancel_flag });
        Ok(())
    }

    pub fn cancel_embedded_auth(&mut self, reason: &str) {
        self.active_privileged_action = None;
        if let Some(mut flow) = self.embedded_auth.take() {
            flow.cancel_flag.store(true, Ordering::SeqCst);
            tokio::task::spawn_blocking(move || {
                flow.pane.stop();
            });
            self.push_log(reason.to_string());
        }
    }

    pub fn resize_embedded_auth(&mut self, cols: u16, rows: u16) -> Result<()> {
        if let Some(flow) = self.embedded_auth.as_mut() {
            flow.pane.resize(cols, rows)?;
        }
        Ok(())
    }

    pub async fn handle_internal_event(&mut self, event: AppInternalEvent) {
        match event {
            AppInternalEvent::UnitsLoaded(units) => {
                self.units = units;
                self.is_loading = false;
                self.update_filter();
                self.push_log("units loaded");
            }
            AppInternalEvent::LogsLoaded(logs) => {
                self.unit_logs = logs;
                self.is_loading = false;
                self.log_state
                    .select(Some(self.unit_logs.len().saturating_sub(1)));
                self.push_log("logs loaded");
            }
            AppInternalEvent::FileLoaded(content, path) => {
                self.unit_file_content = content;
                self.unit_file_path = path;
                self.is_loading = false;
                self.push_log("unit file loaded");
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
                self.push_log(result.log_entry);
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
            AppInternalEvent::Error(err) => {
                self.is_loading = false;
                self.push_log(err);
            }
        }
    }

    fn push_log(&mut self, entry: impl Into<String>) {
        if self.logs.len() >= LOG_CAPACITY {
            self.logs.pop_front();
        }
        self.logs.push_back(entry.into());
    }
}

fn unit_command_for_key(key: KeyEvent) -> Option<&'static str> {
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

fn privileged_action_log(action: &PrivilegedAction) -> String {
    match action {
        PrivilegedAction::UnitCommand { unit_name, action } => {
            format!("auth required for {} on {}", action, unit_name)
        }
        PrivilegedAction::ApplyEdit {
            unit_name, mode, ..
        } => {
            format!(
                "auth required to apply {} for {}",
                mode.action_label(),
                unit_name
            )
        }
    }
}

fn copy_to_clipboard(text: &str) -> String {
    let candidates: [(&str, &[&str]); 3] = [
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("pbcopy", &[]),
    ];
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
    use super::{App, FilterMenu, unit_command_for_key};
    use crate::models::{EditRequest, UnitEditMode, UnitInfo};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
            load_state: load_state.to_string(),
            active_state: active_state.to_string(),
            enablement_state: enablement_state.to_string(),
            sub_state: active_state.to_string(),
            path: OwnedObjectPath::try_from(path).unwrap(),
        }
    }

    fn filtered_names(app: &App) -> Vec<&str> {
        app.filtered_units
            .iter()
            .map(|&index| app.units[index].name.as_str())
            .collect()
    }

    #[test]
    fn update_filter_combines_search_and_state_filters() {
        let mut app = test_app(vec![
            unit(
                "ssh.service",
                "Secure Shell",
                "loaded",
                "active",
                "enabled",
                "/test/unit/ssh",
            ),
            unit(
                "broken.service",
                "Broken worker",
                "loaded",
                "failed",
                "static",
                "/test/unit/broken",
            ),
            unit(
                "db.service",
                "Database",
                "loaded",
                "failed",
                "disabled",
                "/test/unit/db",
            ),
        ]);

        app.active_state_filter = Some("failed".to_string());
        app.enablement_state_filter = Some("static".to_string());
        app.search_query = "broken".to_string();
        app.update_filter();

        assert_eq!(filtered_names(&app), vec!["broken.service"]);
    }

    #[test]
    fn filter_menu_options_include_all_and_expected_hotkeys() {
        let app = test_app(vec![
            unit(
                "ssh.service",
                "Secure Shell",
                "loaded",
                "active",
                "enabled",
                "/test/unit/ssh",
            ),
            unit(
                "broken.service",
                "Broken worker",
                "masked",
                "inactive",
                "static",
                "/test/unit/broken",
            ),
        ]);

        let options = app.filter_menu_options(FilterMenu::Active);

        assert_eq!(options[0].label, "all");
        assert!(options[0].selected);
        assert!(
            options
                .iter()
                .any(|option| option.label == "inactive" && option.hotkey == 'i')
        );
        assert!(options.iter().any(|option| option.label == "active"));
    }

    #[test]
    fn active_and_load_filters_include_documented_states_even_when_absent() {
        let app = test_app(vec![unit(
            "ssh.service",
            "Secure Shell",
            "loaded",
            "active",
            "enabled",
            "/test/unit/ssh",
        )]);

        let active_options = app.filter_menu_options(FilterMenu::Active);
        let load_options = app.filter_menu_options(FilterMenu::Load);

        assert!(active_options.iter().any(|option| option.label == "failed"));
        assert!(
            active_options
                .iter()
                .any(|option| option.label == "reloading")
        );
        assert!(
            load_options
                .iter()
                .any(|option| option.label == "not-found")
        );
        assert!(
            load_options
                .iter()
                .any(|option| option.label == "bad-setting")
        );
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
}
