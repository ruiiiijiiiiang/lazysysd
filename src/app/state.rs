use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    process::{Command, Stdio},
    time::Duration,
};

use ansi_to_tui::IntoText;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
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

pub struct App {
    pub units: Vec<UnitInfo>,
    pub filtered_units: Vec<usize>,
    pub list_state: ListState,
    pub selected_unit_key: UnitSelectionKey,
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
    pub file_search_mode: bool,
    pub file_search_query: String,
    pub file_search_cursor: usize,
    pub file_search_match: Option<usize>,
    pub pending_action: Option<PendingAction>,
    pub pending_edit_review: Option<EditReview>,

    pub embedded_auth: Option<EmbeddedAuthFlow>,
    pub active_privileged_action: Option<PrivilegedAction>,
    pub internal_tx: mpsc::Sender<AppInternalEvent>,

    pub matcher: SkimMatcherV2,
    pub is_loading: bool,

    pub visual_select: bool,
    pub visual_line_select: bool,
    pub search_cursor: usize,
    pub log_search_mode: bool,
    pub log_search_query: String,
    pub log_search_cursor: usize,
    pub selected_log_lines: HashSet<usize>,
    pub selected_log_line_marks: Vec<usize>,
    pub pending_nav_prefix: Option<char>,
}

impl App {
    pub fn blank(internal_tx: mpsc::Sender<AppInternalEvent>) -> Self {
        Self {
            units: Vec::new(),
            filtered_units: Vec::new(),
            list_state: ListState::default(),
            selected_unit_key: UnitSelectionKey::default(),
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
            file_search_mode: false,
            file_search_query: String::new(),
            file_search_cursor: 0,
            file_search_match: None,
            pending_action: None,
            pending_edit_review: None,
            embedded_auth: None,
            active_privileged_action: None,
            internal_tx,
            matcher: SkimMatcherV2::default(),
            is_loading: true,
            visual_select: false,
            visual_line_select: false,
            search_cursor: 0,
            log_search_mode: false,
            log_search_query: String::new(),
            log_search_cursor: 0,
            selected_log_lines: HashSet::new(),
            selected_log_line_marks: Vec::new(),
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

    #[test]
    fn select_filtered_unit_index_tracks_selected_unit_key() {
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

        app.select_filtered_unit_index(Some(1));

        assert_eq!(app.selected_unit_key.name, "beta.service");
        assert_eq!(app.selected_unit_key.scope, "global");
        assert_eq!(app.selected_unit_key.path, "/test/unit/beta");
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
        app.unit_logs = vec!["one".to_string(), "two".to_string(), "three".to_string()];
        app.log_state.select(Some(0));

        app.toggle_log_line_mark();
        app.log_state.select(Some(1));
        app.toggle_log_line_mark();
        app.log_state.select(Some(2));
        app.toggle_log_line_mark();

        assert_eq!(app.selected_log_line_marks, vec![1, 2]);
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
        app.unit_logs = vec![
            "aaa foo aaa".to_string(),
            "foo bar".to_string(),
            "bar foo".to_string(),
            "baz".to_string(),
        ];
        app.log_search_query = "foo".to_string();

        assert_eq!(app.log_search_matches(), vec![0, 1, 2]);

        app.log_state.select(Some(0));
        app.cycle_log_search_match(true);
        assert_eq!(app.log_state.selected(), Some(1));

        app.cycle_log_search_match(false);
        assert_eq!(app.log_state.selected(), Some(0));
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
        app.unit_file_content =
            "[Service]\nExecStart=/usr/bin/foo\nExecStartPost=/usr/bin/foo --flag\n".to_string();
        app.file_search_query = "ExecStart".to_string();

        assert_eq!(app.file_search_matches(), vec![1, 2]);

        app.file_scroll = 0;
        app.cycle_file_search_match(true);
        assert_eq!(app.file_search_match, Some(1));
        assert_eq!(app.file_scroll, 1);

        app.cycle_file_search_match(true);
        assert_eq!(app.file_search_match, Some(2));
        assert_eq!(app.file_scroll, 2);

        app.cycle_file_search_match(false);
        assert_eq!(app.file_search_match, Some(1));
        assert_eq!(app.file_scroll, 1);
    }

    #[test]
    fn entering_search_mode_preserves_existing_queries() {
        let mut app = test_app(vec![unit(
            "alpha.service",
            "Alpha",
            "loaded",
            "active",
            "enabled",
            "/test/unit/alpha",
        )]);

        app.log_search_query = "foo".to_string();
        app.log_search_mode = false;
        app.file_search_query = "bar".to_string();
        app.file_search_mode = false;

        app.log_search_mode = true;
        app.file_search_mode = true;

        assert_eq!(app.log_search_query, "foo");
        assert_eq!(app.file_search_query, "bar");
    }

    #[test]
    fn edit_unit_search_key_moves_cursor_and_inserts_text() {
        let (tx, _rx) = mpsc::channel(1);
        let mut app = App::blank(tx);
        app.search_query = "foo".to_string();
        app.search_cursor = 1;

        app.edit_unit_search_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.search_cursor, 0);

        app.edit_unit_search_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        assert_eq!(app.search_query, "xfoo");
        assert_eq!(app.search_cursor, 1);
    }
}
