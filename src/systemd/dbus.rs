use std::{
    collections::HashMap,
    io::{Error, Result},
    path::Path,
    sync::{Mutex, OnceLock},
};

use futures::{StreamExt, stream};
use serde::Deserialize;
use zbus::{
    Connection, Result as ZbusResult,
    zvariant::Type,
    {proxy, zvariant::OwnedObjectPath},
};

use crate::models::{AttemptResult, UnitAction, UnitInfo};

#[derive(Debug, Deserialize, Type)]
pub struct UnitRow {
    pub name: String,
    pub description: String,
    pub load_state: String,
    pub active_state: String,
    pub sub_state: String,
    pub _following: String,
    pub path: OwnedObjectPath,
    pub _job_id: u32,
    pub _job_type: String,
    pub _job_path: OwnedObjectPath,
}

type UnitFileState = (String, String);
type UnitFileChange = (String, String, String);
type UnitFileChanges = Vec<UnitFileChange>;

static ENABLEMENT_STATE_CACHE: OnceLock<Mutex<HashMap<(String, String), String>>> = OnceLock::new();

#[proxy(
    interface = "org.freedesktop.systemd1.Manager",
    default_service = "org.freedesktop.systemd1",
    default_path = "/org/freedesktop/systemd1"
)]
trait SystemdManager {
    fn list_units(&self) -> ZbusResult<Vec<UnitRow>>;
    fn list_unit_files(&self) -> ZbusResult<Vec<(String, String)>>;
    fn load_unit(&self, name: &str) -> ZbusResult<OwnedObjectPath>;
    fn get_unit_file_state(&self, name: &str) -> ZbusResult<String>;

    #[zbus(allow_interactive_auth)]
    fn start_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn stop_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn restart_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;
    #[zbus(allow_interactive_auth)]
    fn reload_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;
    #[zbus(name = "ResetFailedUnit", allow_interactive_auth)]
    fn reset_failed(&self, name: &str) -> ZbusResult<()>;
    #[zbus(allow_interactive_auth)]
    fn enable_unit_files(
        &self,
        files: Vec<String>,
        runtime: bool,
        force: bool,
    ) -> ZbusResult<(bool, UnitFileChanges)>;
    #[zbus(allow_interactive_auth)]
    fn disable_unit_files(&self, files: Vec<String>, runtime: bool) -> ZbusResult<UnitFileChanges>;
    #[zbus(allow_interactive_auth)]
    fn mask_unit_files(
        &self,
        files: Vec<String>,
        runtime: bool,
        force: bool,
    ) -> ZbusResult<UnitFileChanges>;
    #[zbus(allow_interactive_auth)]
    fn unmask_unit_files(&self, files: Vec<String>, runtime: bool) -> ZbusResult<UnitFileChanges>;
}

#[proxy(
    interface = "org.freedesktop.systemd1.Unit",
    default_service = "org.freedesktop.systemd1"
)]
trait SystemdUnit {
    #[zbus(property)]
    fn id(&self) -> ZbusResult<String>;
    #[zbus(property)]
    fn description(&self) -> ZbusResult<String>;
    #[zbus(property)]
    fn load_state(&self) -> ZbusResult<String>;
    #[zbus(property)]
    fn active_state(&self) -> ZbusResult<String>;
    #[zbus(property)]
    fn sub_state(&self) -> ZbusResult<String>;
    #[zbus(property)]
    fn fragment_path(&self) -> ZbusResult<String>;
}

pub async fn get_unit_fragment_path(unit_path: &OwnedObjectPath, scope: &str) -> Result<String> {
    let connection = if scope == "session" {
        Connection::session().await
    } else {
        Connection::system().await
    }
    .map_err(|e| Error::other(format!("D-Bus connect failed: {e}")))?;

    let unit = SystemdUnitProxy::builder(&connection)
        .path(unit_path.clone())
        .map_err(|e| Error::other(format!("Proxy builder failed: {e}")))?
        .build()
        .await
        .map_err(|e| Error::other(format!("Proxy build failed: {e}")))?;

    unit.fragment_path()
        .await
        .map_err(|e| Error::other(format!("Failed to get FragmentPath: {e}")))
}

pub async fn fetch_all_units() -> Result<Vec<UnitInfo>> {
    let mut all_units = Vec::new();
    if let Ok(system_units) = fetch_units_from_scope("global").await {
        all_units.extend(system_units);
    }
    if let Ok(session_units) = fetch_units_from_scope("session").await {
        all_units.extend(session_units);
    }
    Ok(all_units)
}

async fn fetch_units_from_scope(scope: &str) -> Result<Vec<UnitInfo>> {
    let connection = if scope == "session" {
        Connection::session().await
    } else {
        Connection::system().await
    }
    .map_err(|e| Error::other(format!("D-Bus connect failed: {e}")))?;

    let manager = SystemdManagerProxy::new(&connection)
        .await
        .map_err(|e| Error::other(format!("Proxy create failed: {e}")))?;

    let units_raw = manager
        .list_units()
        .await
        .map_err(|e| Error::other(format!("list_units failed: {e}")))?;
    let unit_file_states = build_unit_file_state_map(
        manager
            .list_unit_files()
            .await
            .map_err(|e| Error::other(format!("list_unit_files failed: {e}")))?,
    );

    let scope_name = scope.to_string();
    let mut units = Vec::with_capacity(units_raw.len());
    let mut unresolved_units = Vec::new();

    for unit in units_raw {
        if let Some(enablement_state) =
            resolve_enablement_state_cached(&scope_name, &unit.name, &unit_file_states)
        {
            units.push(UnitInfo {
                name: unit.name,
                description: unit.description,
                scope: scope_name.clone(),
                load_state: unit.load_state,
                active_state: unit.active_state,
                enablement_state,
                sub_state: unit.sub_state,
                path: unit.path,
            });
        } else {
            unresolved_units.push(unit);
        }
    }

    let resolved_units = stream::iter(unresolved_units.into_iter().map(|unit| {
        let manager = &manager;
        let unit_file_states = &unit_file_states;
        let scope = scope_name.clone();
        async move {
            let enablement_state =
                resolve_enablement_state(manager, &scope, &unit.name, unit_file_states).await;
            UnitInfo {
                name: unit.name,
                description: unit.description,
                scope,
                load_state: unit.load_state,
                active_state: unit.active_state,
                enablement_state,
                sub_state: unit.sub_state,
                path: unit.path,
            }
        }
    }))
    .buffer_unordered(10)
    .collect::<Vec<_>>()
    .await;

    units.extend(resolved_units);

    Ok(units)
}

pub async fn perform_unit_action(name: &str, scope: &str, action: UnitAction) -> AttemptResult {
    match run_dbus_unit_action(name, scope, action).await {
        Ok(res) => res,
        Err(_) => AttemptResult { success: false },
    }
}

async fn run_dbus_unit_action(
    name: &str,
    scope: &str,
    action: UnitAction,
) -> ZbusResult<AttemptResult> {
    let connection = if scope == "session" {
        Connection::session().await?
    } else {
        Connection::system().await?
    };
    let manager = SystemdManagerProxy::new(&connection).await?;

    let mut invalidate_enablement_cache = false;

    match action {
        UnitAction::Start => {
            manager.start_unit(name, "replace").await?;
        }
        UnitAction::Stop => {
            manager.stop_unit(name, "replace").await?;
        }
        UnitAction::Restart => {
            manager.restart_unit(name, "replace").await?;
        }
        UnitAction::Reload => {
            manager.reload_unit(name, "replace").await?;
        }
        UnitAction::ResetFailed => {
            manager.reset_failed(name).await?;
        }
        UnitAction::Enable => {
            manager
                .enable_unit_files(vec![name.to_string()], false, true)
                .await?;
            invalidate_enablement_cache = true;
        }
        UnitAction::Disable => {
            manager
                .disable_unit_files(vec![name.to_string()], false)
                .await?;
            invalidate_enablement_cache = true;
        }
        UnitAction::Mask => {
            manager
                .mask_unit_files(vec![name.to_string()], false, true)
                .await?;
            invalidate_enablement_cache = true;
        }
        UnitAction::Unmask => {
            manager
                .unmask_unit_files(vec![name.to_string()], false)
                .await?;
            invalidate_enablement_cache = true;
        }
    }

    if invalidate_enablement_cache {
        clear_enablement_state_cache();
    }

    Ok(AttemptResult { success: true })
}

fn build_unit_file_state_map(unit_files: Vec<UnitFileState>) -> HashMap<String, String> {
    unit_files
        .into_iter()
        .filter_map(|(path, state)| {
            Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| (name.to_string(), state))
        })
        .collect()
}

fn find_cached_unit_file_state(
    unit_name: &str,
    unit_file_states: &HashMap<String, String>,
) -> Option<String> {
    unit_file_states.get(unit_name).cloned().or_else(|| {
        template_unit_name(unit_name)
            .and_then(|template_name| unit_file_states.get(&template_name).cloned())
    })
}

fn enablement_state_cache() -> &'static Mutex<HashMap<(String, String), String>> {
    ENABLEMENT_STATE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn clear_enablement_state_cache() {
    let mut cache = enablement_state_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.clear();
}

fn cache_enablement_state(scope: &str, unit_name: &str, state: &str) {
    let mut cache = enablement_state_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.insert(
        (scope.to_string(), unit_name.to_string()),
        state.to_string(),
    );
}

fn cached_enablement_state(scope: &str, unit_name: &str) -> Option<String> {
    let cache = enablement_state_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache
        .get(&(scope.to_string(), unit_name.to_string()))
        .cloned()
}

fn unit_has_file(unit_name: &str) -> bool {
    unit_name.ends_with(".service")
        || unit_name.ends_with(".socket")
        || unit_name.ends_with(".timer")
        || unit_name.ends_with(".mount")
        || unit_name.ends_with(".automount")
        || unit_name.ends_with(".path")
        || unit_name.ends_with(".swap")
}

fn resolve_enablement_state_cached(
    scope: &str,
    unit_name: &str,
    unit_file_states: &HashMap<String, String>,
) -> Option<String> {
    if let Some(state) = find_cached_unit_file_state(unit_name, unit_file_states) {
        return Some(state);
    }

    if !unit_has_file(unit_name) {
        return Some("static".to_string());
    }

    cached_enablement_state(scope, unit_name)
}

fn template_unit_name(unit_name: &str) -> Option<String> {
    let (stem, suffix) = unit_name.rsplit_once('.')?;
    let (template, _) = stem.split_once('@')?;
    Some(format!("{template}@.{suffix}"))
}

async fn resolve_enablement_state(
    manager: &SystemdManagerProxy<'_>,
    scope: &str,
    unit_name: &str,
    unit_file_states: &HashMap<String, String>,
) -> String {
    if let Some(state) = resolve_enablement_state_cached(scope, unit_name, unit_file_states) {
        return state;
    }

    let state = manager
        .get_unit_file_state(unit_name)
        .await
        .unwrap_or_else(|_| "transient".to_string());
    cache_enablement_state(scope, unit_name, &state);
    state
}
