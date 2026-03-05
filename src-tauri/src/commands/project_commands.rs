use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::graph::{Edge, Graph, Node, ProjectDocument};
use crate::core::piece_registry::PieceRegistry;
use crate::core::semantic::semantic_pass;
use crate::core::types::{EdgeId, GridPos};
use crate::errors::{AppError, AppResult};
use crate::store::app_state::AppStore;
use crate::store::fs_store::{load_project, save_project};
use crate::store::history::{capture_snapshot, clear_graph_history, record_successful_mutation};

#[derive(Debug, Default)]
pub struct SharedAppState {
    pub store: Mutex<AppStore>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectDto {
    pub name: String,
    pub node_count: usize,
    pub edge_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDirtyStatusDto {
    pub dirty: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
pub enum DirtyDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectCreateArgs {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectOpenArgs {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSaveArgs {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectNewArgs {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectOpenPathArgs {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectSaveAsArgs {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPathChoiceDto {
    pub path: Option<String>,
}

fn validate_project_name(name: &str) -> AppResult<String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AppError::InvalidInput(
            "project name cannot be empty".to_string(),
        ));
    }
    Ok(normalized.to_string())
}

fn default_project_graph(name: &str) -> ProjectDocument {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("c3".to_string()))]),
        input_sides: Default::default(),
        output_side: None,
},
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
        input_sides: Default::default(),
        output_side: None,
},
    );
    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "pattern".to_string(),
    };

    ProjectDocument::new(
        name.to_string(),
        Graph {
            nodes,
            edges: BTreeMap::from([(edge.id.clone(), edge)]),
            name: name.to_string(),
        },
    )
}

pub(crate) fn active_project_mut(store: &mut AppStore) -> AppResult<&mut ProjectDocument> {
    store.current_project.as_mut().ok_or_else(|| {
        AppError::InvalidInput("no active project; create or open a project first".to_string())
    })
}

pub(crate) fn active_project(store: &AppStore) -> AppResult<&ProjectDocument> {
    store.current_project.as_ref().ok_or_else(|| {
        AppError::InvalidInput("no active project; create or open a project first".to_string())
    })
}

fn validate_project_document(graph: &ProjectDocument) -> AppResult<()> {
    if graph.schema_version != ProjectDocument::SCHEMA_VERSION {
        return Err(AppError::InvalidInput(format!(
            "unsupported schema_version: {} (expected {})",
            graph.schema_version,
            ProjectDocument::SCHEMA_VERSION,
        )));
    }

    let registry = PieceRegistry::default_strudel();
    let sem = semantic_pass(&graph.graph, &registry);
    if !sem.errors.is_empty() {
        return Err(AppError::InvalidInput(
            "project graph is invalid for current schema".to_string(),
        ));
    }

    Ok(())
}

fn project_dto(graph: &ProjectDocument) -> ProjectDto {
    ProjectDto {
        name: graph.name.clone(),
        node_count: graph.graph.nodes.len(),
        edge_count: graph.graph.edges.len(),
    }
}

fn project_to_view(store: &AppStore, graph: &ProjectDocument) -> ProjectViewDto {
    ProjectViewDto {
        name: graph.name.clone(),
        schema_version: graph.schema_version,
        node_count: graph.graph.nodes.len(),
        edge_count: graph.graph.edges.len(),
        dirty: store.dirty,
        path: store
            .current_path
            .as_ref()
            .map(|path| path.display().to_string()),
    }
}

fn project_fingerprint(graph: &ProjectDocument) -> AppResult<String> {
    use std::hash::{Hash, Hasher};

    let payload = serde_json::to_vec(graph)?;
    let mut hasher = DefaultHasher::new();
    payload.hash(&mut hasher);
    Ok(format!("{:016x}", hasher.finish()))
}

fn mark_store_clean(store: &mut AppStore) -> AppResult<()> {
    let graph = active_project(store)?;
    store.last_saved_snapshot_hash = Some(project_fingerprint(graph)?);
    store.dirty = false;
    Ok(())
}

pub(crate) fn mark_store_dirty(store: &mut AppStore) {
    store.dirty = true;
}

pub(crate) fn project_new_internal(
    store: &mut AppStore,
    args: ProjectNewArgs,
) -> AppResult<ProjectDto> {
    let name = validate_project_name(args.name.as_deref().unwrap_or("Untitled"))?;
    let history_snapshot = capture_snapshot(store);
    store.current_project = Some(default_project_graph(name.as_str()));
    clear_graph_history(store);
    store.current_path = None;
    store.selection = Default::default();
    store.runtime.last_code.clear();
    store.runtime.last_error = None;
    store.runtime.reset_playback_clock();
    store.runtime.set_playing(false);
    mark_store_dirty(store);
    record_successful_mutation(store, history_snapshot);
    store.push_diagnostic("project_new", name.clone());
    Ok(project_dto(active_project(store)?))
}

fn resolve_path(path: &str) -> AppResult<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("path cannot be empty".to_string()));
    }
    Ok(PathBuf::from(trimmed))
}

pub(crate) fn project_open_path_internal(
    store: &mut AppStore,
    path: &Path,
) -> AppResult<ProjectDto> {
    let payload = load_project(path)?;

    let raw: Value = serde_json::from_str(payload.as_str())?;
    let schema_version = raw
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            AppError::InvalidInput(
                "unsupported project format: missing schema_version (expected v2)".to_string(),
            )
        })?;
    if schema_version != ProjectDocument::SCHEMA_VERSION as u64 {
        return Err(AppError::InvalidInput(format!(
            "This project was created with an older version and cannot be opened. No automatic migration is available. (schema_version={}, expected={})",
            schema_version,
            ProjectDocument::SCHEMA_VERSION
        )));
    }

    let graph: ProjectDocument = serde_json::from_value(raw)?;
    validate_project_document(&graph)?;

    store.current_project = Some(graph);
    clear_graph_history(store);
    store.current_path = Some(path.to_path_buf());
    store.selection = Default::default();
    store.runtime.last_code.clear();
    store.runtime.last_error = None;
    store.runtime.reset_playback_clock();
    store.runtime.set_playing(false);
    mark_store_clean(store)?;
    store.push_diagnostic("project_open", path.display().to_string());
    Ok(project_dto(active_project(store)?))
}

pub(crate) fn project_save_as_internal(store: &mut AppStore, path: &Path) -> AppResult<String> {
    let graph = active_project(store)?;
    let payload = serde_json::to_string_pretty(graph)?;
    save_project(path, payload.as_str())?;
    store.current_path = Some(path.to_path_buf());
    mark_store_clean(store)?;
    store.push_diagnostic("project_save_as", path.display().to_string());
    Ok(format!("saved project to {}", path.display()))
}

pub(crate) fn project_save_current_internal(store: &mut AppStore) -> AppResult<String> {
    let path = store
        .current_path
        .clone()
        .ok_or_else(|| AppError::InvalidInput("no save path set; use Save As".to_string()))?;
    project_save_as_internal(store, path.as_path())
}

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
pub fn project_open(
    state: tauri::State<'_, SharedAppState>,
    args: ProjectOpenArgs,
) -> Result<ProjectDto, String> {
    project_open_path(state, ProjectOpenPathArgs { path: args.path })
}

#[tauri::command]
pub fn project_save_current(state: tauri::State<'_, SharedAppState>) -> Result<String, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    project_save_current_internal(&mut store).map_err(|err| err.to_string())
}

#[tauri::command]
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
pub fn project_snapshot(state: tauri::State<'_, SharedAppState>) -> Result<ProjectViewDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let graph = active_project(&store).map_err(|err| err.to_string())?;
    Ok(project_to_view(&store, graph))
}

#[tauri::command]
pub fn project_pick_open_path(
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

    let mut dialog = FileDialog::new().add_filter("GrooveAtlas Project", &["json"]);
    if let Some(directory) = initial_directory.as_ref() {
        dialog = dialog.set_directory(directory);
    }
    Ok(dialog.pick_file().map(|path| path.display().to_string()))
}

#[tauri::command]
pub fn project_pick_save_path(
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

    let mut dialog = FileDialog::new().add_filter("GrooveAtlas Project", &["json"]);
    if let Some(directory) = initial_directory.as_ref() {
        dialog = dialog.set_directory(directory);
    }
    if let Some(name) = initial_name.as_deref() {
        dialog = dialog.set_file_name(name);
    } else {
        dialog = dialog.set_file_name("project.grooveatlas.json");
    }
    Ok(dialog.save_file().map(|path| path.display().to_string()))
}

#[tauri::command]
pub fn project_prompt_unsaved() -> Result<DirtyDecision, String> {
    Ok(DirtyDecision::Cancel)
}

#[tauri::command]
pub fn project_recovery_status() -> Result<ProjectPathChoiceDto, String> {
    Ok(ProjectPathChoiceDto { path: None })
}

#[tauri::command]
pub fn project_recovery_write() -> Result<String, String> {
    Ok("recovery snapshot disabled in graph-canonical mode".to_string())
}

#[tauri::command]
pub fn project_recovery_load() -> Result<ProjectDto, String> {
    Err("recovery snapshot disabled in graph-canonical mode".to_string())
}

#[tauri::command]
pub fn project_recovery_clear() -> Result<String, String> {
    Ok("recovery snapshot disabled in graph-canonical mode".to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use uuid::Uuid;

    use super::*;

    fn temp_project_path(label: &str) -> PathBuf {
        let file = format!("grooveatlas-{label}-{}.json", Uuid::new_v4());
        std::env::temp_dir().join(file)
    }

    #[test]
    fn default_project_graph_uses_v2_canonical_shape() {
        let project = default_project_graph("demo");
        assert_eq!(project.schema_version, ProjectDocument::SCHEMA_VERSION);
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
        assert_eq!(source.piece_id, "strudel.note");
        assert_eq!(terminal.piece_id, "strudel.output");
        assert_eq!(project.graph.edges.len(), 1);
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
        assert!(message.contains("older version"));
        assert!(message.contains("schema_version=1"));

        let _ = fs::remove_file(path.as_path());
    }
}
