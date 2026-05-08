//! Public DTOs and input payloads for the Cadence backend surface.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::application::state::AppStore;

pub use crate::application::project::ops::{
    SampleLibraryEntryDto, SampleLibrarySnapshotDto, SampleLibrarySourceKind,
};
pub use crate::domain::common::{DomainBridgeKind, EdgeId, GridPos, PortType, TileSide};
pub use crate::domain::preview::{
    Diagnostic as DiagnosticDto, DiagnosticKind as DiagnosticKindDto,
    DiagnosticSeverity as DiagnosticSeverityDto, DomainBridge as DomainBridgeDto,
    PreviewDocument as PreviewDocumentDto, PreviewEvent as PreviewEventDto,
    RationalTime as RationalTimeDto, SemanticOutputType as SemanticOutputTypeDto,
    SemanticSnapshot as SemanticSnapshotDto,
};
pub use crate::domain::project::{
    GraphTarget as CadenceGraphTarget, InitStageOp, InitStageSnapshot as InitStageSnapshotDto,
    InitStageTrickSummary as InitStageTrickDto, SampleLoad as CadenceSampleLoadDto,
};
pub use crate::domain::script::ScriptInputContext as ScriptInputContextDto;

#[derive(Debug, Default)]
/// Shared backend state wrapper around the single in-memory [`AppStore`].
pub struct SharedAppState {
    pub(crate) store: Mutex<AppStore>,
}

impl SharedAppState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectDto {
    pub name: String,
    pub node_count: usize,
    pub edge_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
pub struct ProjectPathChoiceDto {
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportSongResultDto {
    pub exported: bool,
    pub message: String,
    pub path: Option<String>,
    #[serde(default)]
    pub diagnostics: Vec<DiagnosticDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeCommitArgs {
    pub cpm: Option<f32>,
    pub force: Option<bool>,
    pub playing: Option<bool>,
    pub request_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParamValueKind {
    Number,
    Text,
    Bool,
    Rational,
    Json,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParamInlineMode {
    Literal,
    Ident,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamKind {
    Pattern,
    Control,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LiveEditMode {
    Onset,
    Segment,
    Runtime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleSlotDef {
    pub param_id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_kind: Option<StreamKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_semantics: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_context: Option<ScriptInputContextDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleInputDef {
    pub param_id: String,
    #[serde(default)]
    pub slots: Vec<BundleSlotDef>,
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
    /// Numeric parameter that also accepts an incoming pattern stream.
    NumberOrPattern {
        default: f64,
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
    Rational {
        default: String,
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
    #[serde(default)]
    pub input_context: Option<ScriptInputContextDto>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_kind: Option<StreamKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_input: Option<BundleInputDef>,
    #[serde(default)]
    pub temporal_kind: String,
    #[serde(default)]
    pub fan_in: String,
    #[serde(default)]
    pub fan_out: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatternPos {
    pub col: i32,
    pub row: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternSurface {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roots: Vec<PatternRoot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternRoot {
    pub position: PatternPos,
    pub container: PatternContainer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternContainer {
    pub kind: PatternContainerKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<PatternItem>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatternContainerKind {
    Basic,
    Subdivide,
    Alternate,
    Parallel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PatternItem {
    pub position: PatternPos,
    pub kind: PatternItemKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatternItemKind {
    Atom(PatternAtom),
    Container(Box<PatternContainer>),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PatternAtom {
    Note { value: String },
    Scalar { value: f64 },
    Rest,
    Operator { operator: PatternOperatorKind },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PatternOperatorKind {
    Elongation,
    PitchShift,
    Slow,
    Fast,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNodeDto {
    pub position: GridPos,
    pub piece_id: String,
    #[serde(default)]
    pub inline_params: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern_source: Option<PatternSurface>,
    #[serde(default)]
    pub input_sides: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_side: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_state: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphEdgeDto {
    pub id: String,
    pub from: GridPos,
    pub to_node: GridPos,
    pub to_param: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphSnapshotDto {
    #[serde(default)]
    pub nodes: Vec<GraphNodeDto>,
    #[serde(default)]
    pub edges: Vec<GraphEdgeDto>,
    pub name: String,
    pub cols: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphCompilePreviewDto {
    pub can_compile: bool,
    #[serde(default)]
    pub diagnostics: Vec<DiagnosticDto>,
    #[serde(default)]
    pub eval_order: Vec<GridPos>,
    #[serde(default)]
    pub outputs: Vec<GridPos>,
    #[serde(default)]
    pub preview: PreviewDocumentDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectCompilePreviewDto {
    pub can_render: bool,
    pub can_play: bool,
    #[serde(default)]
    pub diagnostics: Vec<DiagnosticDto>,
    #[serde(default)]
    pub preview: PreviewDocumentDto,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimeCommitDto {
    pub success: bool,
    pub rev: u64,
    pub changed: bool,
    pub status: RuntimeStatusDto,
    #[serde(default)]
    pub sample_loads: Vec<CadenceSampleLoadDto>,
    pub output_count: usize,
    pub request_id: Option<u64>,
    #[serde(default)]
    pub diagnostics: Vec<DiagnosticDto>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeProgramStateDto {
    None,
    Current,
    StaleLastGood,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeStatusDto {
    pub rev: u64,
    pub playing: bool,
    pub program_state: RuntimeProgramStateDto,
    pub has_program: bool,
    pub last_error: Option<String>,
    pub play_elapsed_ms: u64,
    pub cycle_position: RationalTimeDto,
    pub cps: RationalTimeDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSyncSnapshotDto {
    pub project: ProjectViewDto,
    pub init_stage: InitStageSnapshotDto,
    pub graph: GraphSnapshotDto,
    #[serde(default)]
    pub catalog: Vec<PieceDef>,
    pub graph_preview: GraphCompilePreviewDto,
    pub project_preview: ProjectCompilePreviewDto,
    pub history_status: HistoryStatusDto,
    pub runtime_status: RuntimeStatusDto,
    pub diagnostics: DiagnosticsSnapshotDto,
    pub recovery_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_library: Option<SampleLibrarySnapshotDto>,
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
    #[serde(default)]
    pub entries: Vec<DiagnosticEntryDto>,
    pub mini_console_visible: bool,
    pub devtools_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphApplyResultDto {
    pub graph: GraphSnapshotDto,
    pub semantic: SemanticSnapshotDto,
    pub preview: GraphCompilePreviewDto,
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum GraphOp {
    NodePlace {
        position: GridPos,
        piece_id: String,
        #[serde(default)]
        inline_params: BTreeMap<String, Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pattern_source: Option<PatternSurface>,
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
        position: GridPos,
        param_id: String,
        value: Value,
    },
    ParamClearInline {
        position: GridPos,
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
    NodeSetPatternSurface {
        position: GridPos,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pattern_source: Option<PatternSurface>,
    },
    ResizeGrid {
        cols: u32,
        rows: u32,
    },
}

pub(crate) fn invalid_graph_op_diagnostic(message: String) -> Vec<DiagnosticDto> {
    vec![DiagnosticDto {
        kind: DiagnosticKindDto::InvalidOperation { reason: message },
        site: None,
        edge_id: None,
        severity: DiagnosticSeverityDto::Error,
    }]
}
