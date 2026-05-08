use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScriptInputContextDto {
    SourcePattern,
    NotePattern,
    StructuralPattern,
    Pattern,
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
pub struct ParamDef {
    pub id: String,
    pub label: String,
    pub side: String,
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
    pub output_side: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeDto {
    pub position: GridPos,
    pub piece_id: String,
    #[serde(default)]
    pub inline_params: BTreeMap<String, Value>,
    #[serde(default)]
    pub pattern_source: Option<PatternSurface>,
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
    PieceSemantic {
        piece_id: String,
        code: String,
        message: String,
    },
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
    DuplicateInputSide {
        side: TileSide,
        params: Vec<String>,
    },
    Cycle {
        involved: Vec<GridPos>,
    },
    NoOutputNode,
    UnreachableNode {
        position: GridPos,
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
        got_value: Value,
    },
    RoleMismatch {
        param: String,
        target_role: String,
        source_role: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainBridgeDto {
    pub edge_id: EdgeId,
    pub source_pos: GridPos,
    pub target_pos: GridPos,
    pub param: String,
    pub kind: DomainBridgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RationalTimeDto {
    pub numerator: i64,
    pub denominator: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewEventDto {
    pub start: RationalTimeDto,
    pub end: RationalTimeDto,
    pub start_normalized: f64,
    pub end_normalized: f64,
    pub label: String,
    #[serde(default)]
    pub site: Option<GridPos>,
    pub output_lane: GridPos,
    #[serde(default)]
    pub pitch: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PreviewDocumentDto {
    #[serde(default)]
    pub delay_edges: Vec<EdgeId>,
    #[serde(default)]
    pub domain_bridges: Vec<DomainBridgeDto>,
    #[serde(default)]
    pub output_lanes: Vec<GridPos>,
    #[serde(default)]
    pub preview_events: Vec<PreviewEventDto>,
    #[serde(default)]
    pub debug_text: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticSnapshotDto {
    pub diagnostics: Vec<DiagnosticDto>,
    pub eval_order: Vec<GridPos>,
    pub outputs: Vec<GridPos>,
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
        #[serde(default)]
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
    NodeSetPatternSurface {
        position: GridPos,
        #[serde(default)]
        pattern_source: Option<PatternSurface>,
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{DomainBridgeKind, PieceDef};

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
        assert_eq!(piece.tags, vec!["control"]);
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
