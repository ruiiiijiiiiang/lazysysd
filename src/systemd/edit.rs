use std::{
    io::{Error, Result},
    process::Stdio,
};

use tokio::{io::AsyncWriteExt, process::Command};

use crate::models::{AttemptResult, UnitEditMode};

const DEFAULT_DROP_IN: &str = "override.conf";

pub async fn perform_unit_edit(
    unit_name: &str,
    mode: UnitEditMode,
    content: String,
) -> AttemptResult {
    match perform_unit_edit_inner(unit_name, mode, content).await {
        Ok(result) => result,
        Err(err) => AttemptResult {
            success: false,
            log_entry: format!("{} for {} failed: {}", mode.action_label(), unit_name, err),
        },
    }
}

async fn perform_unit_edit_inner(
    unit_name: &str,
    mode: UnitEditMode,
    content: String,
) -> Result<AttemptResult> {
    let mut command = Command::new("systemctl");
    command.arg("edit");
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
        .map_err(|e| Error::other(format!("Failed to start systemctl edit: {e}")))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(normalize_edit_content(&content).as_bytes())
            .await
            .map_err(|e| Error::other(format!("Failed to stream edit content: {e}")))?;
    }

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| Error::other(format!("systemctl edit failed: {e}")))?;

    if output.status.success() {
        Ok(AttemptResult {
            success: true,
            log_entry: format!("applied {} for {}", mode.action_label(), unit_name),
        })
    } else {
        Ok(AttemptResult {
            success: false,
            log_entry: format!("{} for {} failed", mode.action_label(), unit_name),
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
