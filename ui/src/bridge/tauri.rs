use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectViewDto {
    pub name: String,
    pub schema_version: u32,
    pub node_count: usize,
    pub edge_count: usize,
    pub dirty: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPathChoiceDto {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirtyDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridPos {
    pub col: i32,
    pub row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct EdgeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamValueKind {
    Number,
    Text,
    Bool,
    Json,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamInlineMode {
    Literal,
    Raw,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParamSchema {
    Number {
        default: f64,
        min: Option<f64>,
        max: Option<f64>,
        can_inline: bool,
    },
    Text {
        default: String,
        can_inline: bool,
    },
    Enum {
        options: Vec<String>,
        default: String,
        can_inline: bool,
    },
    Bool {
        default: bool,
        can_inline: bool,
    },
    Custom {
        port_type: String,
        value_kind: ParamValueKind,
        default: Option<Value>,
        can_inline: bool,
        inline_mode: ParamInlineMode,
        min: Option<f64>,
        max: Option<f64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDef {
    pub id: String,
    pub label: String,
    pub side: String,
    pub schema: ParamSchema,
    pub variadic_group: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PieceDef {
    pub id: String,
    pub label: String,
    pub category: String,
    pub params: Vec<ParamDef>,
    pub output_type: Option<String>,
    pub output_side: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshotDto {
    pub nodes: Value,
    pub edges: Value,
    pub name: String,
    pub cols: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCompilePreviewDto {
    pub can_compile: bool,
    pub code: Option<String>,
    pub exprs: Vec<Value>,
    pub diagnostics: Vec<Value>,
    pub eval_order: Vec<GridPos>,
    pub terminals: Vec<GridPos>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphApplyResultDto {
    pub graph: GraphSnapshotDto,
    pub semantic: Value,
    pub preview_code: Option<String>,
    pub removed_edges: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum GraphOp {
    NodePlace {
        position: GridPos,
        piece_id: String,
        #[serde(default)]
        inline_params: BTreeMap<String, Value>,
    },
    NodeMove {
        from: GridPos,
        to: GridPos,
    },
    NodeSwap {
        a: GridPos,
        b: GridPos,
    },
    NodeRemove {
        position: GridPos,
    },
    EdgeConnect {
        #[serde(default)]
        edge_id: Option<EdgeId>,
        from: GridPos,
        to_node: GridPos,
        to_param: String,
    },
    EdgeDisconnect {
        edge_id: EdgeId,
    },
    ParamSetInline {
        #[serde(alias = "node")]
        position: GridPos,
        #[serde(alias = "param")]
        param_id: String,
        value: Value,
    },
    ParamClearInline {
        #[serde(alias = "node")]
        position: GridPos,
        #[serde(alias = "param")]
        param_id: String,
    },
    ParamSetSide {
        position: GridPos,
        param_id: String,
        side: String,
    },
    ParamClearSide {
        position: GridPos,
        param_id: String,
    },
    OutputSetSide {
        position: GridPos,
        side: String,
    },
    OutputClearSide {
        position: GridPos,
    },
    NodeSetLabel {
        position: GridPos,
        label: Option<String>,
    },
    NodeSetState {
        position: GridPos,
        state: Option<Value>,
    },
    ResizeGrid {
        cols: u32,
        rows: u32,
    },
}

#[derive(Debug, Clone, Serialize)]
struct ProjectCreateArgs {
    name: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectNewArgs {
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectOpenPathArgs {
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectSaveAsArgs {
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct GraphApplyArgs {
    ops: Vec<GraphOp>,
    request_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct MiniConsoleVisibilityArgs {
    visible: bool,
}

#[derive(Debug, Clone, Serialize)]
struct DevtoolsVisibilityArgs {
    visible: bool,
}

#[cfg(target_arch = "wasm32")]
mod wasm_bridge {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(inline_js = r#"
export async function cadenceInvoke(command, payload) {
  const invoke =
    window.__TAURI__?.core?.invoke ??
    window.__TAURI_INTERNALS__?.invoke;
  if (typeof invoke !== "function") {
    throw new Error("tauri invoke unavailable");
  }
  return await invoke(command, payload);
}
"#)]
    extern "C" {
        #[wasm_bindgen(catch, js_name = cadenceInvoke)]
        pub async fn cadence_invoke(command: &str, payload: JsValue) -> Result<JsValue, JsValue>;
    }
}

#[cfg(target_arch = "wasm32")]
fn js_error_message(value: wasm_bindgen::JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| "tauri invoke failed".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn non_wasm_bridge_error() -> String {
    "tauri bridge unavailable on non-wasm target".to_string()
}

async fn invoke_command<T, R>(command: &str, args: &T) -> Result<R, String>
where
    T: Serialize + ?Sized,
    R: DeserializeOwned,
{
    #[cfg(target_arch = "wasm32")]
    {
        let payload = serde_wasm_bindgen::to_value(args).map_err(|err| err.to_string())?;
        let raw = wasm_bridge::cadence_invoke(command, payload)
            .await
            .map_err(js_error_message)?;
        return serde_wasm_bindgen::from_value(raw).map_err(|err| err.to_string());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (command, args);
        Err(non_wasm_bridge_error())
    }
}

pub async fn project_create(name: String) -> Result<ProjectDto, String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err("name cannot be empty".to_string());
    }
    invoke_command(
        "project_create",
        &ProjectCreateArgs {
            name: normalized.to_string(),
        },
    )
    .await
}

pub async fn project_new(name: Option<String>) -> Result<ProjectDto, String> {
    invoke_command("project_new", &ProjectNewArgs { name }).await
}

pub async fn project_open_path(path: String) -> Result<ProjectDto, String> {
    invoke_command("project_open_path", &ProjectOpenPathArgs { path }).await
}

pub async fn project_save_current() -> Result<String, String> {
    invoke_command("project_save_current", &()).await
}

pub async fn project_save_as(path: String) -> Result<String, String> {
    invoke_command("project_save_as", &ProjectSaveAsArgs { path }).await
}

pub async fn project_dirty_status() -> Result<ProjectDirtyStatusDto, String> {
    invoke_command("project_dirty_status", &()).await
}

pub async fn project_snapshot() -> Result<ProjectViewDto, String> {
    invoke_command("project_snapshot", &()).await
}

pub async fn graph_snapshot() -> Result<GraphSnapshotDto, String> {
    invoke_command("graph_snapshot", &()).await
}

pub async fn graph_piece_catalog() -> Result<Vec<PieceDef>, String> {
    invoke_command("graph_piece_catalog", &()).await
}

pub async fn graph_compile_preview() -> Result<GraphCompilePreviewDto, String> {
    invoke_command("graph_compile_preview", &()).await
}

pub async fn graph_apply_ops(
    ops: Vec<GraphOp>,
    request_id: Option<String>,
) -> Result<GraphApplyResultDto, String> {
    invoke_command("graph_apply_ops", &GraphApplyArgs { ops, request_id }).await
}

pub async fn ui_set_mini_console_visible(visible: bool) -> Result<(), String> {
    invoke_command(
        "ui_set_mini_console_visible",
        &MiniConsoleVisibilityArgs { visible },
    )
    .await
}

pub async fn ui_set_devtools_visible(visible: bool) -> Result<(), String> {
    invoke_command(
        "ui_set_devtools_visible",
        &DevtoolsVisibilityArgs { visible },
    )
    .await
}
