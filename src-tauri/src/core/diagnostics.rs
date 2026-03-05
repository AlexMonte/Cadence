use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::core::types::{EdgeId, GridPos, PortType, TileSide};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub site: Option<GridPos>,
    pub edge_id: Option<EdgeId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticResult {
    pub errors: Vec<Diagnostic>,
    pub eval_order: Vec<GridPos>,
    pub terminal: Option<GridPos>,
}

impl SemanticResult {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty() && self.terminal.is_some()
    }
}
