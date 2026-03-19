//! Tauri commands and helpers for project lifecycle, persistence, and init-stage editing.

mod document;
mod init_stage;
mod recovery;

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rfd::AsyncFileDialog;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use self::document::*;
use self::init_stage::*;
use self::recovery::*;
use crate::errors::{AppError, AppResult};
use crate::model::{CadenceProjectDocument, InitStageApplyArgs, InitStageOp, InitStageSnapshotDto};
use crate::store::app_state::AppStore;
use crate::store::fs_store::{load_project, save_project};
use crate::store::history::{capture_snapshot, clear_graph_history, record_successful_mutation};
use tessera::graph::{Edge, Graph, Node, ProjectDocument};
use tessera::subgraph::{
    SUBGRAPH_INPUT_1_ID, SUBGRAPH_INPUT_2_ID, SUBGRAPH_INPUT_3_ID, SUBGRAPH_OUTPUT_ID,
};
use tessera::types::{EdgeId, GridPos};

const STANDARD_GRAPH_COLS: u32 = 12;
const STANDARD_GRAPH_ROWS: u32 = 8;

#[derive(Debug, Default)]
/// Shared Tauri state wrapper around the single in-memory [`AppStore`].
pub struct SharedAppState {
    pub store: Mutex<AppStore>,
    pub allow_main_window_close: Mutex<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Minimal project metadata returned after create/open operations.
pub struct ProjectDto {
    pub name: String,
    pub node_count: usize,
    pub edge_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Dirty flag and current path shown by the UI.
pub struct ProjectDirtyStatusDto {
    pub dirty: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Expanded project metadata used by the editor shell.
pub struct ProjectViewDto {
    pub name: String,
    pub schema_version: u32,
    pub node_count: usize,
    pub edge_count: usize,
    pub dirty: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
/// UI decision returned by the unsaved-changes prompt flow.
pub enum DirtyDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Clone, Deserialize)]
/// Payload for creating a new named project.
pub struct ProjectCreateArgs {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
/// Payload for opening a project by explicit path.
pub struct ProjectOpenArgs {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
/// Save payload; `None` means "use the current path".
pub struct ProjectSaveArgs {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// Payload for creating a fresh project, optionally naming it.
pub struct ProjectNewArgs {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
/// Payload for renaming the active project.
pub struct ProjectRenameArgs {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
/// Explicit path payload used by the open command family.
pub struct ProjectOpenPathArgs {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
/// Explicit path payload used by Save As.
pub struct ProjectSaveAsArgs {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Optional path wrapper used by file-picker and recovery-related commands.
pub struct ProjectPathChoiceDto {
    pub path: Option<String>,
}

/// Return the mutable active project or a consistent "no active project" error.
pub(crate) fn active_project_mut(store: &mut AppStore) -> AppResult<&mut CadenceProjectDocument> {
    store.current_project.as_mut().ok_or_else(|| {
        AppError::InvalidInput("no active project; create or open a project first".to_string())
    })
}

/// Return the active project or a consistent "no active project" error.
pub(crate) fn active_project(store: &AppStore) -> AppResult<&CadenceProjectDocument> {
    store.current_project.as_ref().ok_or_else(|| {
        AppError::InvalidInput("no active project; create or open a project first".to_string())
    })
}

pub(crate) fn project_bootstrap_internal(store: &mut AppStore) -> AppResult<ProjectViewDto> {
    if store.current_project.is_none() {
        project_new_internal(
            store,
            ProjectNewArgs {
                name: Some("Untitled".to_string()),
            },
        )?;
        store.push_diagnostic("project_bootstrap", "created Untitled project".to_string());
    }

    let project = active_project(store)?;
    Ok(project_to_view(store, project))
}

/// Mark the in-memory project as dirty without recalculating a fingerprint.
pub(crate) fn mark_store_dirty(store: &mut AppStore) {
    store.dirty = true;
}

/// Create a brand-new default project and reset editor/runtime state around it.
pub(crate) fn project_new_internal(
    store: &mut AppStore,
    args: ProjectNewArgs,
) -> AppResult<ProjectDto> {
    let name = validate_project_name(args.name.as_deref().unwrap_or("Untitled"))?;
    let history_snapshot = capture_snapshot(store);
    store.current_project = Some(default_project_graph(name.as_str()));
    clear_graph_history(store);
    store.reset_on_project_swap();
    mark_store_clean(store)?;
    let _ = clear_recovery_snapshot();
    record_successful_mutation(store, history_snapshot);
    store.push_diagnostic("project_new", name.clone());
    Ok(project_dto(active_project(store)?))
}

/// Rename the active project and keep its runtime graph name in sync.
pub(crate) fn project_rename_internal(
    store: &mut AppStore,
    args: ProjectRenameArgs,
) -> AppResult<ProjectViewDto> {
    let name = validate_project_name(args.name.as_str())?;
    let current_name = active_project(store)?.name.clone();
    if current_name == name {
        let project = active_project(store)?;
        return Ok(project_to_view(store, project));
    }

    let history_snapshot = capture_snapshot(store);
    {
        let project = active_project_mut(store)?;
        project.name = name.clone();
        project.graph.name = name.clone();
    }
    mark_store_dirty(store);
    record_successful_mutation(store, history_snapshot);
    store.push_diagnostic("project_rename", name);
    let project = active_project(store)?;
    Ok(project_to_view(store, project))
}

/// Load a project from disk, migrate legacy schemas, and reset transient state.
pub(crate) fn project_open_path_internal(
    store: &mut AppStore,
    path: &Path,
) -> AppResult<ProjectDto> {
    let payload = load_project(path)?;
    let (graph, migrated) = deserialize_project_document(payload.as_str())?;

    store.current_project = Some(graph);
    clear_graph_history(store);
    store.reset_on_project_swap();
    store.current_path = Some(path.to_path_buf());
    mark_store_clean(store)?;
    let _ = clear_recovery_snapshot();
    if migrated {
        mark_store_dirty(store);
        store.push_diagnostic("project_open_migrated_grid", path.display().to_string());
    }
    store.push_diagnostic("project_open", path.display().to_string());
    Ok(project_dto(active_project(store)?))
}

/// Save the active project to a specific path and mark it clean.
pub(crate) fn project_save_as_internal(store: &mut AppStore, path: &Path) -> AppResult<String> {
    let graph = active_project(store)?;
    let payload = serde_json::to_string_pretty(graph)?;
    save_project(path, payload.as_str())?;
    store.current_path = Some(path.to_path_buf());
    mark_store_clean(store)?;
    let _ = clear_recovery_snapshot();
    store.push_diagnostic("project_save_as", path.display().to_string());
    Ok(format!("saved project to {}", path.display()))
}

/// Save the active project back to its existing path.
pub(crate) fn project_save_current_internal(store: &mut AppStore) -> AppResult<String> {
    let path = store
        .current_path
        .clone()
        .ok_or_else(|| AppError::InvalidInput("no save path set; use Save As".to_string()))?;
    project_save_as_internal(store, path.as_path())
}

/// Return whether the current project differs from the last saved snapshot.
pub(crate) fn project_dirty_status_internal(store: &AppStore) -> ProjectDirtyStatusDto {
    ProjectDirtyStatusDto {
        dirty: store.dirty,
        path: store
            .current_path
            .as_ref()
            .map(|path| path.display().to_string()),
    }
}

#[tauri::command]
/// Create a new project using an explicit name from the frontend.
pub fn project_create(
    state: tauri::State<'_, SharedAppState>,
    args: ProjectCreateArgs,
) -> Result<ProjectDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_new_internal(
        &mut store,
        ProjectNewArgs {
            name: Some(args.name),
        },
    )
    .map_err(|err| err.to_string())
}

#[tauri::command]
/// Create a new project, defaulting the name when omitted.
pub fn project_new(
    state: tauri::State<'_, SharedAppState>,
    args: ProjectNewArgs,
) -> Result<ProjectDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_new_internal(&mut store, args).map_err(|err| err.to_string())
}

#[tauri::command]
/// Rename the active project.
pub fn project_rename(
    state: tauri::State<'_, SharedAppState>,
    args: ProjectRenameArgs,
) -> Result<ProjectViewDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_rename_internal(&mut store, args).map_err(|err| err.to_string())
}

#[tauri::command]
/// Open a project from a concrete filesystem path.
pub fn project_open_path(
    state: tauri::State<'_, SharedAppState>,
    args: ProjectOpenPathArgs,
) -> Result<ProjectDto, String> {
    let path = resolve_path(args.path.as_str()).map_err(|err| err.to_string())?;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_open_path_internal(&mut store, path.as_path()).map_err(|err| err.to_string())
}

#[tauri::command]
/// Compatibility wrapper around [`project_open_path`].
pub fn project_open(
    state: tauri::State<'_, SharedAppState>,
    args: ProjectOpenArgs,
) -> Result<ProjectDto, String> {
    project_open_path(state, ProjectOpenPathArgs { path: args.path })
}

#[tauri::command]
/// Save the active project to its current path.
pub fn project_save_current(state: tauri::State<'_, SharedAppState>) -> Result<String, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_save_current_internal(&mut store).map_err(|err| err.to_string())
}

#[tauri::command]
/// Save the active project to a new explicit path.
pub fn project_save_as(
    state: tauri::State<'_, SharedAppState>,
    args: ProjectSaveAsArgs,
) -> Result<String, String> {
    let path = resolve_path(args.path.as_str()).map_err(|err| err.to_string())?;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_save_as_internal(&mut store, path.as_path()).map_err(|err| err.to_string())
}

#[tauri::command]
/// Save either to the provided path or to the current path when absent.
pub fn project_save(
    state: tauri::State<'_, SharedAppState>,
    args: ProjectSaveArgs,
) -> Result<String, String> {
    if let Some(path) = args.path {
        return project_save_as(state, ProjectSaveAsArgs { path });
    }
    project_save_current(state)
}

#[tauri::command]
/// Report the active project's dirty flag and current path.
pub fn project_dirty_status(
    state: tauri::State<'_, SharedAppState>,
) -> Result<ProjectDirtyStatusDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    Ok(project_dirty_status_internal(&store))
}

#[tauri::command]
/// Return UI-facing metadata about the active project.
pub fn project_snapshot(state: tauri::State<'_, SharedAppState>) -> Result<ProjectViewDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let graph = active_project(&store).map_err(|err| err.to_string())?;
    Ok(project_to_view(&store, graph))
}

#[tauri::command]
/// Ensure an active project exists for editor startup without masking unrelated errors.
pub fn project_bootstrap(
    state: tauri::State<'_, SharedAppState>,
) -> Result<ProjectViewDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_bootstrap_internal(&mut store).map_err(|err| err.to_string())
}

#[tauri::command]
/// Snapshot the init-stage editor state.
pub fn project_init_snapshot(
    state: tauri::State<'_, SharedAppState>,
) -> Result<InitStageSnapshotDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    Ok(init_stage_snapshot(project))
}

#[tauri::command]
/// Apply a batch of init-stage operations as one undoable mutation.
pub fn project_init_apply(
    state: tauri::State<'_, SharedAppState>,
    args: InitStageApplyArgs,
) -> Result<InitStageSnapshotDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let history_snapshot = capture_snapshot(&store);
    let snapshot = {
        let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
        let changed =
            apply_init_stage_ops(project, args.ops.as_slice()).map_err(|err| err.to_string())?;
        if !changed {
            return Ok(init_stage_snapshot(project));
        }
        init_stage_snapshot(project)
    };
    let clears_runtime_cache = args.ops.iter().any(|op| {
        matches!(
            op,
            InitStageOp::TrickCreate { .. }
                | InitStageOp::TrickRename { .. }
                | InitStageOp::TrickDelete { .. }
        )
    });
    let syncs_trick_caches = args.ops.iter().any(|op| {
        matches!(
            op,
            InitStageOp::TrickCreate { .. } | InitStageOp::TrickDelete { .. }
        )
    });
    if clears_runtime_cache {
        store.clear_compile_cache_for_target(&crate::model::CadenceGraphTarget::Runtime);
    }
    if syncs_trick_caches {
        let trick_ids = {
            let project = active_project(&store).map_err(|err| err.to_string())?;
            project
                .init_stage
                .tricks
                .iter()
                .map(|trick| trick.id.clone())
                .collect::<Vec<_>>()
        };
        store.retain_trick_compile_cache_ids(trick_ids);
    }
    mark_store_dirty(&mut store);
    record_successful_mutation(&mut store, history_snapshot);
    store.push_diagnostic("project_init_apply", format!("ops={}", args.ops.len()));
    Ok(snapshot)
}

#[tauri::command]
/// Open a native file picker for choosing a project to open.
pub async fn project_pick_open_path(
    state: tauri::State<'_, SharedAppState>,
) -> Result<Option<String>, String> {
    let initial_directory = {
        let store = state
            .store
            .lock()
            .map_err(|_| "app state lock poisoned".to_string())?;
        store
            .current_path
            .as_ref()
            .and_then(|path| path.parent())
            .map(PathBuf::from)
    };

    let mut dialog = AsyncFileDialog::new().add_filter("Cadence Project", &["json"]);
    if let Some(directory) = initial_directory.as_ref() {
        dialog = dialog.set_directory(directory);
    }
    Ok(dialog
        .pick_file()
        .await
        .map(|handle| handle.path().display().to_string()))
}

#[tauri::command]
/// Open a native file picker for choosing where to save the current project.
pub async fn project_pick_save_path(
    state: tauri::State<'_, SharedAppState>,
) -> Result<Option<String>, String> {
    let (initial_directory, initial_name) = {
        let store = state
            .store
            .lock()
            .map_err(|_| "app state lock poisoned".to_string())?;
        let directory = store
            .current_path
            .as_ref()
            .and_then(|path| path.parent())
            .map(PathBuf::from);
        let name = store
            .current_path
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|item| item.to_str())
            .map(ToOwned::to_owned);
        (directory, name)
    };

    let mut dialog = AsyncFileDialog::new().add_filter("Cadence Project", &["json"]);
    if let Some(directory) = initial_directory.as_ref() {
        dialog = dialog.set_directory(directory);
    }
    if let Some(name) = initial_name.as_deref() {
        dialog = dialog.set_file_name(name);
    } else {
        dialog = dialog.set_file_name("project.cadence.json");
    }
    Ok(dialog
        .save_file()
        .await
        .map(|handle| handle.path().display().to_string()))
}

#[tauri::command]
/// Report whether the current project is dirty so the frontend can drive its own prompt flow.
pub fn project_prompt_unsaved(
    state: tauri::State<'_, SharedAppState>,
) -> Result<DirtyDecision, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    if store.dirty {
        Ok(DirtyDecision::Cancel)
    } else {
        Ok(DirtyDecision::Discard)
    }
}

#[tauri::command]
/// Return the temp recovery snapshot path when one is available.
pub fn project_recovery_status(
    state: tauri::State<'_, SharedAppState>,
) -> Result<ProjectPathChoiceDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let _ = &store;
    let path = recovery_snapshot_path();
    Ok(ProjectPathChoiceDto {
        path: path.is_file().then(|| path.display().to_string()),
    })
}

#[tauri::command]
/// Persist the active project to the temp recovery snapshot.
pub fn project_recovery_write(state: tauri::State<'_, SharedAppState>) -> Result<String, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let path = write_recovery_snapshot(&store).map_err(|err| err.to_string())?;
    store.push_diagnostic("project_recovery_write", path.clone());
    Ok(format!("wrote recovery snapshot to {path}"))
}

#[tauri::command]
/// Restore the temp recovery snapshot as the active unsaved project.
pub fn project_recovery_load(
    state: tauri::State<'_, SharedAppState>,
) -> Result<ProjectDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let path = recovery_snapshot_path();
    let payload = load_project(path.as_path()).map_err(|err| err.to_string())?;
    let (graph, migrated) =
        deserialize_project_document(payload.as_str()).map_err(|err| err.to_string())?;

    store.current_project = Some(graph);
    clear_graph_history(&mut store);
    store.reset_on_project_swap();
    store.last_saved_snapshot_hash = None;
    store.dirty = true;
    if migrated {
        store.push_diagnostic("project_recovery_migrated_grid", path.display().to_string());
    }
    store.push_diagnostic("project_recovery_load", path.display().to_string());
    Ok(project_dto(
        active_project(&store).map_err(|err| err.to_string())?,
    ))
}

#[tauri::command]
/// Remove the temp recovery snapshot if it exists.
pub fn project_recovery_clear(state: tauri::State<'_, SharedAppState>) -> Result<String, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let message = clear_recovery_snapshot().map_err(|err| err.to_string())?;
    store.push_diagnostic("project_recovery_clear", message.clone());
    Ok(message)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use serde_json::Value;
    use tessera::CompileCache;
    use tessera::compiler::CompileMode;
    use uuid::Uuid;

    use super::*;
    use crate::commands::TerminalStrategy;
    use crate::core::host_adapter::runtime_engine;
    use crate::model::{CadenceInitStage, CadenceTrickDef};
    use crate::store::history;

    fn temp_project_path(label: &str) -> PathBuf {
        let file = format!("cadence-{label}-{}.json", Uuid::new_v4());
        std::env::temp_dir().join(file)
    }

    #[test]
    fn default_project_graph_uses_cadence_v3_shape() {
        let project = default_project_graph("demo");
        assert_eq!(
            project.schema_version,
            CadenceProjectDocument::SCHEMA_VERSION
        );
        assert!(project.init_stage.cps_expr.is_none());
        assert!(project.init_stage.sample_loads.is_empty());
        assert!(project.init_stage.tricks.is_empty());
        assert!(
            project
                .graph
                .nodes
                .contains_key(&GridPos { col: 0, row: 0 })
        );
        assert!(
            project
                .graph
                .nodes
                .contains_key(&GridPos { col: 1, row: 0 })
        );
        let source = project
            .graph
            .nodes
            .get(&GridPos { col: 0, row: 0 })
            .unwrap();
        let terminal = project
            .graph
            .nodes
            .get(&GridPos { col: 1, row: 0 })
            .unwrap();
        assert_eq!(project.graph.cols, STANDARD_GRAPH_COLS);
        assert_eq!(project.graph.rows, STANDARD_GRAPH_ROWS);
        assert_eq!(source.piece_id, "strudel.note");
        assert_eq!(terminal.piece_id, "strudel.output");
        assert_eq!(project.graph.edges.len(), 1);
    }

    #[test]
    fn project_rename_updates_graph_name_and_history() {
        let mut store = AppStore::default();
        project_new_internal(
            &mut store,
            ProjectNewArgs {
                name: Some("Alpha".to_string()),
            },
        )
        .expect("seed project");

        let renamed = project_rename_internal(
            &mut store,
            ProjectRenameArgs {
                name: "Beta".to_string(),
            },
        )
        .expect("rename project");
        assert_eq!(renamed.name, "Beta");
        assert!(store.dirty);
        let project = store.current_project.as_ref().expect("project");
        assert_eq!(project.name, "Beta");
        assert_eq!(project.graph.name, "Beta");

        history::undo(&mut store).expect("undo rename");
        assert_eq!(
            store.current_project.as_ref().expect("project").name,
            "Alpha"
        );
        history::redo(&mut store).expect("redo rename");
        assert_eq!(
            store.current_project.as_ref().expect("project").name,
            "Beta"
        );
    }

    #[test]
    fn project_new_clears_runtime_and_trick_compile_caches() {
        let mut store = AppStore::default();
        project_new_internal(
            &mut store,
            ProjectNewArgs {
                name: Some("Alpha".to_string()),
            },
        )
        .expect("seed project");

        let runtime_graph = store
            .current_project
            .as_ref()
            .expect("project")
            .graph
            .clone();
        let engine = runtime_engine(
            store.current_project.as_ref().expect("project"),
            TerminalStrategy::Stack,
        );
        engine
            .compile_cached(
                &runtime_graph,
                CompileMode::Preview,
                &mut store.runtime_compile_cache,
            )
            .expect("prime runtime cache");
        store
            .trick_compile_caches
            .insert("ghost".to_string(), CompileCache::new());

        assert!(!store.runtime_compile_cache.is_empty());
        assert_eq!(store.trick_compile_caches.len(), 1);

        project_new_internal(
            &mut store,
            ProjectNewArgs {
                name: Some("Beta".to_string()),
            },
        )
        .expect("replace project");

        assert!(store.runtime_compile_cache.is_empty());
        assert!(store.trick_compile_caches.is_empty());
    }

    #[test]
    fn trick_cache_retention_drops_deleted_tricks() {
        let mut store = AppStore::default();
        project_new_internal(
            &mut store,
            ProjectNewArgs {
                name: Some("Alpha".to_string()),
            },
        )
        .expect("seed project");

        {
            let project = active_project_mut(&mut store).expect("project");
            apply_init_stage_ops(
                project,
                &[
                    InitStageOp::TrickCreate {
                        id: "keep".to_string(),
                        name: "Keep".to_string(),
                        graph: None,
                    },
                    InitStageOp::TrickCreate {
                        id: "drop".to_string(),
                        name: "Drop".to_string(),
                        graph: None,
                    },
                ],
            )
            .expect("create tricks");
        }

        store
            .trick_compile_caches
            .insert("keep".to_string(), CompileCache::new());
        store
            .trick_compile_caches
            .insert("drop".to_string(), CompileCache::new());
        store
            .trick_compile_caches
            .insert("ghost".to_string(), CompileCache::new());

        {
            let project = active_project_mut(&mut store).expect("project");
            apply_init_stage_ops(
                project,
                &[InitStageOp::TrickDelete {
                    id: "drop".to_string(),
                }],
            )
            .expect("delete trick");
        }

        let trick_ids = store
            .current_project
            .as_ref()
            .expect("project")
            .init_stage
            .tricks
            .iter()
            .map(|trick| trick.id.clone())
            .collect::<Vec<_>>();
        store.retain_trick_compile_cache_ids(trick_ids);

        assert!(store.trick_compile_caches.contains_key("keep"));
        assert!(!store.trick_compile_caches.contains_key("drop"));
        assert!(!store.trick_compile_caches.contains_key("ghost"));
    }

    #[test]
    fn project_open_rejects_missing_schema_version() {
        let path = temp_project_path("missing-schema");
        fs::write(path.as_path(), r#"{"name":"legacy"}"#).expect("write temp project");

        let mut store = AppStore::default();
        let err = project_open_path_internal(&mut store, path.as_path())
            .expect_err("missing schema_version must fail");
        let message = err.to_string();
        assert!(message.contains("missing schema_version"));

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn project_open_rejects_unsupported_schema_version() {
        let path = temp_project_path("unsupported-schema");
        fs::write(path.as_path(), r#"{"schema_version":1}"#).expect("write temp project");

        let mut store = AppStore::default();
        let err = project_open_path_internal(&mut store, path.as_path())
            .expect_err("unsupported schema_version must fail");
        let message = err.to_string();
        assert!(message.contains("unsupported schema_version"));
        assert!(message.contains("1"));

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn project_open_compacts_graph_to_top_left_and_marks_dirty() {
        let path = temp_project_path("compact-runtime-grid");
        let edge = Edge {
            id: EdgeId::new(),
            from: GridPos { col: 4, row: 3 },
            to_node: GridPos { col: 5, row: 3 },
            to_param: "pattern".to_string(),
        };
        let project = ProjectDocument::new(
            "offset-grid".to_string(),
            Graph {
                nodes: BTreeMap::from([
                    (
                        GridPos { col: 4, row: 3 },
                        Node {
                            piece_id: "strudel.note".to_string(),
                            inline_params: BTreeMap::from([(
                                "value".to_string(),
                                Value::String("c3".to_string()),
                            )]),
                            input_sides: BTreeMap::new(),
                            output_side: None,
                            label: None,
                            node_state: None,
                        },
                    ),
                    (
                        GridPos { col: 5, row: 3 },
                        Node {
                            piece_id: "strudel.output".to_string(),
                            inline_params: BTreeMap::new(),
                            input_sides: BTreeMap::new(),
                            output_side: None,
                            label: None,
                            node_state: None,
                        },
                    ),
                ]),
                edges: BTreeMap::from([(edge.id.clone(), edge)]),
                name: "offset-grid".to_string(),
                cols: 9,
                rows: 9,
            },
        );
        let payload = serde_json::to_string_pretty(&project).expect("serialize project");
        fs::write(path.as_path(), payload).expect("write temp project");

        let mut store = AppStore::default();
        let dto = project_open_path_internal(&mut store, path.as_path()).expect("open compacted");
        assert_eq!(dto.name, "offset-grid");
        assert!(store.dirty);

        let opened = store.current_project.as_ref().expect("project");
        assert_eq!(opened.graph.cols, STANDARD_GRAPH_COLS);
        assert_eq!(opened.graph.rows, STANDARD_GRAPH_ROWS);
        assert!(opened.graph.nodes.contains_key(&GridPos { col: 0, row: 0 }));
        assert!(opened.graph.nodes.contains_key(&GridPos { col: 1, row: 0 }));
        let opened_edge = opened.graph.edges.values().next().expect("edge");
        assert_eq!(opened_edge.from, GridPos { col: 0, row: 0 });
        assert_eq!(opened_edge.to_node, GridPos { col: 1, row: 0 });

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn project_open_migrates_v2_document_with_empty_init_stage() {
        let path = temp_project_path("migrate-v2");
        let project = ProjectDocument::new(
            "legacy".to_string(),
            Graph {
                nodes: BTreeMap::new(),
                edges: BTreeMap::new(),
                name: "legacy".to_string(),
                cols: 9,
                rows: 9,
            },
        );
        let payload = serde_json::to_string_pretty(&project).expect("serialize legacy project");
        fs::write(path.as_path(), payload).expect("write temp project");

        let mut store = AppStore::default();
        let dto = project_open_path_internal(&mut store, path.as_path()).expect("open migrated");
        assert_eq!(dto.name, "legacy");
        assert!(store.dirty);
        let opened = store.current_project.as_ref().expect("project");
        assert_eq!(
            opened.schema_version,
            CadenceProjectDocument::SCHEMA_VERSION
        );
        assert!(opened.init_stage.sample_loads.is_empty());
        assert!(opened.init_stage.tricks.is_empty());
        assert_eq!(opened.graph.cols, STANDARD_GRAPH_COLS);
        assert_eq!(opened.graph.rows, STANDARD_GRAPH_ROWS);

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn project_open_preserves_large_runtime_bounds_and_compacts_trick_graphs() {
        let path = temp_project_path("compact-trick-grid");
        let edge = Edge {
            id: EdgeId::new(),
            from: GridPos { col: 2, row: 1 },
            to_node: GridPos { col: 15, row: 9 },
            to_param: "pattern".to_string(),
        };
        let project = CadenceProjectDocument {
            schema_version: CadenceProjectDocument::SCHEMA_VERSION,
            name: "hybrid".to_string(),
            graph: Graph {
                nodes: BTreeMap::from([
                    (
                        GridPos { col: 2, row: 1 },
                        Node {
                            piece_id: "strudel.note".to_string(),
                            inline_params: BTreeMap::from([(
                                "value".to_string(),
                                Value::String("c3".to_string()),
                            )]),
                            input_sides: BTreeMap::new(),
                            output_side: None,
                            label: None,
                            node_state: None,
                        },
                    ),
                    (
                        GridPos { col: 15, row: 9 },
                        Node {
                            piece_id: "strudel.output".to_string(),
                            inline_params: BTreeMap::new(),
                            input_sides: BTreeMap::new(),
                            output_side: None,
                            label: None,
                            node_state: None,
                        },
                    ),
                ]),
                edges: BTreeMap::from([(edge.id.clone(), edge)]),
                name: "hybrid".to_string(),
                cols: 18,
                rows: 12,
            },
            init_stage: CadenceInitStage {
                cps_expr: None,
                sample_loads: vec![],
                tricks: vec![CadenceTrickDef {
                    id: "melodia".to_string(),
                    name: "melodia".to_string(),
                    graph: Graph {
                        nodes: BTreeMap::from([(
                            GridPos { col: 3, row: 2 },
                            Node {
                                piece_id: "cadence.trick_input_1".to_string(),
                                inline_params: BTreeMap::new(),
                                input_sides: BTreeMap::new(),
                                output_side: None,
                                label: None,
                                node_state: None,
                            },
                        )]),
                        edges: BTreeMap::new(),
                        name: "melodia".to_string(),
                        cols: 9,
                        rows: 9,
                    },
                }],
            },
        };
        let payload = serde_json::to_string_pretty(&project).expect("serialize project");
        fs::write(path.as_path(), payload).expect("write temp project");

        let mut store = AppStore::default();
        project_open_path_internal(&mut store, path.as_path()).expect("open hybrid");
        assert!(store.dirty);

        let opened = store.current_project.as_ref().expect("project");
        assert_eq!(opened.graph.cols, 14);
        assert_eq!(opened.graph.rows, 9);
        assert!(opened.graph.nodes.contains_key(&GridPos { col: 0, row: 0 }));
        assert!(
            opened
                .graph
                .nodes
                .contains_key(&GridPos { col: 13, row: 8 })
        );
        let trick = opened
            .init_stage
            .tricks
            .iter()
            .find(|item| item.id == "melodia")
            .expect("trick");
        assert_eq!(trick.graph.cols, STANDARD_GRAPH_COLS);
        assert_eq!(trick.graph.rows, STANDARD_GRAPH_ROWS);
        assert!(trick.graph.nodes.contains_key(&GridPos { col: 0, row: 0 }));

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn project_open_keeps_already_compact_graph_clean() {
        let path = temp_project_path("compact-clean");
        let project = ProjectDocument::new(
            "clean".to_string(),
            Graph {
                nodes: BTreeMap::from([
                    (
                        GridPos { col: 0, row: 0 },
                        Node {
                            piece_id: "strudel.note".to_string(),
                            inline_params: BTreeMap::from([(
                                "value".to_string(),
                                Value::String("c3".to_string()),
                            )]),
                            input_sides: BTreeMap::new(),
                            output_side: None,
                            label: None,
                            node_state: None,
                        },
                    ),
                    (
                        GridPos { col: 1, row: 0 },
                        Node {
                            piece_id: "strudel.output".to_string(),
                            inline_params: BTreeMap::new(),
                            input_sides: BTreeMap::new(),
                            output_side: None,
                            label: None,
                            node_state: None,
                        },
                    ),
                ]),
                edges: BTreeMap::new(),
                name: "clean".to_string(),
                cols: STANDARD_GRAPH_COLS,
                rows: STANDARD_GRAPH_ROWS,
            },
        );
        let payload = serde_json::to_string_pretty(&project).expect("serialize project");
        fs::write(path.as_path(), payload).expect("write temp project");

        let mut store = AppStore::default();
        project_open_path_internal(&mut store, path.as_path()).expect("open clean");
        assert!(!store.dirty);

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn project_bootstrap_creates_default_project_when_missing() {
        let mut store = AppStore::default();

        let view = project_bootstrap_internal(&mut store).expect("bootstrap project");

        assert_eq!(view.name, "Untitled");
        assert!(store.current_project.is_some());
        assert!(!store.dirty);
    }

    #[test]
    fn project_bootstrap_preserves_existing_project() {
        let mut store = AppStore::default();
        project_new_internal(
            &mut store,
            ProjectNewArgs {
                name: Some("Existing".to_string()),
            },
        )
        .expect("new project");

        let view = project_bootstrap_internal(&mut store).expect("bootstrap existing");

        assert_eq!(view.name, "Existing");
        assert_eq!(
            store.current_project.as_ref().expect("project").name,
            "Existing"
        );
    }
}
