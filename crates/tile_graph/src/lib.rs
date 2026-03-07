pub mod code_expr;
pub mod compiler;
pub mod diagnostics;
pub mod graph;
pub mod ops;
pub mod piece;
pub mod piece_registry;
pub mod semantic;
pub mod types;

pub use code_expr::CodeExpr;
pub use compiler::{CompileMode, CompileProgram, NodeStateUpdate, compile_graph};
pub use diagnostics::{Diagnostic, DiagnosticKind, DiagnosticSeverity, SemanticResult};
pub use graph::{Edge, Graph, GraphOp, GraphOpRecord, Node, ProjectDocument};
pub use ops::{
    ApplyOpsOutcome, EdgeConnectProbeReason, EdgeTargetParamProbe, apply_ops_to_graph,
    pick_target_param_for_edge, probe_edge_connect, validate_edge_connect,
};
pub use piece::{
    ParamDef, ParamInlineMode, ParamSchema, ParamValueKind, Piece, PieceDef, PieceInputs,
};
pub use piece_registry::PieceRegistry;
pub use semantic::semantic_pass;
pub use types::{EdgeId, GridPos, PieceCategory, PortType, TileSide, adjacent_in_direction};
