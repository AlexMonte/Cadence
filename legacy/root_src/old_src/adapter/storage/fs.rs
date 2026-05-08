use std::fs;
use std::path::{Path, PathBuf};

#[cfg(not(test))]
use directories::ProjectDirs;
use rfd::AsyncFileDialog;

use super::WorkspaceProjectSnapshot;

const RECOVERY_DIR_NAME: &str = "cadence-recovery";
const RECOVERY_FILE_NAME: &str = "project-recovery.cadence.json";
const WORKSPACE_FILE_NAME: &str = "workspace-session.json";

pub(crate) fn parent_location(location: &str) -> Option<String> {
    Path::new(location)
        .parent()
        .map(|parent| parent.display().to_string())
}

pub(crate) fn file_name(location: &str) -> Option<String> {
    Path::new(location)
        .file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
}

fn app_support_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CADENCE_APP_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }

    #[cfg(test)]
    let default_dir =
        std::env::temp_dir().join(format!("cadence-test-app-data-{}", std::process::id()));
    #[cfg(not(test))]
    let default_dir = ProjectDirs::from("com", "cadence", "Cadence")
        .map(|dirs| dirs.data_local_dir().to_path_buf())
        .ok_or_else(|| "unable to resolve Cadence app data directory".to_string())?;

    Ok(default_dir)
}

fn workspace_snapshot_path() -> Result<PathBuf, String> {
    Ok(app_support_dir()?.join(WORKSPACE_FILE_NAME))
}

pub(crate) fn current_workspace_project() -> Result<Option<WorkspaceProjectSnapshot>, String> {
    #[cfg(test)]
    if std::env::var_os("CADENCE_ENABLE_TEST_WORKSPACE_SESSION").is_none() {
        return Ok(None);
    }

    let path = workspace_snapshot_path()?;
    if !path.is_file() {
        return Ok(None);
    }

    let payload = fs::read_to_string(&path).map_err(|err| err.to_string())?;
    let snapshot: WorkspaceProjectSnapshot =
        serde_json::from_str(payload.as_str()).map_err(|err| err.to_string())?;
    if snapshot.payload.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(snapshot))
}

pub(crate) fn write_workspace_project(snapshot: &WorkspaceProjectSnapshot) -> Result<(), String> {
    #[cfg(test)]
    if std::env::var_os("CADENCE_ENABLE_TEST_WORKSPACE_SESSION").is_none() {
        return Ok(());
    }

    if snapshot.payload.trim().is_empty() {
        return Err("cannot persist empty workspace payload".to_string());
    }

    let path = workspace_snapshot_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let payload = serde_json::to_string_pretty(snapshot).map_err(|err| err.to_string())?;
    fs::write(path, payload).map_err(|err| err.to_string())
}

#[allow(dead_code)]
pub(crate) fn clear_workspace_project() -> Result<(), String> {
    #[cfg(test)]
    if std::env::var_os("CADENCE_ENABLE_TEST_WORKSPACE_SESSION").is_none() {
        return Ok(());
    }

    let path = workspace_snapshot_path()?;
    if path.exists() {
        fs::remove_file(path).map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub(crate) fn load_project(location: &str) -> Result<String, String> {
    let path = Path::new(location);
    if !path.is_file() {
        return Err(format!("project file not found: {}", path.display()));
    }

    fs::read_to_string(path).map_err(|err| err.to_string())
}

pub(crate) fn save_project(location: &str, payload: &str) -> Result<String, String> {
    if payload.trim().is_empty() {
        return Err("cannot save empty payload".to_string());
    }

    let path = Path::new(location);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    fs::write(path, payload).map_err(|err| err.to_string())?;
    Ok(path.display().to_string())
}

pub(crate) async fn pick_open_path(
    initial_directory: Option<&str>,
) -> Result<Option<String>, String> {
    let mut dialog = AsyncFileDialog::new().add_filter("Cadence Project", &["json"]);
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(PathBuf::from(directory));
    }
    Ok(dialog
        .pick_file()
        .await
        .map(|handle| handle.path().display().to_string()))
}

pub(crate) async fn pick_save_path(
    initial_directory: Option<&str>,
    initial_name: Option<&str>,
    default_name: &str,
) -> Result<Option<String>, String> {
    let mut dialog = AsyncFileDialog::new().add_filter("Cadence Project", &["json"]);
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(PathBuf::from(directory));
    }
    dialog = dialog.set_file_name(initial_name.unwrap_or(default_name));
    Ok(dialog
        .save_file()
        .await
        .map(|handle| handle.path().display().to_string()))
}

pub(crate) async fn pick_export_path(
    initial_directory: Option<&str>,
    file_name: &str,
) -> Result<Option<String>, String> {
    let mut dialog = AsyncFileDialog::new().add_filter("Cadence Debug Snapshot", &["txt"]);
    if let Some(directory) = initial_directory {
        dialog = dialog.set_directory(PathBuf::from(directory));
    }
    dialog = dialog.set_file_name(file_name);

    Ok(dialog
        .save_file()
        .await
        .map(|handle| handle.path().display().to_string()))
}

pub(crate) fn save_text_export(location: &str, payload: &str) -> Result<String, String> {
    let path = Path::new(location);
    let mut text = payload.to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(path, text).map_err(|err| err.to_string())?;
    Ok(path.display().to_string())
}

pub(crate) fn recovery_snapshot_location() -> String {
    std::env::temp_dir()
        .join(RECOVERY_DIR_NAME)
        .join(RECOVERY_FILE_NAME)
        .display()
        .to_string()
}

pub(crate) fn recovery_snapshot_status() -> Result<Option<String>, String> {
    let path = PathBuf::from(recovery_snapshot_location());
    Ok(path.is_file().then(|| path.display().to_string()))
}

pub(crate) fn clear_recovery_snapshot() -> Result<String, String> {
    let path = PathBuf::from(recovery_snapshot_location());
    if path.exists() {
        fs::remove_file(&path).map_err(|err| err.to_string())?;
    }
    Ok(format!("cleared recovery snapshot at {}", path.display()))
}

pub(crate) fn write_recovery_snapshot(payload: &str) -> Result<String, String> {
    let path = PathBuf::from(recovery_snapshot_location());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    fs::write(&path, payload).map_err(|err| err.to_string())?;
    Ok(path.display().to_string())
}

pub(crate) fn load_recovery_snapshot() -> Result<String, String> {
    fs::read_to_string(PathBuf::from(recovery_snapshot_location())).map_err(|err| err.to_string())
}
