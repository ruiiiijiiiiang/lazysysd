use zbus::zvariant::OwnedObjectPath;

#[derive(Debug, Clone)]
pub struct UnitInfo {
    pub name: String,
    pub description: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub path: OwnedObjectPath,
}

pub struct AttemptResult {
    pub headline: String,
    pub detail: String,
    pub log_entry: String,
}

pub enum AppInternalEvent {
    PtyOutput(String),
    PtyClosed,
    AuthResult(AttemptResult),
    UnitsLoaded(Vec<UnitInfo>),
    LogsLoaded(Vec<String>),
    FileLoaded(String, String), // content, path
    Error(String),
}

pub enum PendingAction {
    EditFile(String),
}
