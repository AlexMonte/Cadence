//! Backend helpers for project lifecycle, persistence, and init-stage editing.

#[path = "support/document.rs"]
mod document;
#[path = "support/init_stage.rs"]
mod init_stage;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use self::document::*;
use self::init_stage::*;
#[cfg(not(target_arch = "wasm32"))]
use crate::adapter::samples::bundled_sample_root;
#[cfg(target_arch = "wasm32")]
use crate::adapter::samples::bundled_web_sample_urls;
use crate::adapter::samples::{PathSampleCatalog, SampleCatalogEntry};
use crate::{
    adapter::storage::{
        WorkspaceProjectSnapshot, clear_recovery_snapshot, current_workspace_project, file_name,
        load_project, load_recovery_snapshot, parent_location, pick_open_path, pick_save_path,
        recovery_snapshot_location, recovery_snapshot_status, save_project,
        workspace_project_location, write_recovery_snapshot, write_workspace_project,
    },
    application::{
        error::{AppError, AppResult},
        history::{capture_snapshot, clear_graph_history, record_successful_mutation},
        state::AppStore,
    },
    domain::project::{CadenceGraphTarget, CadenceProjectDocument, InitStageOp, InitStageSnapshot},
    infrastructure::dto::SharedAppState,
};
#[cfg(test)]
use tessera::graph::Node;
use tessera::graph::{Edge, Graph};
#[cfg(test)]
use tessera::types::TileSide;
use tessera::types::{EdgeId, GridPos};

const STANDARD_GRAPH_COLS: u32 = 12;
const STANDARD_GRAPH_ROWS: u32 = 8;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Optional path wrapper used by file-picker and recovery-related commands.
pub struct ProjectPathChoiceDto {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SampleLibrarySourceKind {
    Unavailable,
    ProjectDirectory,
    EnvDirectory,
    BundledDirectory,
    BundledWeb,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SampleLibraryEntryDto {
    pub key: String,
    pub library: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SampleLibrarySnapshotDto {
    pub available: bool,
    pub source_kind: SampleLibrarySourceKind,
    pub source_label: String,
    pub source_path: Option<String>,
    #[serde(default)]
    pub entries: Vec<SampleLibraryEntryDto>,
    pub error: Option<String>,
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

fn persist_workspace_session(store: &AppStore) -> AppResult<()> {
    let project = active_project(store)?;
    let payload = serde_json::to_string_pretty(project)?;
    write_workspace_project(&WorkspaceProjectSnapshot {
        location: store.current_path.clone(),
        payload,
    })
    .map_err(AppError::InvalidInput)
}

fn normalize_workspace_location_for_open(
    location: &str,
    project: &CadenceProjectDocument,
) -> Option<String> {
    if location.contains("://") {
        return workspace_project_location(project.name.as_str());
    }
    Some(location.to_string())
}

fn normalize_loaded_project(project: &mut CadenceProjectDocument) -> bool {
    let before = serde_json::to_string(project).ok();
    crate::adapter::tessera::host_adapter::normalize_project_piece_sides(project);
    serde_json::to_string(project).ok() != before
}

pub(crate) fn project_bootstrap_internal(store: &mut AppStore) -> AppResult<ProjectViewDto> {
    if store.current_project.is_none() {
        if let Some(snapshot) = current_workspace_project().map_err(AppError::InvalidInput)? {
            let (mut graph, normalized) = deserialize_project_document(snapshot.payload.as_str())?;
            let normalized = normalize_loaded_project(&mut graph) | normalized;
            store.current_project = Some(graph);
            clear_graph_history(store);
            store.reset_on_project_swap();
            store.current_path = snapshot.location.clone();
            mark_store_clean(store)?;
            if normalized {
                mark_store_dirty(store);
                persist_workspace_session(store)?;
                store.push_diagnostic(
                    "project_bootstrap_normalized",
                    store
                        .current_path
                        .clone()
                        .unwrap_or_else(|| "workspace".to_string()),
                );
            }
            store.push_diagnostic(
                "project_bootstrap",
                format!(
                    "loaded {}",
                    store
                        .current_path
                        .clone()
                        .unwrap_or_else(|| "workspace session".to_string())
                ),
            );
        } else {
            project_new_internal(store, Some("Untitled".to_string()))?;
            store.push_diagnostic("project_bootstrap", "created Untitled project".to_string());
        }
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
    name: Option<String>,
) -> AppResult<ProjectDto> {
    let name = validate_project_name(name.as_deref().unwrap_or("Untitled"))?;
    let history_snapshot = capture_snapshot(store);
    store.current_project = Some(default_project_graph(name.as_str()));
    clear_graph_history(store);
    store.reset_on_project_swap();
    mark_store_clean(store)?;
    persist_workspace_session(store)?;
    let _ = clear_recovery_snapshot();
    record_successful_mutation(store, history_snapshot);
    store.push_diagnostic("project_new", name.clone());
    Ok(project_dto(active_project(store)?))
}

/// Rename the active project and keep its runtime graph name in sync.
pub(crate) fn project_rename_internal(
    store: &mut AppStore,
    name: String,
) -> AppResult<ProjectViewDto> {
    let name = validate_project_name(name.as_str())?;
    let current_name = active_project(store)?.name.clone();
    if current_name == name {
        let project = active_project(store)?;
        return Ok(project_to_view(store, project));
    }

    let history_snapshot = capture_snapshot(store);
    {
        let project = active_project_mut(store)?;
        project.name = name.clone();
        project.runtime_graph_mut().name = name.clone();
    }
    mark_store_dirty(store);
    record_successful_mutation(store, history_snapshot);
    store.push_diagnostic("project_rename", name);
    let project = active_project(store)?;
    Ok(project_to_view(store, project))
}

/// Load a project from a platform-defined location, normalize the graph workspace, and reset transient
/// state.
pub(crate) fn project_open_path_internal(
    store: &mut AppStore,
    location: &str,
) -> AppResult<ProjectDto> {
    let payload = load_project(location).map_err(AppError::InvalidInput)?;
    let (mut graph, normalized) = deserialize_project_document(payload.as_str())?;
    let normalized = normalize_loaded_project(&mut graph) | normalized;

    store.current_project = Some(graph);
    clear_graph_history(store);
    store.reset_on_project_swap();
    store.current_path = normalize_workspace_location_for_open(location, active_project(store)?);
    mark_store_clean(store)?;
    persist_workspace_session(store)?;
    let _ = clear_recovery_snapshot();
    if normalized {
        mark_store_dirty(store);
        store.push_diagnostic("project_open_normalized_grid", location.to_string());
    }
    store.push_diagnostic("project_open", location.to_string());
    Ok(project_dto(active_project(store)?))
}

/// Save the active project to a specific path and mark it clean.
pub(crate) fn project_save_as_internal(store: &mut AppStore, location: &str) -> AppResult<String> {
    let graph = active_project(store)?;
    let payload = serde_json::to_string_pretty(graph)?;
    let resolved_location =
        save_project(location, payload.as_str()).map_err(AppError::InvalidInput)?;
    store.current_path = Some(resolved_location.clone());
    mark_store_clean(store)?;
    persist_workspace_session(store)?;
    let _ = clear_recovery_snapshot();
    store.push_diagnostic("project_save_as", resolved_location.clone());
    Ok(format!("saved project to {resolved_location}"))
}

/// Save the active project back to its existing path.
pub(crate) fn project_save_current_internal(store: &mut AppStore) -> AppResult<String> {
    let location = store
        .current_path
        .clone()
        .ok_or_else(|| AppError::InvalidInput("no save path set; use Save As".to_string()))?;
    project_save_as_internal(store, location.as_str())
}

/// Return whether the current project differs from the last saved snapshot.
pub(crate) fn project_dirty_status_internal(store: &AppStore) -> ProjectDirtyStatusDto {
    ProjectDirtyStatusDto {
        dirty: store.dirty,
        path: store.current_path.clone(),
    }
}

/// Create a new project using an explicit name from the frontend.
pub fn project_create(state: &SharedAppState, name: String) -> Result<ProjectDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_new_internal(&mut store, Some(name)).map_err(|err| err.to_string())
}

/// Create a new project, defaulting the name when omitted.
pub fn project_new(state: &SharedAppState, name: Option<String>) -> Result<ProjectDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_new_internal(&mut store, name).map_err(|err| err.to_string())
}

/// Rename the active project.
pub fn project_rename(state: &SharedAppState, name: String) -> Result<ProjectViewDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_rename_internal(&mut store, name).map_err(|err| err.to_string())
}

/// Open a project from a concrete filesystem path.
pub fn project_open_path(state: &SharedAppState, path: String) -> Result<ProjectDto, String> {
    let path = resolve_path(path.as_str()).map_err(|err| err.to_string())?;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_open_path_internal(&mut store, path.as_str()).map_err(|err| err.to_string())
}

/// Compatibility wrapper around [`project_open_path`].
pub fn project_open(state: &SharedAppState, path: String) -> Result<ProjectDto, String> {
    project_open_path(state, path)
}

/// Save the active project to its current path.
pub fn project_save_current(state: &SharedAppState) -> Result<String, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_save_current_internal(&mut store).map_err(|err| err.to_string())
}

/// Save the active project to a new explicit path.
pub fn project_save_as(state: &SharedAppState, path: String) -> Result<String, String> {
    let path = resolve_path(path.as_str()).map_err(|err| err.to_string())?;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_save_as_internal(&mut store, path.as_str()).map_err(|err| err.to_string())
}

/// Save either to the provided path or to the current path when absent.
pub fn project_save(state: &SharedAppState, path: Option<String>) -> Result<String, String> {
    if let Some(path) = path {
        return project_save_as(state, path);
    }
    project_save_current(state)
}

/// Report the active project's dirty flag and current path.
pub fn project_dirty_status(state: &SharedAppState) -> Result<ProjectDirtyStatusDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    Ok(project_dirty_status_internal(&store))
}

/// Return UI-facing metadata about the active project.
pub fn project_snapshot(state: &SharedAppState) -> Result<ProjectViewDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let graph = active_project(&store).map_err(|err| err.to_string())?;
    Ok(project_to_view(&store, graph))
}

/// Ensure an active project exists for editor startup without masking unrelated errors.
pub fn project_bootstrap(state: &SharedAppState) -> Result<ProjectViewDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_bootstrap_internal(&mut store).map_err(|err| err.to_string())
}

/// Snapshot the init-stage editor state.
pub fn project_init_snapshot(state: &SharedAppState) -> Result<InitStageSnapshot, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    Ok(init_stage_snapshot(project))
}

/// Snapshot the currently discoverable sample library for Setup.
pub fn project_sample_library(state: &SharedAppState) -> Result<SampleLibrarySnapshotDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    Ok(sample_library_snapshot(&store))
}

fn sample_library_entries(catalog: &PathSampleCatalog) -> Vec<SampleLibraryEntryDto> {
    catalog
        .entries()
        .into_iter()
        .map(sample_library_entry)
        .collect()
}

fn sample_library_entry(entry: SampleCatalogEntry) -> SampleLibraryEntryDto {
    SampleLibraryEntryDto {
        key: entry.key,
        library: entry.library,
        tags: entry.tags,
    }
}

fn unavailable_sample_library_snapshot(
    source_kind: SampleLibrarySourceKind,
    source_label: impl Into<String>,
    source_path: Option<String>,
    error: Option<String>,
) -> SampleLibrarySnapshotDto {
    SampleLibrarySnapshotDto {
        available: false,
        source_kind,
        source_label: source_label.into(),
        source_path,
        entries: Vec::new(),
        error,
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
struct SampleLibrarySource {
    kind: SampleLibrarySourceKind,
    label: &'static str,
    path: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
fn sample_library_snapshot(store: &AppStore) -> SampleLibrarySnapshotDto {
    let Some(source) = resolve_sample_library_source(store) else {
        return unavailable_sample_library_snapshot(
            SampleLibrarySourceKind::Unavailable,
            "Sample library unavailable",
            None,
            Some(
                "No sample directory is configured. Set CADENCE_SAMPLE_ROOT or open a saved project with a sibling `samples/` directory."
                    .to_string(),
            ),
        );
    };

    match PathSampleCatalog::load_directory(&source.path) {
        Ok(catalog) => SampleLibrarySnapshotDto {
            available: true,
            source_kind: source.kind,
            source_label: source.label.to_string(),
            source_path: Some(source.path.display().to_string()),
            entries: sample_library_entries(&catalog),
            error: None,
        },
        Err(error) => unavailable_sample_library_snapshot(
            source.kind,
            source.label,
            Some(source.path.display().to_string()),
            Some(error.to_string()),
        ),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_sample_library_source(store: &AppStore) -> Option<SampleLibrarySource> {
    if let Some(path) = std::env::var_os("CADENCE_SAMPLE_ROOT") {
        return Some(SampleLibrarySource {
            kind: SampleLibrarySourceKind::EnvDirectory,
            label: "CADENCE_SAMPLE_ROOT",
            path: PathBuf::from(path),
        });
    }

    if let Some(path) = store
        .current_path
        .as_deref()
        .filter(|path| !path.contains("://"))
        .and_then(|path| Path::new(path).parent())
        .map(|parent| parent.join("samples"))
    {
        return Some(SampleLibrarySource {
            kind: SampleLibrarySourceKind::ProjectDirectory,
            label: "Project samples directory",
            path,
        });
    }

    bundled_sample_root().map(|path| SampleLibrarySource {
        kind: SampleLibrarySourceKind::BundledDirectory,
        label: "Bundled Cadence samples",
        path,
    })
}

#[cfg(target_arch = "wasm32")]
fn sample_library_snapshot(_store: &AppStore) -> SampleLibrarySnapshotDto {
    let samples = bundled_web_sample_urls()
        .into_iter()
        .map(|(key, url)| (key, std::path::PathBuf::from(url)))
        .collect();
    let catalog = PathSampleCatalog::new(samples);

    SampleLibrarySnapshotDto {
        available: true,
        source_kind: SampleLibrarySourceKind::BundledWeb,
        source_label: "Bundled web sample library".to_string(),
        source_path: Some("/assets/generated_samples".to_string()),
        entries: sample_library_entries(&catalog),
        error: None,
    }
}

/// Apply a batch of init-stage operations as one undoable mutation.
pub fn project_init_apply(
    state: &SharedAppState,
    ops: Vec<InitStageOp>,
) -> Result<InitStageSnapshot, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let history_snapshot = capture_snapshot(&store);
    let snapshot = {
        let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
        let changed =
            apply_init_stage_ops(project, ops.as_slice()).map_err(|err| err.to_string())?;
        if !changed {
            return Ok(init_stage_snapshot(project));
        }
        init_stage_snapshot(project)
    };
    let clears_runtime_cache = ops.iter().any(|op| {
        matches!(
            op,
            InitStageOp::TrickCreate { .. }
                | InitStageOp::TrickRename { .. }
                | InitStageOp::TrickDelete { .. }
        )
    });
    let syncs_trick_caches = ops.iter().any(|op| {
        matches!(
            op,
            InitStageOp::TrickCreate { .. } | InitStageOp::TrickDelete { .. }
        )
    });
    if clears_runtime_cache {
        store.clear_compile_cache_for_target(&CadenceGraphTarget::Runtime);
    }
    if syncs_trick_caches {
        let trick_ids = {
            let project = active_project(&store).map_err(|err| err.to_string())?;
            project
                .tricks()
                .iter()
                .map(|trick| trick.id.clone())
                .collect::<Vec<_>>()
        };
        store.retain_trick_compile_cache_ids(trick_ids);
    }
    mark_store_dirty(&mut store);
    record_successful_mutation(&mut store, history_snapshot);
    store.push_diagnostic("project_init_apply", format!("ops={}", ops.len()));
    Ok(snapshot)
}

/// Open a platform file picker for choosing a project to open.
pub async fn project_pick_open_path(state: &SharedAppState) -> Result<Option<String>, String> {
    let initial_directory = {
        let store = state
            .store
            .lock()
            .map_err(|_| "app state lock poisoned".to_string())?;
        store
            .current_path
            .as_ref()
            .and_then(|path| parent_location(path.as_str()))
    };
    pick_open_path(initial_directory.as_deref()).await
}

/// Open a platform file picker for choosing where to save the current project.
pub async fn project_pick_save_path(state: &SharedAppState) -> Result<Option<String>, String> {
    let (initial_directory, initial_name) = {
        let store = state
            .store
            .lock()
            .map_err(|_| "app state lock poisoned".to_string())?;
        let directory = store
            .current_path
            .as_ref()
            .and_then(|path| parent_location(path.as_str()));
        let name = store
            .current_path
            .as_ref()
            .and_then(|path| file_name(path.as_str()));
        (directory, name)
    };
    pick_save_path(
        initial_directory.as_deref(),
        initial_name.as_deref(),
        "project.cadence.json",
    )
    .await
}

/// Return the temp recovery snapshot path when one is available.
pub fn project_recovery_status(state: &SharedAppState) -> Result<ProjectPathChoiceDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let _ = &store;
    Ok(ProjectPathChoiceDto {
        path: recovery_snapshot_status()?,
    })
}

/// Persist the active project to the temp recovery snapshot.
pub fn project_recovery_write(state: &SharedAppState) -> Result<String, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    let payload = serde_json::to_string_pretty(project).map_err(|err| err.to_string())?;
    let path = write_recovery_snapshot(payload.as_str())?;
    store.push_diagnostic("project_recovery_write", path.clone());
    Ok(format!("wrote recovery snapshot to {path}"))
}

/// Restore the temp recovery snapshot as the active unsaved project.
pub fn project_recovery_load(state: &SharedAppState) -> Result<ProjectDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let path = recovery_snapshot_location();
    let payload = load_recovery_snapshot()?;
    let (mut graph, normalized) =
        deserialize_project_document(payload.as_str()).map_err(|err| err.to_string())?;
    let normalized = normalize_loaded_project(&mut graph) | normalized;

    store.current_project = Some(graph);
    clear_graph_history(&mut store);
    store.reset_on_project_swap();
    store.last_saved_snapshot_hash = None;
    store.dirty = true;
    persist_workspace_session(&store).map_err(|err| err.to_string())?;
    if normalized {
        store.push_diagnostic("project_recovery_normalized_grid", path.clone());
    }
    store.push_diagnostic("project_recovery_load", path.clone());
    Ok(project_dto(
        active_project(&store).map_err(|err| err.to_string())?,
    ))
}

/// Remove the temp recovery snapshot if it exists.
pub fn project_recovery_clear(state: &SharedAppState) -> Result<String, String> {
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
    use std::path::PathBuf;

    use serde_json::Value;
    use tessera::analysis::AnalysisCache;
    use uuid::Uuid;

    use super::*;
    use crate::adapter::tessera::host_adapter::runtime_engine;
    use crate::application::authoring::compile::compile_project;
    use crate::application::history;
    use tessera::diagnostics::DiagnosticKind;

    #[derive(Debug, Serialize)]
    struct LegacyV2ProjectDocument {
        schema_version: u32,
        name: String,
        graph: Graph,
    }

    fn temp_project_path(label: &str) -> PathBuf {
        let file = format!("cadence-{label}-{}.json", Uuid::new_v4());
        std::env::temp_dir().join(file)
    }

    fn fixture_project_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures")
            .join("projects")
            .join(name)
    }

    #[test]
    fn default_project_graph_uses_cadence_v4_workspace_shape() {
        let project = default_project_graph("demo");
        assert_eq!(
            project.schema_version,
            CadenceProjectDocument::SCHEMA_VERSION
        );
        assert!(project.init_stage.cps_expr.is_none());
        assert_eq!(project.init_stage.sample_loads.len(), 1);
        assert_eq!(project.init_stage.sample_loads[0].id, "bd");
        assert_eq!(
            project.init_stage.sample_loads[0].source,
            crate::adapter::samples::DEFAULT_KICK_SELECTOR
        );
        assert!(project.tricks().is_empty());
        assert!(
            project
                .runtime_graph()
                .nodes
                .contains_key(&GridPos { col: 0, row: 0 })
        );
        assert!(
            project
                .runtime_graph()
                .nodes
                .contains_key(&GridPos { col: 1, row: 0 })
        );
        let source = project
            .runtime_graph()
            .nodes
            .get(&GridPos { col: 0, row: 0 })
            .unwrap();
        let terminal = project
            .runtime_graph()
            .nodes
            .get(&GridPos { col: 1, row: 0 })
            .unwrap();
        assert_eq!(project.runtime_graph().cols, STANDARD_GRAPH_COLS);
        assert_eq!(project.runtime_graph().rows, STANDARD_GRAPH_ROWS);
        assert_eq!(source.piece_id, "cadence.sound");
        assert_eq!(terminal.piece_id, "cadence.output");
        assert_eq!(project.runtime_graph().edges.len(), 1);
    }

    #[test]
    fn tessera_core_smoke_demo_fixture_opens_and_compiles() {
        let path = fixture_project_path("tessera-core-smoke-demo.cadence.json");
        let mut store = AppStore::default();

        let opened = project_open_path_internal(&mut store, path.to_string_lossy().as_ref())
            .expect("fixture should open");

        assert_eq!(opened.name, "Tessera Core Smoke Demo");
        let project = active_project(&store).expect("active project");
        assert_eq!(project.tricks().len(), 1);
        assert_eq!(project.init_stage.sample_loads.len(), 1);

        let compiled = compile_project(project);
        assert!(compiled.can_render, "fixture should render");
        assert!(
            compiled.can_play,
            "fixture should be playable: selectors={:?} outputs={} diagnostics={:?}",
            compiled.sample_selectors, compiled.output_count, compiled.diagnostics
        );
        assert_eq!(compiled.output_count, 4);
        assert_eq!(
            compiled.sample_selectors,
            vec!["bd".to_string(), "ghost".to_string(), "kick".to_string()]
        );
        assert!(
            compiled
                .preview
                .debug_text
                .as_deref()
                .is_some_and(|text| text.contains("trick pulse(pattern)"))
        );
    }

    #[test]
    fn project_rename_updates_graph_name_and_history() {
        let mut store = AppStore::default();
        project_new_internal(&mut store, Some("Alpha".to_string())).expect("seed project");

        let renamed =
            project_rename_internal(&mut store, "Beta".to_string()).expect("rename project");
        assert_eq!(renamed.name, "Beta");
        assert!(store.dirty);
        let project = store.current_project.as_ref().expect("project");
        assert_eq!(project.name, "Beta");
        assert_eq!(project.runtime_graph().name, "Beta");

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
        project_new_internal(&mut store, Some("Alpha".to_string())).expect("seed project");

        let runtime_graph = store
            .current_project
            .as_ref()
            .expect("project")
            .runtime_graph()
            .clone();
        let engine = runtime_engine(store.current_project.as_ref().expect("project"));
        let _ = engine.analyze_cached(&runtime_graph, &mut store.runtime_compile_cache);
        store
            .trick_compile_caches
            .insert("ghost".to_string(), AnalysisCache::new());

        assert!(!store.runtime_compile_cache.is_empty());
        assert_eq!(store.trick_compile_caches.len(), 1);

        project_new_internal(&mut store, Some("Beta".to_string())).expect("replace project");

        assert!(store.runtime_compile_cache.is_empty());
        assert!(store.trick_compile_caches.is_empty());
    }

    #[test]
    fn trick_cache_retention_drops_deleted_tricks() {
        let mut store = AppStore::default();
        project_new_internal(&mut store, Some("Alpha".to_string())).expect("seed project");

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
            .insert("keep".to_string(), AnalysisCache::new());
        store
            .trick_compile_caches
            .insert("drop".to_string(), AnalysisCache::new());
        store
            .trick_compile_caches
            .insert("ghost".to_string(), AnalysisCache::new());

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
            .tricks()
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
        let err = project_open_path_internal(&mut store, path.to_string_lossy().as_ref())
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
        let err = project_open_path_internal(&mut store, path.to_string_lossy().as_ref())
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
        let project = CadenceProjectDocument::new(
            "offset-grid".to_string(),
            Graph {
                nodes: BTreeMap::from([
                    (
                        GridPos { col: 4, row: 3 },
                        Node {
                            piece_id: "cadence.note".to_string(),
                            inline_params: BTreeMap::from([(
                                "value".to_string(),
                                Value::String("c3".to_string()),
                            )]),
                            pattern_source: None,
                            input_sides: BTreeMap::from([
                                ("pattern".to_string(), TileSide::LEFT),
                                ("value".to_string(), TileSide::BOTTOM),
                            ]),
                            output_side: Some(TileSide::RIGHT),
                            label: None,
                            node_state: None,
                        },
                    ),
                    (
                        GridPos { col: 5, row: 3 },
                        Node {
                            piece_id: "cadence.output".to_string(),
                            inline_params: BTreeMap::new(),
                            pattern_source: None,
                            input_sides: BTreeMap::from([("pattern".to_string(), TileSide::LEFT)]),
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
        let dto = project_open_path_internal(&mut store, path.to_string_lossy().as_ref())
            .expect("open compacted");
        assert_eq!(dto.name, "offset-grid");
        assert!(store.dirty);

        let opened = store.current_project.as_ref().expect("project");
        assert_eq!(opened.runtime_graph().cols, STANDARD_GRAPH_COLS);
        assert_eq!(opened.runtime_graph().rows, STANDARD_GRAPH_ROWS);
        assert!(
            opened
                .runtime_graph()
                .nodes
                .contains_key(&GridPos { col: 0, row: 0 })
        );
        assert!(
            opened
                .runtime_graph()
                .nodes
                .contains_key(&GridPos { col: 1, row: 0 })
        );
        let opened_edge = opened.runtime_graph().edges.values().next().expect("edge");
        assert_eq!(opened_edge.from, GridPos { col: 0, row: 0 });
        assert_eq!(opened_edge.to_node, GridPos { col: 1, row: 0 });

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn project_open_rejects_v2_document() {
        let path = temp_project_path("reject-v2");
        let project = LegacyV2ProjectDocument {
            schema_version: 2,
            name: "legacy".to_string(),
            graph: Graph {
                nodes: BTreeMap::new(),
                edges: BTreeMap::new(),
                name: "legacy".to_string(),
                cols: 9,
                rows: 9,
            },
        };
        let payload = serde_json::to_string_pretty(&project).expect("serialize legacy project");
        fs::write(path.as_path(), payload).expect("write temp project");

        let mut store = AppStore::default();
        let err = project_open_path_internal(&mut store, path.to_string_lossy().as_ref())
            .expect_err("v2 projects should be rejected");
        let message = err.to_string();
        assert!(message.contains("unsupported schema_version"));
        assert!(message.contains("2"));

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn project_open_marks_dirty_when_editor_contract_normalizes() {
        let path = temp_project_path("compact-clean");
        let project = CadenceProjectDocument::new(
            "clean".to_string(),
            Graph {
                nodes: BTreeMap::from([
                    (
                        GridPos { col: 0, row: 0 },
                        Node {
                            piece_id: "cadence.note".to_string(),
                            inline_params: BTreeMap::from([(
                                "value".to_string(),
                                Value::String("c3".to_string()),
                            )]),
                            pattern_source: None,
                            input_sides: BTreeMap::new(),
                            output_side: None,
                            label: None,
                            node_state: None,
                        },
                    ),
                    (
                        GridPos { col: 1, row: 0 },
                        Node {
                            piece_id: "cadence.output".to_string(),
                            inline_params: BTreeMap::new(),
                            pattern_source: None,
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
        project_open_path_internal(&mut store, path.to_string_lossy().as_ref())
            .expect("open clean");
        assert!(store.dirty);

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
        project_new_internal(&mut store, Some("Existing".to_string())).expect("new project");

        let view = project_bootstrap_internal(&mut store).expect("bootstrap existing");

        assert_eq!(view.name, "Existing");
        assert_eq!(
            store.current_project.as_ref().expect("project").name,
            "Existing"
        );
    }

    #[test]
    fn project_open_rejects_duplicate_trick_ids() {
        let path = temp_project_path("duplicate-tricks");
        let mut project = default_project_graph("dup-tricks");
        project.workspace.tricks = vec![
            crate::domain::project::CadenceTrickWorkspace {
                id: "pulse".to_string(),
                name: "Pulse".to_string(),
                graph: default_trick_graph("Pulse"),
            },
            crate::domain::project::CadenceTrickWorkspace {
                id: "pulse".to_string(),
                name: "Pulse Again".to_string(),
                graph: default_trick_graph("Pulse Again"),
            },
        ];
        let payload = serde_json::to_string_pretty(&project).expect("serialize project");
        fs::write(path.as_path(), payload).expect("write temp project");

        let mut store = AppStore::default();
        let err = project_open_path_internal(&mut store, path.to_string_lossy().as_ref())
            .expect_err("duplicate trick ids must fail");
        assert!(err.to_string().contains("duplicate trick id"));

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn project_open_rejects_ambiguous_sample_selectors() {
        let path = temp_project_path("sample-selector-collision");
        let mut project = default_project_graph("selector-collision");
        project.init_stage.sample_loads = vec![
            crate::domain::project::CadenceSampleLoad {
                id: "bd".to_string(),
                source: crate::adapter::samples::DEFAULT_KICK_SELECTOR.to_string(),
                aliases: BTreeMap::from([("kick".to_string(), "bd".to_string())]),
            },
            crate::domain::project::CadenceSampleLoad {
                id: "kick".to_string(),
                source: "club/kick.wav".to_string(),
                aliases: BTreeMap::new(),
            },
        ];
        let payload = serde_json::to_string_pretty(&project).expect("serialize project");
        fs::write(path.as_path(), payload).expect("write temp project");

        let mut store = AppStore::default();
        let err = project_open_path_internal(&mut store, path.to_string_lossy().as_ref())
            .expect_err("ambiguous sample selectors must fail");
        assert!(err.to_string().contains("ambiguous sample selector"));

        let _ = fs::remove_file(path.as_path());
    }

    #[test]
    fn invalid_persisted_tempo_surfaces_compile_diagnostic() {
        let path = temp_project_path("invalid-persisted-tempo");
        let mut project = default_project_graph("tempo-warning");
        project.init_stage.cps_expr = Some("113/0".to_string());
        let payload = serde_json::to_string_pretty(&project).expect("serialize project");
        fs::write(path.as_path(), payload).expect("write temp project");

        let mut store = AppStore::default();
        project_open_path_internal(&mut store, path.to_string_lossy().as_ref())
            .expect("invalid tempo should still load");
        let compiled = compile_project(active_project(&store).expect("project"));
        assert!(!compiled.can_play);
        assert!(compiled.diagnostics.iter().any(|diagnostic| {
            matches!(
                &diagnostic.kind,
                DiagnosticKind::InvalidOperation { reason }
                    if reason.contains("invalid cps expression")
            )
        }));

        let _ = fs::remove_file(path.as_path());
    }
}
