//! Platform-aware project storage and picker bindings.

use serde::{Deserialize, Serialize};

#[cfg(any(target_arch = "wasm32", test))]
const PROJECT_JSON_SUFFIX: &str = ".cadence.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct WorkspaceProjectSnapshot {
    pub location: Option<String>,
    pub payload: String,
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn canonical_project_name_seed(name: &str) -> String {
    let mut seed = name.trim();
    if seed.is_empty() {
        return "project".to_string();
    }

    while let Some(stripped) = seed.strip_suffix(PROJECT_JSON_SUFFIX) {
        seed = stripped.trim_end_matches('.');
    }

    if seed.is_empty() {
        "project".to_string()
    } else {
        seed.to_string()
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod fs;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
#[allow(unused_imports)]
pub(crate) use self::fs::{
    clear_recovery_snapshot, clear_workspace_project, current_workspace_project, file_name,
    load_project, load_recovery_snapshot, parent_location, pick_export_path, pick_open_path,
    pick_save_path, recovery_snapshot_location, recovery_snapshot_status, save_project,
    save_text_export, write_recovery_snapshot, write_workspace_project,
};
#[cfg(target_arch = "wasm32")]
#[allow(unused_imports)]
pub(crate) use self::web::{
    clear_recovery_snapshot, clear_workspace_project, current_workspace_project, file_name,
    flush_storage, load_project, load_recovery_snapshot, parent_location, pick_export_path,
    pick_open_path, pick_save_path, prepare_storage, recovery_snapshot_location,
    recovery_snapshot_status, save_project, save_text_export, workspace_project_location,
    write_recovery_snapshot, write_workspace_project,
};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn workspace_project_location(_project_name: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::canonical_project_name_seed;

    #[test]
    fn canonical_project_name_seed_defaults_empty_values() {
        assert_eq!(canonical_project_name_seed(""), "project");
        assert_eq!(canonical_project_name_seed("   "), "project");
    }

    #[test]
    fn canonical_project_name_seed_drops_repeated_project_suffixes() {
        assert_eq!(
            canonical_project_name_seed("project.cadence.json"),
            "project"
        );
        assert_eq!(
            canonical_project_name_seed("project.cadence.json.cadence.json"),
            "project"
        );
        assert_eq!(
            canonical_project_name_seed("project.cadence.json.cadence.json.cadence.json"),
            "project"
        );
    }

    #[test]
    fn canonical_project_name_seed_preserves_user_project_names() {
        assert_eq!(
            canonical_project_name_seed("Smoke IndexedDB"),
            "Smoke IndexedDB"
        );
    }
}
