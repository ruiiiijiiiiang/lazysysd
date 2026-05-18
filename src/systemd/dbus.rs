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
    zvariant::{Type, Value},
    {proxy, zvariant::OwnedObjectPath},
};

use crate::models::{
    AttemptResult, UnitAction, UnitActiveState, UnitEnablementState, UnitInfo, UnitLoadState,
    UnitScope,
};

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

#[derive(Debug, Clone)]
struct EnablementInfo {
    pub path: String,
    pub state: String,
}

type UnitFileState = (String, String);
type UnitFileChange = (String, String, String);
type UnitFileChanges = Vec<UnitFileChange>;
type EnablementCacheMap = HashMap<(String, String), EnablementInfo>;

static ENABLEMENT_STATE_CACHE: OnceLock<Mutex<EnablementCacheMap>> = OnceLock::new();

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

    #[zbus(allow_interactive_auth)]
    fn start_transient_unit(
        &self,
        name: &str,
        mode: &str,
        properties: Vec<(&str, Value<'_>)>,
        aux: Vec<(&str, Vec<(&str, Value<'_>)>)>,
    ) -> ZbusResult<OwnedObjectPath>;
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
        if let Some(info) =
            resolve_cached_enablement_state(&scope_name, &unit.name, &unit_file_states)
        {
            let scope = scope_name.parse::<UnitScope>().unwrap_or(UnitScope::Global);
            units.push(UnitInfo {
                name: unit.name,
                description: unit.description,
                scope,
                load_state: unit
                    .load_state
                    .parse::<UnitLoadState>()
                    .unwrap_or(UnitLoadState::Unknown),
                active_state: unit
                    .active_state
                    .parse::<UnitActiveState>()
                    .unwrap_or(UnitActiveState::Unknown),
                enablement_state: info
                    .state
                    .parse::<UnitEnablementState>()
                    .unwrap_or(UnitEnablementState::Unknown),
                sub_state: unit.sub_state,
                path: unit.path,
                fragment_path: info.path,
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
            let info =
                resolve_enablement_state(manager, &scope, &unit.name, unit_file_states).await;
            let scope = scope.parse::<UnitScope>().unwrap_or(UnitScope::Global);
            UnitInfo {
                name: unit.name,
                description: unit.description,
                scope,
                load_state: unit
                    .load_state
                    .parse::<UnitLoadState>()
                    .unwrap_or(UnitLoadState::Unknown),
                active_state: unit
                    .active_state
                    .parse::<UnitActiveState>()
                    .unwrap_or(UnitActiveState::Unknown),
                enablement_state: info
                    .state
                    .parse::<UnitEnablementState>()
                    .unwrap_or(UnitEnablementState::Unknown),
                sub_state: unit.sub_state,
                path: unit.path,
                fragment_path: info.path,
            }
        }
    }))
    .buffer_unordered(10)
    .collect::<Vec<_>>()
    .await;

    units.extend(resolved_units);

    Ok(units)
}

pub async fn perform_unit_action(
    name: &str,
    scope: UnitScope,
    action: UnitAction,
) -> AttemptResult {
    match run_dbus_unit_action(name, scope, action).await {
        Ok(res) => res,
        Err(e) => AttemptResult {
            success: false,
            error: Some(e.to_string()),
        },
    }
}

async fn run_dbus_unit_action(
    name: &str,
    scope: UnitScope,
    action: UnitAction,
) -> ZbusResult<AttemptResult> {
    let connection = match scope {
        UnitScope::Session => Connection::session().await?,
        UnitScope::Global => Connection::system().await?,
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

    Ok(AttemptResult {
        success: true,
        error: None,
    })
}

fn build_unit_file_state_map(unit_files: Vec<UnitFileState>) -> HashMap<String, EnablementInfo> {
    unit_files
        .into_iter()
        .filter_map(|(path, state)| {
            let name = Path::new(&path).file_name()?.to_str()?.to_string();
            Some((name, EnablementInfo { path, state }))
        })
        .collect()
}

fn unit_has_file(unit_name: &str) -> bool {
    unit_name.ends_with(".service")
        || unit_name.ends_with(".socket")
        || unit_name.ends_with(".timer")
        || unit_name.ends_with(".mount")
        || unit_name.ends_with(".automount")
        || unit_name.ends_with(".path")
        || unit_name.ends_with(".swap")
        || unit_name.ends_with(".target")
        || unit_name.ends_with(".slice")
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
    unit_file_states: &HashMap<String, EnablementInfo>,
) -> EnablementInfo {
    if let Some(res) = resolve_cached_enablement_state(scope, unit_name, unit_file_states) {
        return res;
    }

    let state = manager
        .get_unit_file_state(unit_name)
        .await
        .unwrap_or_else(|_| "transient".to_string());

    let res = EnablementInfo {
        state,
        path: String::new(),
    };
    cache_enablement_state(scope, unit_name, res.clone());
    res
}

fn resolve_cached_enablement_state(
    scope: &str,
    unit_name: &str,
    unit_file_states: &HashMap<String, EnablementInfo>,
) -> Option<EnablementInfo> {
    if let Some(info) = unit_file_states.get(unit_name).cloned().or_else(|| {
        template_unit_name(unit_name)
            .and_then(|template_name| unit_file_states.get(&template_name).cloned())
    }) {
        return Some(info);
    }

    if !unit_has_file(unit_name) {
        return Some(EnablementInfo {
            state: "static".to_string(),
            path: String::new(),
        });
    }

    get_cached_enablement_state(scope, unit_name)
}

// Enablement state has to be fetched individually for each unit.
// Use a cache to avoid resolving them during each refresh.
fn enablement_state_cache() -> &'static Mutex<EnablementCacheMap> {
    ENABLEMENT_STATE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_enablement_state(scope: &str, unit_name: &str, info: EnablementInfo) {
    let mut cache = enablement_state_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.insert((scope.to_string(), unit_name.to_string()), info);
}

fn get_cached_enablement_state(scope: &str, unit_name: &str) -> Option<EnablementInfo> {
    let cache = enablement_state_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache
        .get(&(scope.to_string(), unit_name.to_string()))
        .cloned()
}

fn clear_enablement_state_cache() {
    let mut cache = enablement_state_cache()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.clear();
}
