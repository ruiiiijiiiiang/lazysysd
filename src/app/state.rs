use std::{
    collections::{HashSet, VecDeque},
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
    models::{AppInternalEvent, PendingAction, UnitInfo},
    systemd::{
        auth::{EmbeddedAuthFlow, EmbeddedAuthPane},
        dbus::{fetch_all_units, get_unit_fragment_path, perform_unit_action},
        journal::JournalManager,
    },
};

const LOG_CAPACITY: usize = 10;
const AUTH_START_DELAY: std::time::Duration = std::time::Duration::from_millis(150);

#[derive(PartialEq)]
pub enum ViewMode {
    UnitList,
    LogView,
    FileView,
}

pub struct App {
    pub units: Vec<UnitInfo>,
    pub filtered_units: Vec<usize>,
    pub list_state: ListState,
    pub search_query: String,
    pub is_searching: bool,
    pub view_mode: ViewMode,
    pub unit_logs: Vec<String>,
    pub log_state: ListState,
    pub last_area_height: u16,
    pub unit_file_content: String,
    pub unit_file_path: String,
    pub file_scroll: u16,
    pub pending_action: Option<PendingAction>,

    pub logs: VecDeque<String>,
    pub embedded_auth: Option<EmbeddedAuthFlow>,
    pub internal_tx: mpsc::Sender<AppInternalEvent>,

    pub matcher: SkimMatcherV2,
    pub is_loading: bool,

    pub visual_select: bool,
    pub selected_log_lines: HashSet<usize>,
}

impl App {
    pub async fn new(internal_tx: mpsc::Sender<AppInternalEvent>) -> Self {
        let mut app = Self {
            units: Vec::new(),
            filtered_units: Vec::new(),
            list_state: ListState::default(),
            search_query: String::new(),
            is_searching: false,
            view_mode: ViewMode::UnitList,
            unit_logs: Vec::new(),
            log_state: ListState::default(),
            last_area_height: 0,
            unit_file_content: String::new(),
            unit_file_path: String::new(),
            file_scroll: 0,
            pending_action: None,
            logs: VecDeque::new(),
            embedded_auth: None,
            internal_tx,
            matcher: SkimMatcherV2::default(),
            is_loading: true,
            visual_select: false,
            selected_log_lines: HashSet::new(),
        };

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
        if self.search_query.is_empty() {
            self.filtered_units = (0..self.units.len()).collect();
        } else {
            let mut scored: Vec<(usize, i64)> = self
                .units
                .iter()
                .enumerate()
                .filter_map(|(i, unit)| {
                    let target = format!("{} {}", unit.name, unit.description);
                    self.matcher
                        .fuzzy_match(&target, &self.search_query)
                        .map(|score| (i, score))
                })
                .collect();
            scored.sort_by_key(|&(_, score)| -score);
            self.filtered_units = scored.into_iter().map(|(i, _)| i).collect();
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

    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Modal priority
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

        if self.view_mode == ViewMode::LogView {
            if self.visual_select {
                match key.code {
                    KeyCode::Esc => {
                        self.visual_select = false;
                        self.selected_log_lines.clear();
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.move_log_selection(1);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.move_log_selection(-1);
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
                            let msg = tokio::task::spawn_blocking(move || copy_to_clipboard(&cloned))
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

            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view_mode = ViewMode::UnitList;
                    self.visual_select = false;
                    self.selected_log_lines.clear();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.move_log_selection(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.move_log_selection(-1);
                }
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_log_selection(self.last_area_height as i32 / 2);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_log_selection(-(self.last_area_height as i32 / 2));
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_log_selection(self.last_area_height as i32);
                }
                KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_log_selection(-(self.last_area_height as i32));
                }
                KeyCode::Char('v') if !self.unit_logs.is_empty() => {
                    self.visual_select = true;
                    if self.log_state.selected().is_none() {
                        self.log_state.select(Some(0));
                    }
                }
                KeyCode::Char('v') => {}
                KeyCode::Char('r') => {
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
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.view_mode = ViewMode::UnitList;
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.file_scroll = self.file_scroll.saturating_add(1);
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.file_scroll = self.file_scroll.saturating_sub(1);
                }
                KeyCode::Char('e') if !self.unit_file_path.is_empty() => {
                    self.pending_action =
                        Some(PendingAction::EditFile(self.unit_file_path.clone()));
                    return Ok(true);
                }
                _ => {}
            }
            return Ok(false);
        }

        // Search mode
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

        // Normal mode (Vim style)
        match key.code {
            KeyCode::Char('q') => Ok(true),
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_selection(1);
                Ok(false)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_selection(-1);
                Ok(false)
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_selection(self.last_area_height as i32 / 2);
                Ok(false)
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_selection(-(self.last_area_height as i32 / 2));
                Ok(false)
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_selection(self.last_area_height as i32);
                Ok(false)
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.move_selection(-(self.last_area_height as i32));
                Ok(false)
            }
            KeyCode::Char('/') => {
                self.is_searching = true;
                Ok(false)
            }
            KeyCode::Char('r') => {
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
            KeyCode::Char('a') => {
                // Restart action
                if let Some(unit) = self.get_selected_unit() {
                    let name = unit.name.clone();
                    let (cols, rows) = terminal::size().unwrap_or((80, 24));
                    self.start_embedded_auth(&name, "restart", cols, rows)
                        .await?;
                }
                Ok(false)
            }
            _ => Ok(false),
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

    async fn start_embedded_auth(
        &mut self,
        unit_name: &str,
        action: &str,
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
        let u_name = unit_name.to_string();
        let a_name = action.to_string();

        let u_name_worker = u_name.clone();
        let a_name_worker = a_name.clone();

        tokio::spawn(async move {
            tokio::time::sleep(AUTH_START_DELAY).await;
            if cancel_clone.load(Ordering::SeqCst) {
                return;
            }

            let result = perform_unit_action(&u_name_worker, &a_name_worker).await;
            let _ = tx_clone.send(AppInternalEvent::AuthResult(result)).await;
        });

        self.push_log(format!("auth required for {} on {}", a_name, u_name));
        self.embedded_auth = Some(EmbeddedAuthFlow { pane, cancel_flag });
        Ok(())
    }

    pub fn cancel_embedded_auth(&mut self, reason: &str) {
        if let Some(mut flow) = self.embedded_auth.take() {
            flow.cancel_flag.store(true, Ordering::SeqCst);
            flow.pane.stop();
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
            AppInternalEvent::AuthResult(result) => {
                if let Some(mut flow) = self.embedded_auth.take() {
                    flow.pane.stop();
                    self.push_log(result.log_entry);
                    self.refresh_units().await;
                }
            }
            AppInternalEvent::Error(err) => {
                self.is_loading = false;
                self.push_log(err);
            }
            _ => {}
        }
    }

    fn push_log(&mut self, entry: impl Into<String>) {
        if self.logs.len() >= LOG_CAPACITY {
            self.logs.pop_front();
        }
        self.logs.push_back(entry.into());
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
