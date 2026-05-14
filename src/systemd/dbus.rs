use std::io::{Error, Result};

use serde::Deserialize;
use zbus::{
    Connection, Error as ZbusError, Result as ZbusResult,
    zvariant::Type,
    {proxy, zvariant::OwnedObjectPath},
};

use crate::models::{AttemptResult, UnitInfo};

const JOB_MODE: &str = "replace";

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
    fn load_unit(&self, name: &str) -> ZbusResult<OwnedObjectPath>;

    #[zbus(allow_interactive_auth)]
    fn restart_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;

    #[zbus(allow_interactive_auth)]
    fn stop_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;

    #[zbus(allow_interactive_auth)]
    fn start_unit(&self, name: &str, mode: &str) -> ZbusResult<OwnedObjectPath>;
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

    Ok(units_raw
        .into_iter()
        .map(|u| UnitInfo {
            name: u.name,
            description: u.description,
            load_state: u.load_state,
            active_state: u.active_state,
            sub_state: u.sub_state,
            path: u.path,
        })
        .collect())
}

pub async fn perform_unit_action(name: &str, action: &str) -> AttemptResult {
    match perform_unit_action_inner(name, action).await {
        Ok(res) => res,
        Err(e) => AttemptResult {
            headline: format!("Action failed: {}", action),
            detail: e.to_string(),
            log_entry: format!("{} on {} failed: {}", action, name, e),
        },
    }
}

async fn perform_unit_action_inner(name: &str, action: &str) -> Result<AttemptResult> {
    let connection = Connection::system()
        .await
        .map_err(|e| Error::other(e.to_string()))?;
    let manager = SystemdManagerProxy::new(&connection)
        .await
        .map_err(|e| Error::other(e.to_string()))?;

    let result = match action {
        "restart" => manager.restart_unit(name, JOB_MODE).await,
        "stop" => manager.stop_unit(name, JOB_MODE).await,
        "start" => manager.start_unit(name, JOB_MODE).await,
        _ => return Err(Error::other("Unknown action")),
    };

    match result {
        Ok(path) => Ok(AttemptResult {
            headline: "Success".to_string(),
            detail: format!("Job queued: {}", path.as_str()),
            log_entry: format!("{} on {} queued", action, name),
        }),
        Err(e) => Ok(classify_systemd_error(e, name)),
    }
}

fn classify_systemd_error(err: ZbusError, target: &str) -> AttemptResult {
    let detail = err.to_string();
    let lower = detail.to_ascii_lowercase();
    let headline = if lower.contains("accessdenied") || lower.contains("not authorized") {
        format!("Rejected: Authorization failed for {}", target)
    } else {
        format!("Systemd error on {}", target)
    };

    AttemptResult {
        headline,
        detail,
        log_entry: format!("error on {}: {}", target, err),
    }
}
