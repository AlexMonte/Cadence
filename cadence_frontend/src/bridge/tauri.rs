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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportSongResultDto {
    pub exported: bool,
    pub message: String,
    pub path: Option<String>,
    pub diagnostics: Vec<DiagnosticDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DirtyDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CadenceSampleLoadDto {
    pub id: String,
    pub source: String,
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridPos {
    pub col: i32,
    pub row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct EdgeId(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CadenceGraphTarget {
    Runtime,
    Trick { trick_id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitStageTrickDto {
    pub id: String,
    pub name: String,
    pub node_count: usize,
    pub edge_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InitStageSnapshotDto {
    pub cps_expr: Option<String>,
    pub sample_loads: Vec<CadenceSampleLoadDto>,
    pub tricks: Vec<InitStageTrickDto>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TerminalStrategy {
    Stack,
    Dollar,
}

impl Default for TerminalStrategy {
    fn default() -> Self {
        Self::Stack
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeCommitArgs {
    pub cpm: Option<f32>,
    pub force: Option<bool>,
    pub playing: Option<bool>,
    pub code_override: Option<String>,
    pub terminal_strategy: Option<TerminalStrategy>,
    pub request_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParamValueKind {
    Number,
    Text,
    Bool,
    Json,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ParamInlineMode {
    Literal,
    Ident,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum PortType {
    Plain(String),
    Detailed {
        kind: String,
        #[serde(default)]
        domain: Option<String>,
    },
}

impl PortType {
    pub fn kind(&self) -> &str {
        match self {
            Self::Plain(kind) | Self::Detailed { kind, .. } => kind.as_str(),
        }
    }

    pub fn domain(&self) -> Option<&str> {
        match self {
            Self::Plain(_) => Some("control"),
            Self::Detailed { domain, .. } => domain.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
        port_type: PortType,
        value_kind: ParamValueKind,
        default: Option<Value>,
        can_inline: bool,
        inline_mode: ParamInlineMode,
        min: Option<f64>,
        max: Option<f64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParamDef {
    pub id: String,
    pub label: String,
    pub side: String,
    pub schema: ParamSchema,
    #[serde(default)]
    pub text_semantics: Option<String>,
    pub variadic_group: Option<String>,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PieceDef {
    pub id: String,
    pub label: String,
    pub category: String,
    #[serde(default)]
    pub semantic_kind: String,
    pub namespace: String,
    pub params: Vec<ParamDef>,
    pub output_type: Option<PortType>,
    pub output_side: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeDto {
    pub position: GridPos,
    pub piece_id: String,
    #[serde(default)]
    pub inline_params: BTreeMap<String, Value>,
    #[serde(default)]
    pub input_sides: BTreeMap<String, String>,
    #[serde(default)]
    pub output_side: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub node_state: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphEdgeDto {
    pub id: String,
    pub from: GridPos,
    pub to_node: GridPos,
    pub to_param: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshotDto {
    pub nodes: Vec<GraphNodeDto>,
    pub edges: Vec<GraphEdgeDto>,
    pub name: String,
    pub cols: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiagnosticKind {
    UnknownPiece {
        piece_id: String,
    },
    UnknownNode {
        pos: GridPos,
    },
    UnknownParam {
        piece_id: String,
        param: String,
    },
    InvalidOperation {
        reason: String,
    },
    DuplicateConnection {
        to_node: GridPos,
        to_param: String,
    },
    Cycle {
        involved: Vec<GridPos>,
    },
    NoTerminalNode,
    MultipleTerminalNodes {
        positions: Vec<GridPos>,
    },
    UnreachableNode {
        position: GridPos,
    },
    TypeMismatch {
        expected: PortType,
        got: PortType,
        param: String,
    },
    UnsupportedDomainCrossing {
        expected: PortType,
        got: PortType,
        param: String,
    },
    DelayTypeMismatch {
        default: PortType,
        feedback: PortType,
    },
    SideMismatch {
        from_pos: GridPos,
        to_pos: GridPos,
        expected_side: TileSide,
    },
    NotAdjacent {
        from_pos: GridPos,
        to_pos: GridPos,
    },
    OutputFromTerminal {
        position: GridPos,
    },
    MissingRequiredParam {
        param: String,
    },
    InlineNotAllowed {
        param: String,
    },
    InlineTypeMismatch {
        param: String,
        expected: PortType,
        got_value: Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiagnosticDto {
    pub kind: DiagnosticKind,
    pub site: Option<GridPos>,
    pub edge_id: Option<EdgeId>,
    pub severity: DiagnosticSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DomainBridgeKind {
    ControlToAudio,
    AudioToControl,
    EventToControl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelaySlotDto {
    pub slot: String,
    pub node: GridPos,
    pub default_expr: Value,
    #[serde(default)]
    pub port_type: Option<PortType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainBridgeDto {
    pub edge_id: EdgeId,
    pub source_pos: GridPos,
    pub target_pos: GridPos,
    pub param: String,
    pub kind: DomainBridgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActivityKindDto {
    Trigger {
        #[serde(default)]
        label: Option<String>,
        intensity: f64,
    },
    Sustain {
        progress: f64,
    },
    Processing {
        label: String,
    },
    RuntimeError {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActivityEventDto {
    pub site: GridPos,
    #[serde(default)]
    pub param: Option<String>,
    pub kind: ActivityKindDto,
    #[serde(default)]
    pub at: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CompileMetaDto {
    #[serde(default)]
    pub delay_slots: Vec<DelaySlotDto>,
    #[serde(default)]
    pub domain_bridges: Vec<DomainBridgeDto>,
    #[serde(default)]
    pub activity_events: Vec<ActivityEventDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphCompilePreviewDto {
    pub can_compile: bool,
    pub code: Option<String>,
    pub exprs: Vec<String>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub eval_order: Vec<GridPos>,
    pub terminals: Vec<GridPos>,
    #[serde(default)]
    pub compile_meta: CompileMetaDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectCompilePreviewDto {
    pub can_render: bool,
    pub can_play: bool,
    pub code: Option<String>,
    pub diagnostics: Vec<DiagnosticDto>,
    #[serde(default)]
    pub compile_meta: CompileMetaDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeCommitDto {
    pub success: bool,
    pub rev: u64,
    pub changed: bool,
    pub playing: bool,
    pub code: Option<String>,
    pub cps_expr: Option<String>,
    pub sample_loads: Vec<CadenceSampleLoadDto>,
    pub declaration_code: Vec<String>,
    pub runtime_code: Option<String>,
    pub voice_count: usize,
    pub play_elapsed_ms: u64,
    pub request_id: Option<u64>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeStatusDto {
    pub rev: u64,
    pub playing: bool,
    pub has_program: bool,
    pub last_error: Option<String>,
    pub play_elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryStatusDto {
    pub can_undo: bool,
    pub can_redo: bool,
    pub past_len: usize,
    pub future_len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticEntryDto {
    pub at_ms: u64,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagnosticsSnapshotDto {
    pub entries: Vec<DiagnosticEntryDto>,
    pub mini_console_visible: bool,
    pub devtools_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticOutputTypeDto {
    pub position: GridPos,
    pub port_type: PortType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticSnapshotDto {
    pub diagnostics: Vec<DiagnosticDto>,
    pub eval_order: Vec<GridPos>,
    pub terminals: Vec<GridPos>,
    #[serde(default)]
    pub output_types: Vec<SemanticOutputTypeDto>,
    #[serde(default)]
    pub domain_bridges: Vec<DomainBridgeDto>,
    #[serde(default)]
    pub delay_edges: Vec<EdgeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphApplyResultDto {
    pub graph: GraphSnapshotDto,
    pub semantic: SemanticSnapshotDto,
    pub preview: GraphCompilePreviewDto,
    pub removed_edges: Vec<GraphEdgeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeConnectProbeReason {
    UnknownSourceNode,
    UnknownTargetNode,
    UnknownSourcePiece,
    UnknownTargetPiece,
    UnknownTargetParam,
    NotAdjacent,
    SideMismatch,
    OutputFromTerminal,
    NoParamOnTargetSide,
    TargetParamOccupied,
    TypeMismatch,
    UnsupportedDomain,
    NoCompatibleParam,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TileSide {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum RepairSuggestion {
    MoveNode {
        node: GridPos,
        to: GridPos,
    },
    SetOutputSide {
        position: GridPos,
        side: TileSide,
    },
    SetParamSide {
        position: GridPos,
        param_id: String,
        side: TileSide,
    },
    DisconnectEdge {
        edge_id: EdgeId,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphPickTargetParamDto {
    pub to_param: Option<String>,
    pub implicit_bridge: Option<DomainBridgeKind>,
    pub reason: Option<EdgeConnectProbeReason>,
    pub detail: Option<String>,
    #[serde(default)]
    pub suggestions: Vec<RepairSuggestion>,
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
    NodeAutoWire {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum InitStageOp {
    SetCps {
        expr: Option<String>,
    },
    SampleLoadUpsert {
        id: String,
        source: String,
        #[serde(default)]
        aliases: BTreeMap<String, String>,
    },
    SampleLoadRemove {
        id: String,
    },
    TrickCreate {
        id: String,
        name: String,
        #[serde(default)]
        graph: Option<Value>,
    },
    TrickRename {
        id: String,
        name: String,
    },
    TrickDelete {
        id: String,
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
struct ProjectRenameArgs {
    name: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectOpenPathArgs {
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectSaveArgs {
    path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct ProjectSaveAsArgs {
    path: String,
}

#[derive(Debug, Clone, Serialize)]
struct ExportSongArgs {
    path: String,
    cpm: Option<f32>,
    code_override: Option<String>,
    terminal_strategy: Option<TerminalStrategy>,
}

#[derive(Debug, Clone, Serialize)]
struct GraphApplyArgs {
    ops: Vec<GraphOp>,
    request_id: Option<String>,
    target: CadenceGraphTarget,
}

#[derive(Debug, Clone, Serialize)]
struct InitStageApplyArgs {
    ops: Vec<InitStageOp>,
}

#[derive(Debug, Clone, Serialize)]
struct GraphTargetArgs {
    target: CadenceGraphTarget,
}

#[derive(Debug, Clone, Serialize)]
struct GraphPickTargetParamArgs {
    from: GridPos,
    to_node: GridPos,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_param: Option<String>,
    target: CadenceGraphTarget,
}

#[derive(Debug, Clone, Serialize)]
struct MiniConsoleVisibilityArgs {
    visible: bool,
}

#[derive(Debug, Clone, Serialize)]
struct DevtoolsVisibilityArgs {
    visible: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ArgsEnvelope<'a, T: ?Sized> {
    args: &'a T,
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
        .or_else(|| {
            serde_wasm_bindgen::from_value::<Value>(value)
                .ok()
                .map(|raw| raw.to_string())
        })
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

async fn invoke_command_with_args<T, R>(command: &str, args: &T) -> Result<R, String>
where
    T: Serialize + ?Sized,
    R: DeserializeOwned,
{
    invoke_command(command, &ArgsEnvelope { args }).await
}

pub async fn project_create(name: String) -> Result<ProjectDto, String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err("name cannot be empty".to_string());
    }
    invoke_command_with_args(
        "project_create",
        &ProjectCreateArgs {
            name: normalized.to_string(),
        },
    )
    .await
}

pub async fn project_new(name: Option<String>) -> Result<ProjectDto, String> {
    invoke_command_with_args("project_new", &ProjectNewArgs { name }).await
}

pub async fn project_rename(name: String) -> Result<ProjectViewDto, String> {
    invoke_command_with_args("project_rename", &ProjectRenameArgs { name }).await
}

pub async fn project_open_path(path: String) -> Result<ProjectDto, String> {
    invoke_command_with_args("project_open_path", &ProjectOpenPathArgs { path }).await
}

pub async fn project_save(path: Option<String>) -> Result<String, String> {
    invoke_command_with_args("project_save", &ProjectSaveArgs { path }).await
}

pub async fn project_save_current() -> Result<String, String> {
    invoke_command("project_save_current", &()).await
}

pub async fn project_save_as(path: String) -> Result<String, String> {
    invoke_command_with_args("project_save_as", &ProjectSaveAsArgs { path }).await
}

pub async fn project_dirty_status() -> Result<ProjectDirtyStatusDto, String> {
    invoke_command("project_dirty_status", &()).await
}

pub async fn project_snapshot() -> Result<ProjectViewDto, String> {
    invoke_command("project_snapshot", &()).await
}

pub async fn project_bootstrap() -> Result<ProjectViewDto, String> {
    invoke_command("project_bootstrap", &()).await
}

pub async fn project_init_snapshot() -> Result<InitStageSnapshotDto, String> {
    invoke_command("project_init_snapshot", &()).await
}

pub async fn project_init_apply(ops: Vec<InitStageOp>) -> Result<InitStageSnapshotDto, String> {
    invoke_command_with_args("project_init_apply", &InitStageApplyArgs { ops }).await
}

pub async fn project_compile_preview() -> Result<ProjectCompilePreviewDto, String> {
    invoke_command("project_compile_preview", &()).await
}

pub async fn project_pick_open_path() -> Result<Option<String>, String> {
    invoke_command("project_pick_open_path", &()).await
}

pub async fn project_pick_save_path() -> Result<Option<String>, String> {
    invoke_command("project_pick_save_path", &()).await
}

pub async fn project_prompt_unsaved() -> Result<DirtyDecision, String> {
    invoke_command("project_prompt_unsaved", &()).await
}

pub async fn project_recovery_status() -> Result<ProjectPathChoiceDto, String> {
    invoke_command("project_recovery_status", &()).await
}

pub async fn project_recovery_write() -> Result<String, String> {
    invoke_command("project_recovery_write", &()).await
}

pub async fn project_recovery_load() -> Result<ProjectDto, String> {
    invoke_command("project_recovery_load", &()).await
}

pub async fn project_recovery_clear() -> Result<String, String> {
    invoke_command("project_recovery_clear", &()).await
}

pub async fn app_quit() -> Result<(), String> {
    invoke_command("app_quit", &()).await
}

pub async fn window_close_main() -> Result<(), String> {
    invoke_command("window_close_main", &()).await
}

pub async fn graph_snapshot(target: CadenceGraphTarget) -> Result<GraphSnapshotDto, String> {
    invoke_command_with_args("graph_snapshot", &GraphTargetArgs { target }).await
}

pub async fn graph_piece_catalog(target: CadenceGraphTarget) -> Result<Vec<PieceDef>, String> {
    invoke_command_with_args("graph_piece_catalog", &GraphTargetArgs { target }).await
}

pub async fn graph_compile_preview(
    target: CadenceGraphTarget,
) -> Result<GraphCompilePreviewDto, String> {
    invoke_command_with_args("graph_compile_preview", &GraphTargetArgs { target }).await
}

pub async fn graph_apply_ops(
    ops: Vec<GraphOp>,
    request_id: Option<String>,
    target: CadenceGraphTarget,
) -> Result<GraphApplyResultDto, String> {
    invoke_command_with_args(
        "graph_apply_ops",
        &GraphApplyArgs {
            ops,
            request_id,
            target,
        },
    )
    .await
}

pub async fn graph_pick_target_param(
    from: GridPos,
    to_node: GridPos,
    target: CadenceGraphTarget,
    to_param: Option<String>,
) -> Result<GraphPickTargetParamDto, String> {
    invoke_command_with_args(
        "graph_pick_target_param",
        &GraphPickTargetParamArgs {
            from,
            to_node,
            to_param,
            target,
        },
    )
    .await
}

pub async fn runtime_commit(args: RuntimeCommitArgs) -> Result<RuntimeCommitDto, String> {
    invoke_command_with_args("runtime_commit", &args).await
}

pub async fn runtime_status() -> Result<RuntimeStatusDto, String> {
    invoke_command("runtime_status", &()).await
}

pub async fn runtime_stop() -> Result<RuntimeStatusDto, String> {
    invoke_command("runtime_stop", &()).await
}

pub async fn runtime_reset_on_project_swap() -> Result<RuntimeStatusDto, String> {
    invoke_command("runtime_reset_on_project_swap", &()).await
}

pub async fn history_status() -> Result<HistoryStatusDto, String> {
    invoke_command("history_status", &()).await
}

pub async fn history_undo() -> Result<HistoryStatusDto, String> {
    invoke_command("history_undo", &()).await
}

pub async fn history_redo() -> Result<HistoryStatusDto, String> {
    invoke_command("history_redo", &()).await
}

pub async fn diagnostics_snapshot() -> Result<DiagnosticsSnapshotDto, String> {
    invoke_command("diagnostics_snapshot", &()).await
}

pub async fn export_pick_song_path() -> Result<Option<String>, String> {
    invoke_command("export_pick_song_path", &()).await
}

pub async fn export_song(
    path: String,
    cpm: Option<f32>,
    code_override: Option<String>,
    terminal_strategy: Option<TerminalStrategy>,
) -> Result<ExportSongResultDto, String> {
    invoke_command_with_args(
        "export_song",
        &ExportSongArgs {
            path,
            cpm,
            code_override,
            terminal_strategy,
        },
    )
    .await
}

pub async fn ui_set_mini_console_visible(visible: bool) -> Result<(), String> {
    invoke_command_with_args(
        "ui_set_mini_console_visible",
        &MiniConsoleVisibilityArgs { visible },
    )
    .await
}

pub async fn ui_set_devtools_visible(visible: bool) -> Result<(), String> {
    invoke_command_with_args(
        "ui_set_devtools_visible",
        &DevtoolsVisibilityArgs { visible },
    )
    .await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{DiagnosticKind, DomainBridgeKind, PieceDef, PortType};

    #[test]
    fn port_type_accepts_string_and_object_shapes() {
        let plain: PortType = serde_json::from_value(json!("pattern")).expect("plain port type");
        assert_eq!(plain.kind(), "pattern");
        assert_eq!(plain.domain(), Some("control"));

        let detailed: PortType = serde_json::from_value(json!({
            "kind": "bool",
            "domain": "audio"
        }))
        .expect("detailed port type");
        assert_eq!(detailed.kind(), "bool");
        assert_eq!(detailed.domain(), Some("audio"));
    }

    #[test]
    fn piece_def_deserializes_tessera_catalog_entries_with_object_port_types() {
        let piece: PieceDef = serde_json::from_value(json!({
            "id": "core.if_expr",
            "label": "if expr",
            "category": "control",
            "semantic_kind": "construct",
            "namespace": "core",
            "params": [
                {
                    "id": "cond",
                    "label": "cond",
                    "side": "left",
                    "schema": {
                        "kind": "custom",
                        "port_type": { "kind": "bool" },
                        "value_kind": "bool",
                        "default": false,
                        "can_inline": true,
                        "inline_mode": "literal",
                        "min": null,
                        "max": null
                    },
                    "text_semantics": "plain",
                    "variadic_group": null,
                    "required": true
                }
            ],
            "output_type": { "kind": "any" },
            "output_side": "right",
            "description": "Branch between values.",
            "tags": ["control"]
        }))
        .expect("piece def should deserialize");

        assert_eq!(piece.namespace, "core");
        assert_eq!(piece.output_type.as_ref().map(PortType::kind), Some("any"));
        assert_eq!(piece.tags, vec!["control"]);
    }

    #[test]
    fn diagnostic_kind_deserializes_structured_payloads() {
        let kind: DiagnosticKind = serde_json::from_value(json!({
            "kind": "unsupported_domain_crossing",
            "expected": { "kind": "pattern", "domain": "audio" },
            "got": { "kind": "number", "domain": "control" },
            "param": "value"
        }))
        .expect("diagnostic kind should deserialize");

        match kind {
            DiagnosticKind::UnsupportedDomainCrossing {
                expected,
                got,
                param,
            } => {
                assert_eq!(expected.kind(), "pattern");
                assert_eq!(got.kind(), "number");
                assert_eq!(param, "value");
            }
            other => panic!("unexpected diagnostic kind: {other:?}"),
        }
    }

    #[test]
    fn graph_probe_implicit_bridge_deserializes_as_typed_enum() {
        let payload = json!({
            "to_param": "value",
            "implicit_bridge": "control_to_audio",
            "reason": null,
            "detail": "bridgeable",
            "suggestions": []
        });

        let probe = serde_json::from_value::<super::GraphPickTargetParamDto>(payload)
            .expect("probe should deserialize");
        assert_eq!(
            probe.implicit_bridge,
            Some(DomainBridgeKind::ControlToAudio)
        );
    }
}
