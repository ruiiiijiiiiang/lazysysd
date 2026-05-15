use zbus::zvariant::OwnedObjectPath;

#[derive(Debug, Clone)]
pub struct UnitInfo {
    pub name: String,
    pub description: String,
    pub scope: String,
    pub load_state: String,
    pub active_state: String,
    pub enablement_state: String,
    pub sub_state: String,
    pub path: OwnedObjectPath,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum UnitEditMode {
    Override,
    Full,
}

impl UnitEditMode {
    pub fn action_label(self) -> &'static str {
        match self {
            Self::Override => "override",
            Self::Full => "full replacement",
        }
    }

    pub fn draft_label(self, unit_name: &str) -> String {
        match self {
            Self::Override => format!("Draft Override: {unit_name}"),
            Self::Full => format!("Draft Replacement: {unit_name}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditRequest {
    pub unit_name: String,
    pub scope: String,
    pub mode: UnitEditMode,
    pub initial_content: String,
    pub restore_content: String,
    pub restore_path: String,
}

#[derive(Debug, Clone)]
pub struct EditReview {
    pub unit_name: String,
    pub scope: String,
    pub mode: UnitEditMode,
    pub edited_content: String,
    pub restore_content: String,
    pub restore_path: String,
}

#[derive(Debug, Clone)]
pub enum PrivilegedAction {
    UnitCommand {
        unit_name: String,
        scope: String,
        action: String,
    },
    ApplyEdit {
        unit_name: String,
        scope: String,
        mode: UnitEditMode,
        content: String,
    },
}

pub struct AttemptResult {
    pub success: bool,
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
    EditFile(EditRequest),
}
