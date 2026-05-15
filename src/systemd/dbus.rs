use std::{
    collections::HashMap,
    io::{Error, Result},
    path::Path,
};

use serde::Deserialize;
use zbus::{
    Connection, Result as ZbusResult,
    zvariant::Type,
    {proxy, zvariant::OwnedObjectPath},
};

use crate::models::{AttemptResult, UnitInfo};

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
    #[zbus(allow_interactive_auth)]
    fn enable_unit_files(
        &self,
        files: Vec<String>,
        runtime: bool,
        force: bool,
    ) -> ZbusResult<(bool, Vec<(String, String, String)>)>;
    #[zbus(allow_interactive_auth)]
    fn disable_unit_files(
        &self,
        files: Vec<String>,
        runtime: bool,
    ) -> ZbusResult<Vec<(String, String, String)>>;
    #[zbus(allow_interactive_auth)]
    fn mask_unit_files(
        &self,
        files: Vec<String>,
        runtime: bool,
        force: bool,
    ) -> ZbusResult<Vec<(String, String, String)>>;
    #[zbus(allow_interactive_auth)]
    fn unmask_unit_files(
        &self,
        files: Vec<String>,
        runtime: bool,
    ) -> ZbusResult<Vec<(String, String, String)>>;
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

    use futures::{StreamExt, stream};

    let units = stream::iter(units_raw)
        .map(|u| {
            let manager = &manager;
            let unit_file_states = &unit_file_states;
            async move {
                let enablement_state =
                    resolve_enablement_state(manager, &u.name, unit_file_states).await;
                UnitInfo {
                    name: u.name,
                    description: u.description,
                    scope: scope.to_string(),
                    load_state: u.load_state,
                    active_state: u.active_state,
                    enablement_state,
                    sub_state: u.sub_state,
                    path: u.path,
                }
            }
        })
        .buffer_unordered(10)
        .collect::<Vec<_>>()
        .await;

    Ok(units)
}

pub async fn perform_unit_action(name: &str, scope: &str, action: &str) -> AttemptResult {
    match run_dbus_unit_action(name, scope, action).await {
        Ok(res) => res,
        Err(_) => AttemptResult { success: false },
    }
}

async fn run_dbus_unit_action(name: &str, scope: &str, action: &str) -> ZbusResult<AttemptResult> {
    let connection = if scope == "session" {
        Connection::session().await?
    } else {
        Connection::system().await?
    };
    let manager = SystemdManagerProxy::new(&connection).await?;

    match action {
        "start" => {
            manager.start_unit(name, "replace").await?;
        }
        "stop" => {
            manager.stop_unit(name, "replace").await?;
        }
        "restart" => {
            manager.restart_unit(name, "replace").await?;
        }
        "reload" => {
            manager.reload_unit(name, "replace").await?;
        }
        "enable" => {
            manager
                .enable_unit_files(vec![name.to_string()], false, true)
                .await?;
        }
        "disable" => {
            manager
                .disable_unit_files(vec![name.to_string()], false)
                .await?;
        }
        "mask" => {
            manager
                .mask_unit_files(vec![name.to_string()], false, true)
                .await?;
        }
        "unmask" => {
            manager
                .unmask_unit_files(vec![name.to_string()], false)
                .await?;
        }
        _ => {
            return Ok(AttemptResult { success: false });
        }
    }

    Ok(AttemptResult { success: true })
}

fn build_unit_file_state_map(unit_files: Vec<(String, String)>) -> HashMap<String, String> {
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

fn template_unit_name(unit_name: &str) -> Option<String> {
    let (stem, suffix) = unit_name.rsplit_once('.')?;
    let (template, _) = stem.split_once('@')?;
    Some(format!("{template}@.{suffix}"))
}

async fn resolve_enablement_state(
    manager: &SystemdManagerProxy<'_>,
    unit_name: &str,
    unit_file_states: &HashMap<String, String>,
) -> String {
    if let Some(state) = find_cached_unit_file_state(unit_name, unit_file_states) {
        return state;
    }

    let has_file = unit_name.ends_with(".service")
        || unit_name.ends_with(".socket")
        || unit_name.ends_with(".timer")
        || unit_name.ends_with(".mount")
        || unit_name.ends_with(".automount")
        || unit_name.ends_with(".path")
        || unit_name.ends_with(".swap");

    if !has_file {
        return "static".to_string();
    }

    manager
        .get_unit_file_state(unit_name)
        .await
        .unwrap_or_else(|_| "transient".to_string())
}
