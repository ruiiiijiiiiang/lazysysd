use std::{
    collections::HashMap,
    io::{Error, Result},
    path::Path,
};

use serde::Deserialize;
use tokio::task;
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

    fn start_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;
    fn stop_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;
    fn restart_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;
    fn reload_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;
    fn enable_unit_files(
        &self,
        files: Vec<String>,
        runtime: bool,
        force: bool,
    ) -> ZbusResult<(bool, Vec<(String, String, String)>)>;
    fn disable_unit_files(
        &self,
        files: Vec<String>,
        runtime: bool,
    ) -> ZbusResult<Vec<(String, String, String)>>;
    fn mask_unit_files(
        &self,
        files: Vec<String>,
        runtime: bool,
        force: bool,
    ) -> ZbusResult<Vec<(String, String, String)>>;
    fn unmask_unit_files(&self, files: Vec<String>, runtime: bool) -> ZbusResult<Vec<(String, String, String)>>;
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

pub async fn get_unit_fragment_path(unit_path: &OwnedObjectPath) -> Result<String> {
    let connection = Connection::system()
        .await
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
    let connection = Connection::system()
        .await
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
    let mut units = Vec::with_capacity(units_raw.len());

    for u in units_raw {
        let enablement_state = resolve_enablement_state(&manager, &u.name, &unit_file_states).await;
        units.push(UnitInfo {
            name: u.name,
            description: u.description,
            load_state: u.load_state,
            active_state: u.active_state,
            enablement_state,
            sub_state: u.sub_state,
            path: u.path,
        });
    }

    Ok(units)
}

pub async fn perform_unit_action(name: &str, action: &str) -> AttemptResult {
    match action {
        "start" | "stop" | "restart" | "reload" | "enable" | "disable" | "mask" | "unmask" => {
            run_systemctl_unit_action(name, action).await
        }
        _ => AttemptResult {
            success: false,
            log_entry: format!("Unknown action: {}", action),
        },
    }
}

async fn run_systemctl_unit_action(name: &str, action: &str) -> AttemptResult {
    let unit_name = name.to_string();
    let systemctl_action = action.to_string();

    match task::spawn_blocking(move || {
        let output = std::process::Command::new("systemctl")
            .arg(&systemctl_action)
            .arg(&unit_name)
            .output()
            .map_err(|e| format!("Failed to run systemctl: {}", e))?;

        if output.status.success() {
            Ok(AttemptResult {
                success: true,
                log_entry: format!("{} on {} completed", systemctl_action, unit_name),
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let err_msg = if stderr.is_empty() {
                format!("exit code {}", output.status.code().unwrap_or(-1))
            } else {
                stderr
            };
            Ok(AttemptResult {
                success: false,
                log_entry: format!("{} on {} failed: {}", systemctl_action, unit_name, err_msg),
            })
        }
    })
    .await {
        Ok(res) => match res {
            Ok(r) => r,
            Err(e) => AttemptResult {
                success: false,
                log_entry: e,
            }
        },
        Err(e) => AttemptResult {
            success: false,
            log_entry: format!("Task failed: {}", e),
        }
    }
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

    manager
        .get_unit_file_state(unit_name)
        .await
        .unwrap_or_else(|_| "unknown".to_string())
}
