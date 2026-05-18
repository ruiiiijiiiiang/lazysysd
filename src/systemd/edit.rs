use std::{
    io::{Error, Result},
    path::Path,
    process::Stdio,
};

use tokio::{io::AsyncWriteExt, process::Command};

use crate::models::{AttemptResult, UnitEditMode, UnitScope};

const DEFAULT_DROP_IN: &str = "override.conf";

pub async fn perform_unit_edit(
    unit_name: &str,
    scope: UnitScope,
    mode: UnitEditMode,
    content: String,
) -> AttemptResult {
    match perform_unit_edit_inner(unit_name, scope, mode, content).await {
        Ok(result) => result,
        Err(e) => AttemptResult {
            success: false,
            error: Some(e.to_string()),
        },
    }
}

async fn perform_unit_edit_inner(
    unit_name: &str,
    scope: UnitScope,
    mode: UnitEditMode,
    content: String,
) -> Result<AttemptResult> {
    let mut command = match scope {
        UnitScope::Session => Command::new("systemctl"),
        UnitScope::Global => {
            let mut cmd = Command::new("pkexec");
            cmd.arg("systemctl");
            cmd
        }
    };

    if scope == UnitScope::Session {
        command.arg("--user");
    }

    command.arg("edit").arg("--force");
    if scope == UnitScope::Global && is_system_directory_read_only() {
        command.arg("--runtime");
    }

    match mode {
        UnitEditMode::Override => {
            command.arg(format!("--drop-in={DEFAULT_DROP_IN}"));
        }
        UnitEditMode::Full => {
            command.arg("--full");
        }
    }
    command
        .arg("--stdin")
        .arg(unit_name)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| Error::other(format!("Failed to start edit command: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(normalize_edit_content(&content).as_bytes())
            .await
            .map_err(|e| Error::other(format!("Failed to stream edit content: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| Error::other(format!("Edit command failed to wait: {e}")))?;

    if output.status.success() {
        Ok(AttemptResult {
            success: true,
            error: None,
        })
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        let error_msg = if err.trim().is_empty() {
            format!("Command exited with status: {}", output.status)
        } else {
            err.trim().to_string()
        };
        Ok(AttemptResult {
            success: false,
            error: Some(error_msg),
        })
    }
}

fn normalize_edit_content(content: &str) -> String {
    if content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{content}\n")
    }
}

// Special handler for immutable distros
fn is_system_directory_read_only() -> bool {
    use std::ffi::CString;
    use std::mem::MaybeUninit;

    let path = CString::new("/etc/systemd/system").unwrap_or_else(|_| CString::new("").unwrap());
    let mut stats = MaybeUninit::<libc::statvfs>::uninit();

    unsafe {
        if libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) == 0 {
            let stats = stats.assume_init();
            (stats.f_flag & libc::ST_RDONLY) != 0
        } else {
            // Fallback for NixOS specifically if statvfs fails for some reason
            Path::new("/etc/NIXOS").exists()
        }
    }
}
