//! Cadence-owned preview and diagnostic concepts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::common::{DomainBridgeKind, EdgeId, GridPos, PortType, TileSide};

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
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub site: Option<GridPos>,
    pub edge_id: Option<EdgeId>,
    pub severity: DiagnosticSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainBridge {
    pub edge_id: EdgeId,
    pub source_pos: GridPos,
    pub target_pos: GridPos,
    pub param: String,
    pub kind: DomainBridgeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RationalTime {
    pub numerator: i64,
    pub denominator: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PreviewEvent {
    pub start: RationalTime,
    pub end: RationalTime,
    pub start_normalized: f64,
    pub end_normalized: f64,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<GridPos>,
    pub output_lane: GridPos,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PreviewDocument {
    #[serde(default)]
    pub delay_edges: Vec<EdgeId>,
    #[serde(default)]
    pub domain_bridges: Vec<DomainBridge>,
    #[serde(default)]
    pub output_lanes: Vec<GridPos>,
    #[serde(default)]
    pub preview_events: Vec<PreviewEvent>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debug_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticOutputType {
    pub position: GridPos,
    pub port_type: PortType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticSnapshot {
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default)]
    pub eval_order: Vec<GridPos>,
    #[serde(default)]
    pub outputs: Vec<GridPos>,
    #[serde(default)]
    pub output_types: Vec<SemanticOutputType>,
    #[serde(default)]
    pub domain_bridges: Vec<DomainBridge>,
    #[serde(default)]
    pub delay_edges: Vec<EdgeId>,
}
