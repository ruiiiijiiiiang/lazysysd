use std::cell::RefCell;

use nucleo_matcher::Matcher;
use tokio::sync::mpsc;

use crate::{
    app::{
        auth::EmbeddedAuthFlow,
        state::{file::FileViewState, log::LogViewState, search::SearchState, unit::UnitListState},
    },
    models::{
        AppInternalEvent, EditReview, Notification, NotificationType, PendingAction,
        PrivilegedAction, UnitScope,
    },
};

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
    Type,
    Scope,
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
    pub count: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct UnitSelectionKey {
    pub name: String,
    pub scope: UnitScope,
    pub path: String,
}

pub struct App {
    pub view_mode: ViewMode,
    pub unit_list: UnitListState,
    pub log_view: LogViewState,
    pub file_view: FileViewState,
    pub search: SearchState,

    pub terminal_size: (u16, u16),
    pub main_content_height: u16,
    pub pending_action: Option<PendingAction>,
    pub pending_edit_review: Option<EditReview>,

    pub embedded_auth: Option<EmbeddedAuthFlow>,
    pub active_privileged_action: Option<PrivilegedAction>,
    pub internal_tx: mpsc::Sender<AppInternalEvent>,

    pub matcher: RefCell<Matcher>,
    pub is_loading: bool,
    pub pending_nav_prefix: Option<char>,
    pub error_message: Option<String>,
    pub notification: Option<Notification>,
}

impl App {
    pub fn blank(internal_tx: mpsc::Sender<AppInternalEvent>) -> Self {
        Self {
            view_mode: ViewMode::UnitList,
            unit_list: UnitListState::default(),
            log_view: LogViewState::default(),
            file_view: FileViewState::default(),
            search: SearchState::default(),
            terminal_size: (0, 0),
            main_content_height: 0,
            pending_action: None,
            pending_edit_review: None,
            embedded_auth: None,
            active_privileged_action: None,
            internal_tx,
            matcher: RefCell::new(Matcher::default()),
            is_loading: true,
            pending_nav_prefix: None,
            error_message: None,
            notification: None,
        }
    }

    pub async fn new(internal_tx: mpsc::Sender<AppInternalEvent>) -> Self {
        let mut app = Self::blank(internal_tx);
        app.refresh_units(false).await;
        app
    }

    pub fn notify(&mut self, message: String, kind: NotificationType) {
        self.notification = Some(Notification { message, kind });

        let tx = self.internal_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let _ = tx.send(AppInternalEvent::ClearNotification).await;
        });
    }
}
