//! Canonical project and init-stage data structures shared across the desktop app.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tessera::graph::{Edge, Graph, GraphOpRecord, Node};
use tessera::subgraph::SubgraphDef;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Top-level saved project document for Cadence.
pub struct CadenceProjectDocument {
    /// Persisted schema version for migration/compatibility checks.
    pub schema_version: u32,
    /// User-visible project name.
    pub name: String,
    /// Main runtime graph edited in the grid workspace.
    pub graph: Graph,
    /// Pre-runtime setup: CPS, sample loads, and reusable tricks.
    #[serde(default)]
    pub init_stage: CadenceInitStage,
}

impl CadenceProjectDocument {
    /// Current on-disk schema version written by this build.
    pub const SCHEMA_VERSION: u32 = 3;

    /// Build a new project document around an already prepared runtime graph.
    pub fn new(name: String, graph: Graph) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            name,
            graph,
            init_stage: CadenceInitStage::default(),
        }
    }

    /// Resolve either the runtime graph or a trick graph by target.
    pub fn graph(&self, target: &CadenceGraphTarget) -> Option<&Graph> {
        match target {
            CadenceGraphTarget::Runtime => Some(&self.graph),
            CadenceGraphTarget::Trick { trick_id } => self
                .init_stage
                .tricks
                .iter()
                .find(|trick| trick.id == *trick_id)
                .map(|trick| &trick.graph),
        }
    }

    /// Mutable variant of [`CadenceProjectDocument::graph`].
    pub fn graph_mut(&mut self, target: &CadenceGraphTarget) -> Option<&mut Graph> {
        match target {
            CadenceGraphTarget::Runtime => Some(&mut self.graph),
            CadenceGraphTarget::Trick { trick_id } => self
                .init_stage
                .tricks
                .iter_mut()
                .find(|trick| trick.id == *trick_id)
                .map(|trick| &mut trick.graph),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
/// Setup executed before the main runtime graph is compiled or played.
pub struct CadenceInitStage {
    /// Optional tempo expression rendered as `setCps(...)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cps_expr: Option<String>,
    /// Sample banks to preload into the Strudel runtime.
    #[serde(default)]
    pub sample_loads: Vec<CadenceSampleLoad>,
    /// User-authored reusable subgraphs.
    #[serde(default)]
    pub tricks: Vec<CadenceTrickDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// One named sample-bank load in the init stage.
pub struct CadenceSampleLoad {
    /// Short id used from Strudel code, for example `bd`.
    pub id: String,
    /// Remote or local source passed through to `samples(...)`.
    pub source: String,
    /// Optional aliases expanded within the loaded bank.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

/// Cadence's reusable "trick" definition is a Tessera subgraph.
pub type CadenceTrickDef = SubgraphDef;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
/// Which graph a graph-oriented command should operate on.
pub enum CadenceGraphTarget {
    /// The main song/runtime graph.
    #[default]
    Runtime,
    /// A reusable trick graph stored inside the init stage.
    Trick {
        /// Stable trick identifier, not the display name.
        trick_id: String,
    },
}

#[derive(Debug, Clone)]
/// Undo/redo entry that remembers which graph a mutation belongs to.
pub struct TargetedGraphOpRecord {
    pub target: CadenceGraphTarget,
    pub record: GraphOpRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// One placed graph node serialized for Cadence's frontend boundary.
pub struct GraphNodeDto {
    pub position: tessera::types::GridPos,
    pub piece_id: String,
    #[serde(default)]
    pub inline_params: BTreeMap<String, Value>,
    #[serde(default)]
    pub input_sides: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_side: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_state: Option<Value>,
}

impl From<(tessera::types::GridPos, &Node)> for GraphNodeDto {
    fn from((position, node): (tessera::types::GridPos, &Node)) -> Self {
        Self {
            position,
            piece_id: node.piece_id.clone(),
            inline_params: node.inline_params.clone(),
            input_sides: node
                .input_sides
                .iter()
                .map(|(param_id, side)| (param_id.clone(), tile_side_label(*side)))
                .collect(),
            output_side: node.output_side.map(tile_side_label),
            label: node.label.clone(),
            node_state: node.node_state.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// One graph edge serialized for Cadence's frontend boundary.
pub struct GraphEdgeDto {
    pub id: String,
    pub from: tessera::types::GridPos,
    pub to_node: tessera::types::GridPos,
    pub to_param: String,
}

impl From<&Edge> for GraphEdgeDto {
    fn from(edge: &Edge) -> Self {
        Self {
            id: edge.id.0.to_string(),
            from: edge.from,
            to_node: edge.to_node,
            to_param: edge.to_param.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
/// Cadence-owned graph snapshot returned across the Tauri seam.
pub struct GraphSnapshotDto {
    #[serde(default)]
    pub nodes: Vec<GraphNodeDto>,
    #[serde(default)]
    pub edges: Vec<GraphEdgeDto>,
    pub name: String,
    pub cols: u32,
    pub rows: u32,
}

impl From<&Graph> for GraphSnapshotDto {
    fn from(graph: &Graph) -> Self {
        Self {
            nodes: graph
                .nodes
                .iter()
                .map(|(position, node)| GraphNodeDto::from((*position, node)))
                .collect(),
            edges: graph.edges.values().map(GraphEdgeDto::from).collect(),
            name: graph.name.clone(),
            cols: graph.cols,
            rows: graph.rows,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
/// Inferred output type for one node in a semantic snapshot.
pub struct SemanticOutputTypeDto {
    pub position: tessera::types::GridPos,
    pub port_type: tessera::types::PortType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Cadence-owned semantic snapshot returned alongside graph mutations.
pub struct SemanticSnapshotDto {
    #[serde(default)]
    pub diagnostics: Vec<DiagnosticDto>,
    #[serde(default)]
    pub eval_order: Vec<tessera::types::GridPos>,
    #[serde(default)]
    pub terminals: Vec<tessera::types::GridPos>,
    #[serde(default)]
    pub output_types: Vec<SemanticOutputTypeDto>,
    #[serde(default)]
    pub domain_bridges: Vec<DomainBridgeDto>,
    #[serde(default)]
    pub delay_edges: Vec<tessera::types::EdgeId>,
}

impl From<&tessera::diagnostics::SemanticResult> for SemanticSnapshotDto {
    fn from(value: &tessera::diagnostics::SemanticResult) -> Self {
        Self {
            diagnostics: value
                .diagnostics
                .clone()
                .into_iter()
                .map(DiagnosticDto::from)
                .collect(),
            eval_order: value.eval_order.clone(),
            terminals: value.terminals.clone(),
            output_types: value
                .output_types
                .iter()
                .map(|(position, port_type)| SemanticOutputTypeDto {
                    position: *position,
                    port_type: port_type.clone(),
                })
                .collect(),
            domain_bridges: value
                .domain_bridges
                .values()
                .cloned()
                .map(DomainBridgeDto::from)
                .collect(),
            delay_edges: value.delay_edges.iter().cloned().collect(),
        }
    }
}

fn tile_side_label(side: tessera::types::TileSide) -> String {
    match side {
        tessera::types::TileSide::TOP => "top".to_string(),
        tessera::types::TileSide::RIGHT => "right".to_string(),
        tessera::types::TileSide::BOTTOM => "bottom".to_string(),
        tessera::types::TileSide::LEFT => "left".to_string(),
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
/// Shared compile metadata emitted by Tessera preview/runtime compilation.
pub struct CompileMetaDto {
    #[serde(default)]
    pub delay_slots: Vec<DelaySlotDto>,
    #[serde(default)]
    pub domain_bridges: Vec<DomainBridgeDto>,
    #[serde(default)]
    pub activity_events: Vec<ActivityEventDto>,
}

impl From<&tessera::CompileProgram> for CompileMetaDto {
    fn from(value: &tessera::CompileProgram) -> Self {
        Self {
            delay_slots: value
                .delay_slots
                .iter()
                .cloned()
                .map(DelaySlotDto::from)
                .collect(),
            domain_bridges: value
                .domain_bridges
                .iter()
                .cloned()
                .map(DomainBridgeDto::from)
                .collect(),
            activity_events: value
                .activity_events
                .iter()
                .cloned()
                .map(ActivityEventDto::from)
                .collect(),
        }
    }
}

// --- Cadence-owned diagnostic types ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverityDto {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DiagnosticKindDto {
    UnknownPiece {
        piece_id: String,
    },
    UnknownNode {
        pos: tessera::types::GridPos,
    },
    UnknownParam {
        piece_id: String,
        param: String,
    },
    InvalidOperation {
        reason: String,
    },
    DuplicateConnection {
        to_node: tessera::types::GridPos,
        to_param: String,
    },
    Cycle {
        involved: Vec<tessera::types::GridPos>,
    },
    NoTerminalNode,
    MultipleTerminalNodes {
        positions: Vec<tessera::types::GridPos>,
    },
    UnreachableNode {
        position: tessera::types::GridPos,
    },
    TypeMismatch {
        expected: tessera::types::PortType,
        got: tessera::types::PortType,
        param: String,
    },
    UnsupportedDomainCrossing {
        expected: tessera::types::PortType,
        got: tessera::types::PortType,
        param: String,
    },
    DelayTypeMismatch {
        default: tessera::types::PortType,
        feedback: tessera::types::PortType,
    },
    SideMismatch {
        from_pos: tessera::types::GridPos,
        to_pos: tessera::types::GridPos,
        expected_side: tessera::types::TileSide,
    },
    NotAdjacent {
        from_pos: tessera::types::GridPos,
        to_pos: tessera::types::GridPos,
    },
    OutputFromTerminal {
        position: tessera::types::GridPos,
    },
    MissingRequiredParam {
        param: String,
    },
    InlineNotAllowed {
        param: String,
    },
    InlineTypeMismatch {
        param: String,
        expected: tessera::types::PortType,
        got_value: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiagnosticDto {
    pub kind: DiagnosticKindDto,
    pub site: Option<tessera::types::GridPos>,
    pub edge_id: Option<tessera::types::EdgeId>,
    pub severity: DiagnosticSeverityDto,
}

impl From<tessera::diagnostics::Diagnostic> for DiagnosticDto {
    fn from(d: tessera::diagnostics::Diagnostic) -> Self {
        Self {
            kind: DiagnosticKindDto::from(d.kind),
            site: d.site,
            edge_id: d.edge_id,
            severity: DiagnosticSeverityDto::from(d.severity),
        }
    }
}

impl From<tessera::diagnostics::DiagnosticKind> for DiagnosticKindDto {
    fn from(k: tessera::diagnostics::DiagnosticKind) -> Self {
        use tessera::diagnostics::DiagnosticKind as TK;
        match k {
            TK::UnknownPiece { piece_id } => Self::UnknownPiece { piece_id },
            TK::UnknownNode { pos } => Self::UnknownNode { pos },
            TK::UnknownParam { piece_id, param } => Self::UnknownParam { piece_id, param },
            TK::InvalidOperation { reason } => Self::InvalidOperation { reason },
            TK::DuplicateConnection { to_node, to_param } => {
                Self::DuplicateConnection { to_node, to_param }
            }
            TK::Cycle { involved } => Self::Cycle { involved },
            TK::NoTerminalNode => Self::NoTerminalNode,
            TK::MultipleTerminalNodes { positions } => Self::MultipleTerminalNodes { positions },
            TK::UnreachableNode { position } => Self::UnreachableNode { position },
            TK::TypeMismatch {
                expected,
                got,
                param,
            } => Self::TypeMismatch {
                expected,
                got,
                param,
            },
            TK::UnsupportedDomainCrossing {
                expected,
                got,
                param,
            } => Self::UnsupportedDomainCrossing {
                expected,
                got,
                param,
            },
            TK::DelayTypeMismatch { default, feedback } => {
                Self::DelayTypeMismatch { default, feedback }
            }
            TK::SideMismatch {
                from_pos,
                to_pos,
                expected_side,
            } => Self::SideMismatch {
                from_pos,
                to_pos,
                expected_side,
            },
            TK::NotAdjacent { from_pos, to_pos } => Self::NotAdjacent { from_pos, to_pos },
            TK::OutputFromTerminal { position } => Self::OutputFromTerminal { position },
            TK::MissingRequiredParam { param } => Self::MissingRequiredParam { param },
            TK::InlineNotAllowed { param } => Self::InlineNotAllowed { param },
            TK::InlineTypeMismatch {
                param,
                expected,
                got_value,
            } => Self::InlineTypeMismatch {
                param,
                expected,
                got_value,
            },
        }
    }
}

impl From<tessera::diagnostics::DiagnosticSeverity> for DiagnosticSeverityDto {
    fn from(s: tessera::diagnostics::DiagnosticSeverity) -> Self {
        use tessera::diagnostics::DiagnosticSeverity as TS;
        match s {
            TS::Error => Self::Error,
            TS::Warning => Self::Warning,
            TS::Info => Self::Info,
        }
    }
}

// --- CompileMeta sub-type DTOs ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelaySlotDto {
    pub slot: String,
    pub node: tessera::types::GridPos,
    pub default_expr: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_type: Option<tessera::types::PortType>,
}

impl From<tessera::compiler::DelaySlot> for DelaySlotDto {
    fn from(ds: tessera::compiler::DelaySlot) -> Self {
        Self {
            slot: ds.slot,
            node: ds.node,
            default_expr: serde_json::to_value(&ds.default_expr).unwrap_or_default(),
            port_type: ds.port_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainBridgeDto {
    pub edge_id: tessera::types::EdgeId,
    pub source_pos: tessera::types::GridPos,
    pub target_pos: tessera::types::GridPos,
    pub param: String,
    pub kind: tessera::types::DomainBridgeKind,
}

impl From<tessera::types::DomainBridge> for DomainBridgeDto {
    fn from(db: tessera::types::DomainBridge) -> Self {
        Self {
            edge_id: db.edge_id,
            source_pos: db.source_pos,
            target_pos: db.target_pos,
            param: db.param,
            kind: db.kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEventDto {
    pub site: tessera::types::GridPos,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    pub kind: ActivityKindDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<serde_json::Value>,
}

impl From<tessera::activity::ActivityEvent> for ActivityEventDto {
    fn from(ae: tessera::activity::ActivityEvent) -> Self {
        Self {
            site: ae.site,
            param: ae.param,
            kind: ActivityKindDto::from(ae.kind),
            at: ae
                .at
                .map(|expr| serde_json::to_value(expr).unwrap_or_default()),
        }
    }
}

impl From<tessera::activity::ActivityKind> for ActivityKindDto {
    fn from(k: tessera::activity::ActivityKind) -> Self {
        use tessera::activity::ActivityKind as AK;
        match k {
            AK::Trigger { label, intensity } => Self::Trigger { label, intensity },
            AK::Sustain { progress } => Self::Sustain { progress },
            AK::Processing { label } => Self::Processing { label },
            AK::RuntimeError { message } => Self::RuntimeError { message },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
/// Mutations supported by the init-stage editor.
pub enum InitStageOp {
    /// Set or clear the global CPS expression.
    SetCps { expr: Option<String> },
    /// Insert or update a sample load entry.
    SampleLoadUpsert {
        id: String,
        source: String,
        #[serde(default)]
        aliases: BTreeMap<String, String>,
    },
    /// Remove a sample load by id.
    SampleLoadRemove { id: String },
    /// Create a new trick, optionally seeding its graph.
    TrickCreate {
        id: String,
        name: String,
        #[serde(default)]
        graph: Option<Graph>,
    },
    /// Rename an existing trick.
    TrickRename { id: String, name: String },
    /// Delete a trick and its graph.
    TrickDelete { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Snapshot returned to the UI when it needs to render the init workspace.
pub struct InitStageSnapshotDto {
    pub cps_expr: Option<String>,
    pub sample_loads: Vec<CadenceSampleLoad>,
    pub tricks: Vec<InitStageTrickDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Compact metadata about one trick in the init workspace.
pub struct InitStageTrickDto {
    pub id: String,
    pub name: String,
    pub node_count: usize,
    pub edge_count: usize,
}

impl From<&CadenceInitStage> for InitStageSnapshotDto {
    fn from(value: &CadenceInitStage) -> Self {
        Self {
            cps_expr: value.cps_expr.clone(),
            sample_loads: value.sample_loads.clone(),
            tricks: value
                .tricks
                .iter()
                .map(|trick| InitStageTrickDto {
                    id: trick.id.clone(),
                    name: trick.name.clone(),
                    node_count: trick.graph.nodes.len(),
                    edge_count: trick.graph.edges.len(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Batch payload for applying multiple init-stage changes atomically.
pub struct InitStageApplyArgs {
    pub ops: Vec<InitStageOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Full-project compile preview used by the code modal and runtime controls.
pub struct ProjectCompilePreviewDto {
    pub can_render: bool,
    pub can_play: bool,
    pub code: Option<String>,
    pub diagnostics: Vec<DiagnosticDto>,
    #[serde(default)]
    pub compile_meta: CompileMetaDto,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tessera::activity::ActivityEvent;
    use tessera::ast::Expr;
    use tessera::compiler::{CompileProgram, DelaySlot};
    use tessera::types::{
        DomainBridge, DomainBridgeKind, EdgeId, ExecutionDomain, GridPos, PortType,
    };

    #[test]
    fn compile_meta_dto_copies_delay_slots_domain_bridges_and_activity_events() {
        let program = CompileProgram {
            terminals: Vec::new(),
            state_updates: Vec::new(),
            activity_events: vec![ActivityEvent::processing(
                GridPos { col: 4, row: 1 },
                "warming",
            )],
            delay_slots: vec![DelaySlot {
                slot: "d_2_1".to_string(),
                node: GridPos { col: 2, row: 1 },
                default_expr: Expr::int(0),
                port_type: Some(PortType::number().with_domain(ExecutionDomain::Control)),
            }],
            domain_bridges: vec![DomainBridge {
                edge_id: EdgeId::new(),
                source_pos: GridPos { col: 0, row: 0 },
                target_pos: GridPos { col: 1, row: 0 },
                param: "value".to_string(),
                kind: DomainBridgeKind::ControlToAudio,
            }],
            diagnostics: Vec::new(),
        };

        let dto = CompileMetaDto::from(&program);
        assert_eq!(dto.delay_slots.len(), 1);
        assert_eq!(dto.delay_slots[0].slot, "d_2_1");
        assert_eq!(dto.domain_bridges.len(), 1);
        assert_eq!(dto.domain_bridges[0].kind, DomainBridgeKind::ControlToAudio);
        assert_eq!(dto.activity_events.len(), 1);
        assert_eq!(dto.activity_events[0].site, GridPos { col: 4, row: 1 });
    }
}
