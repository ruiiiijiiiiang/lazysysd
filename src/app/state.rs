use std::{
    cell::RefCell,
    collections::HashSet,
    fs,
    io::{Read, Write},
    process::{Command, Stdio},
    time::Duration,
};

use ansi_to_tui::IntoText;
use nucleo_matcher::{
    Matcher, Utf32Str,
    pattern::{CaseMatching, Normalization, Pattern},
};
use ratatui::widgets::ListState;
use tokio::{spawn, sync::mpsc};

use crate::{
    models::{AppInternalEvent, EditReview, PendingAction, PrivilegedAction, UnitInfo},
    systemd::{
        auth::EmbeddedAuthFlow,
        dbus::{fetch_all_units, get_unit_fragment_path},
        journal::JournalManager,
    },
};

pub const AUTH_START_DELAY: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewMode {
    UnitList,
    LogView,
    FileView,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchInputAction {
    Edit,
    Cursor,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnitSelectionKey {
    pub name: String,
    pub scope: String,
    pub path: String,
}

#[derive(Default)]
pub struct UnitListState {
    pub units: Vec<UnitInfo>,
    pub filtered_indices: Vec<usize>,
    pub state: ListState,
    pub selected_key: UnitSelectionKey,
    pub active_filter: Option<String>,
    pub enablement_filter: Option<String>,
    pub load_filter: Option<String>,
    pub scope_filter: Option<String>,
    pub open_filter_menu: Option<FilterMenu>,
}

#[derive(Default)]
pub struct LogViewState {
    pub logs: Vec<String>,
    pub state: ListState,
    pub visual_select: bool,
    pub visual_line_select: bool,
    pub selected_lines: HashSet<usize>,
    pub line_marks: Vec<usize>,
}

#[derive(Default)]
pub struct FileViewState {
    pub content: String,
    pub path: String,
    pub scroll: u16,
    pub search_match: Option<usize>,
}

impl UnitListState {
    pub fn select_index(&mut self, index: Option<usize>) {
        self.state.select(index);
        self.selected_key = index
            .and_then(|i| self.filtered_indices.get(i).copied())
            .and_then(|unit_index| self.units.get(unit_index))
            .map(|unit| UnitSelectionKey {
                name: unit.name.clone(),
                scope: unit.scope.clone(),
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

impl LogViewState {
    pub fn move_selection(&mut self, delta: i32) {
        if self.logs.is_empty() {
            return;
        }
        let i = match self.state.selected() {
            Some(i) => {
                let next = i as i32 + delta;
                if next < 0 {
                    0
                } else if next >= self.logs.len() as i32 {
                    self.logs.len() - 1
                } else {
                    next as usize
                }
            }
            None => {
                if delta > 0 {
                    0
                } else {
                    self.logs.len().saturating_sub(1)
                }
            }
        };
        self.state.select(Some(i));
    }

    pub fn toggle_line_mark(&mut self) {
        let Some(index) = self.state.selected() else {
            return;
        };

        if self.line_marks.contains(&index) {
            self.line_marks.retain(|&i| i != index);
            return;
        }

        if self.line_marks.len() == 2 {
            self.line_marks.remove(0);
        }

        self.line_marks.push(index);
    }

    pub fn selected_line_range(&self) -> Option<(usize, usize)> {
        match self.line_marks.as_slice() {
            [only] => Some((*only, *only)),
            [start, end] => Some(((*start).min(*end), (*start).max(*end))),
            _ => None,
        }
    }

    pub fn selected_lines_text(&self) -> Option<String> {
        let (start, end) = self.selected_line_range()?;
        let lines: Vec<&str> = (start..=end)
            .filter_map(|index| self.logs.get(index).map(|line| line.as_str()))
            .collect();

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    pub fn clear_visual_modes(&mut self) {
        self.visual_select = false;
        self.visual_line_select = false;
        self.selected_lines.clear();
        self.line_marks.clear();
    }
}

impl FileViewState {
    pub fn move_scroll(&mut self, delta: i32, _height: u16) {
        let total_lines = self.content.lines().count() as i32;
        let next_scroll = self.scroll as i32 + delta;
        self.scroll = next_scroll
            .clamp(0, total_lines.saturating_sub(1).max(0)) as u16;
    }
}

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
            NavAction::Up => self.move_scroll(-1, height),
            NavAction::Down => self.move_scroll(1, height),
            NavAction::HalfPageUp => self.move_scroll(-half_height, height),
            NavAction::HalfPageDown => self.move_scroll(half_height, height),
            NavAction::PageUp => self.move_scroll(-(height as i32), height),
            NavAction::PageDown => self.move_scroll(height as i32, height),
            NavAction::Top => self.scroll = 0,
            NavAction::Bottom => {
                self.scroll = total_lines.saturating_sub(height as i32).max(0) as u16
            }
        }
    }
}

#[derive(Default)]
pub struct SearchState {
    pub query: String,
    pub is_active: bool,
    pub cursor: usize,
}

impl SearchState {
    pub fn start(&mut self) {
        self.is_active = true;
        self.cursor = self.query.chars().count();
    }

    pub fn clear(&mut self) {
        self.is_active = false;
        self.query.clear();
        self.cursor = 0;
    }
}
pub struct App {
    pub view_mode: ViewMode,
    pub unit_list: UnitListState,
    pub log_view: LogViewState,
    pub file_view: FileViewState,
    pub search: SearchState,

    pub last_area_height: u16,
    pub pending_action: Option<PendingAction>,
    pub pending_edit_review: Option<EditReview>,

    pub embedded_auth: Option<EmbeddedAuthFlow>,
    pub active_privileged_action: Option<PrivilegedAction>,
    pub internal_tx: mpsc::Sender<AppInternalEvent>,

    pub matcher: RefCell<Matcher>,
    pub is_loading: bool,
    pub pending_nav_prefix: Option<char>,
}

impl App {
    pub fn blank(internal_tx: mpsc::Sender<AppInternalEvent>) -> Self {
        Self {
            view_mode: ViewMode::UnitList,
            unit_list: UnitListState::default(),
            log_view: LogViewState::default(),
            file_view: FileViewState::default(),
            search: SearchState::default(),
            last_area_height: 0,
            pending_action: None,
            pending_edit_review: None,
            embedded_auth: None,
            active_privileged_action: None,
            internal_tx,
            matcher: RefCell::new(Matcher::default()),
            is_loading: true,
            pending_nav_prefix: None,
        }
    }

    pub async fn new(internal_tx: mpsc::Sender<AppInternalEvent>) -> Self {
        let mut app = Self::blank(internal_tx);
        app.refresh_units().await;
        app
    }

    pub fn enter_unit_list_view(&mut self) {
        self.view_mode = ViewMode::UnitList;
        self.search.clear();
        self.log_view.clear_visual_modes();
        self.file_view.search_match = None;
    }

    pub async fn enter_log_view(&mut self) {
        if let Some(unit) = self.get_selected_unit() {
            let name = unit.name.clone();
            let scope = unit.scope.clone();
            self.search.clear();
            self.view_mode = ViewMode::LogView;
            self.log_view.logs.clear();
            self.log_view.state.select(None);
            self.log_view.clear_visual_modes();
            self.fetch_unit_logs(name, scope).await;
        }
    }

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

    pub async fn refresh_units(&mut self) {
        self.is_loading = true;
        let tx = self.internal_tx.clone();
        spawn(async move {
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
        spawn(async move {
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
        spawn(async move {
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
                    match fs::read_to_string(&path) {
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

    pub fn search_score(&self, unit: &UnitInfo) -> Option<u32> {
        if self.search.query.is_empty() {
            return None;
        }

        let mut matcher = self.matcher.borrow_mut();
        let pattern = Pattern::parse(
            &self.search.query,
            CaseMatching::Ignore,
            Normalization::Smart,
        );
        let target = format!("{} {}", unit.name, unit.description);
        let mut buffer = Vec::new();
        pattern.score(Utf32Str::new(target.as_str(), &mut buffer), &mut matcher)
    }

    pub fn unit_matches_search(&self, unit: &UnitInfo) -> bool {
        self.search.query.is_empty() || self.search_score(unit).is_some()
    }

    pub fn matches_filter_value(selected: Option<&str>, actual: &str) -> bool {
        match selected {
            Some(expected) => expected == actual,
            None => true,
        }
    }
}

pub fn build_override_template(unit_name: &str, source_path: &str) -> String {
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
    let candidates: [(&str, &[&str]); 2] =
        [("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])];
    let sanitized = strip_ansi_content(text);
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
            let _ = stdin.write_all(sanitized.as_bytes());
        }

        match child.wait() {
            Ok(status) if status.success() => {
                return format!("Copied {} chars to clipboard via {}", sanitized.len(), cmd);
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

pub fn strip_ansi_content(content: &str) -> String {
    content
        .lines()
        .map(|line| match line.as_bytes().into_text() {
            Ok(text) => text
                .lines
                .into_iter()
                .map(|line| {
                    line.spans
                        .into_iter()
                        .map(|span| span.content.into_owned())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Err(_) => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tokio::sync::mpsc;
    use zbus::zvariant::OwnedObjectPath;

    use crate::models::{EditRequest, UnitEditMode, UnitInfo};

    fn test_app(units: Vec<UnitInfo>) -> App {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.unit_list.units = units;
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
        app.file_view.content = "[Service]\nExecStart=/usr/bin/ssh\n".to_string();
        app.file_view.path = "/usr/lib/systemd/system/ssh.service".to_string();

        app.finish_edit_request(
            EditRequest {
                unit_name: "ssh.service".to_string(),
                scope: "global".to_string(),
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
                "loaded",
                "active",
                "enabled",
                "/test/unit/alpha",
            ),
            unit(
                "beta.service",
                "Beta",
                "loaded",
                "active",
                "enabled",
                "/test/unit/beta",
            ),
        ]);

        app.unit_list.select_index(Some(1));

        assert_eq!(app.unit_list.selected_key.name, "beta.service");
        assert_eq!(app.unit_list.selected_key.scope, "global");
        assert_eq!(app.unit_list.selected_key.path, "/test/unit/beta");
        assert_eq!(
            app.get_selected_unit().map(|unit| unit.name.as_str()),
            Some("beta.service")
        );
    }

    #[test]
    fn selected_log_line_marks_keep_at_most_two_entries() {
        let mut app = test_app(vec![unit(
            "alpha.service",
            "Alpha",
            "loaded",
            "active",
            "enabled",
            "/test/unit/alpha",
        )]);
        app.log_view.logs = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        app.log_view.state.select(Some(0));

        app.toggle_log_line_mark();
        app.log_view.state.select(Some(1));
        app.toggle_log_line_mark();
        app.log_view.state.select(Some(2));
        app.toggle_log_line_mark();

        assert_eq!(app.log_view.line_marks, vec![1, 2]);
        assert_eq!(app.selected_log_line_range(), Some((1, 2)));
        assert_eq!(app.selected_log_lines_text().as_deref(), Some("two\nthree"));
    }

    #[test]
    fn log_search_helpers_match_exact_text_and_cycle() {
        let mut app = test_app(vec![unit(
            "alpha.service",
            "Alpha",
            "loaded",
            "active",
            "enabled",
            "/test/unit/alpha",
        )]);
        app.log_view.logs = vec![
            "aaa foo aaa".to_string(),
            "foo bar".to_string(),
            "bar foo".to_string(),
            "baz".to_string(),
        ];
        app.search.query = "foo".to_string();

        assert_eq!(app.log_search_matches(), vec![0, 1, 2]);

        app.log_view.state.select(Some(0));
        app.cycle_log_search_match(true);
        assert_eq!(app.log_view.state.selected(), Some(1));

        app.cycle_log_search_match(false);
        assert_eq!(app.log_view.state.selected(), Some(0));
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

    #[test]
    fn entering_search_mode_preserves_existing_query() {
        let mut app = test_app(vec![unit(
            "alpha.service",
            "Alpha",
            "loaded",
            "active",
            "enabled",
            "/test/unit/alpha",
        )]);

        app.search.query = "foo".to_string();

        app.start_search();

        assert_eq!(app.search.query, "foo");
        assert!(app.search.is_active);
    }

    #[test]
    fn handle_search_key_moves_cursor_and_inserts_text() {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.search.query = "foo".to_string();
        app.search.cursor = 1;

        app.handle_search_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.search.cursor, 0);

        app.handle_search_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(app.search.query, "xfoo");
        assert_eq!(app.search.cursor, 1);
    }
}
