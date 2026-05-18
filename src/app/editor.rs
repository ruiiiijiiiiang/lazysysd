use std::{
    env, fs,
    io::{Error, Result},
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use which::which;

use crate::{
    app::utils::strip_ansi_content,
    models::{EditRequest, UnitEditMode, UnitScope},
};

pub fn resolve_editor() -> Result<String> {
    let candidates = env::var("VISUAL")
        .ok()
        .filter(|e| !e.is_empty())
        .or_else(|| env::var("EDITOR").ok().filter(|e| !e.is_empty()))
        .map(|e| vec![e])
        .unwrap_or_else(|| {
            ["nano", "vim", "emacs", "vi"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        });

    for editor in &candidates {
        if which(editor).is_ok() {
            return Ok(editor.clone());
        }
    }

    Err(Error::other(
        "No text editor found. Set $VISUAL or $EDITOR, or install one of: nano, vim, emacs, vi",
    ))
}

pub fn run_editor_request(
    initial_content: String,
    unit_name: String,
    scope: UnitScope,
    mode: UnitEditMode,
    restore_content: String,
    restore_path: String,
) -> Result<(EditRequest, String)> {
    let editor = resolve_editor()?;
    let temp_path = create_temp_edit_path(&unit_name, mode);
    fs::write(&temp_path, &initial_content)
        .map_err(|e| Error::other(format!("Failed to prepare edit draft: {e}")))?;

    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{} \"$0\"", editor))
        .arg(&temp_path)
        .status()
        .map_err(|e| Error::other(format!("Failed to start editor: {e}")))?;

    let edited_content = fs::read_to_string(&temp_path)
        .map_err(|e| Error::other(format!("Failed to read edited draft: {e}")))?;
    let _ = fs::remove_file(&temp_path);

    let request = EditRequest {
        unit_name,
        scope,
        mode,
        initial_content,
        restore_content,
        restore_path,
    };

    if !status.success() && edited_content == request.initial_content {
        let initial_content = request.initial_content.clone();
        return Ok((request, initial_content));
    }

    Ok((request, edited_content))
}

pub fn run_editor_text(filename: String, content: String) -> Result<String> {
    let editor = resolve_editor()?;
    let temp_path = env::temp_dir().join(format!("sdctl-{filename}-{}.log", unique_suffix()));
    fs::write(&temp_path, strip_ansi_content(&content))
        .map_err(|e| Error::other(format!("Failed to prepare log draft: {e}")))?;

    let status = Command::new("sh")
        .arg("-c")
        .arg(format!("{} \"$0\"", editor))
        .arg(&temp_path)
        .status()
        .map_err(|e| Error::other(format!("Failed to start editor: {e}")))?;

    let edited_content = fs::read_to_string(&temp_path)
        .map_err(|e| Error::other(format!("Failed to read edited log draft: {e}")))?;
    let _ = fs::remove_file(&temp_path);

    if !status.success() && edited_content == content {
        return Ok(content);
    }

    Ok(edited_content)
}

fn create_temp_edit_path(unit_name: &str, mode: UnitEditMode) -> PathBuf {
    let mut sanitized_name = unit_name.replace(|c: char| !c.is_ascii_alphanumeric(), "_");
    if sanitized_name.is_empty() {
        sanitized_name = "unit".to_string();
    }

    let mode_label = match mode {
        UnitEditMode::Override => "override",
        UnitEditMode::Full => "full",
    };
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    env::temp_dir().join(format!("sdctl-{sanitized_name}-{mode_label}-{unique}.conf"))
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
