use super::*;

const RECOVERY_DIR_NAME: &str = "cadence-recovery";
const RECOVERY_FILE_NAME: &str = "project-recovery.cadence.json";

pub(super) fn recovery_snapshot_path() -> PathBuf {
    std::env::temp_dir()
        .join(RECOVERY_DIR_NAME)
        .join(RECOVERY_FILE_NAME)
}

pub(super) fn clear_recovery_snapshot() -> AppResult<String> {
    let path = recovery_snapshot_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(format!("cleared recovery snapshot at {}", path.display()))
}

pub(super) fn write_recovery_snapshot(store: &AppStore) -> AppResult<String> {
    let project = active_project(store)?;
    let path = recovery_snapshot_path();
    let payload = serde_json::to_string_pretty(project)?;
    save_project(path.as_path(), payload.as_str())?;
    Ok(path.display().to_string())
}
