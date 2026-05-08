//! Tessera analysis and graph authoring bindings.

use std::collections::BTreeMap;

pub(crate) mod cadence_schema;
pub(crate) mod graph_defaults;
#[cfg(test)]
mod grid_tests;
pub(crate) mod host_adapter;
pub(crate) mod lowering;
pub(crate) mod piece_registry;
pub(crate) mod pieces;

use tessera::{
    analysis::AnalyzedGraph,
    diagnostics::{
        Diagnostic as TesseraDiagnostic, DiagnosticKind as TesseraDiagnosticKind,
        DiagnosticSeverity as TesseraDiagnosticSeverity,
    },
    graph::{
        Edge as TesseraEdge, Graph as TesseraGraph, GraphOp as TesseraGraphOp, Node as TesseraNode,
    },
    types::{EdgeId as TesseraEdgeId, GridPos as TesseraGridPos, TileSide as TesseraTileSide},
};
use uuid::Uuid;

use crate::{
    domain::{
        project::{GraphTarget, InitStageOp},
        script::ScriptInputContext,
    },
    infrastructure::dto::{self, GraphOp},
};

use crate::domain::preview::{
    Diagnostic as CadenceDiagnostic, DiagnosticKind as CadenceDiagnosticKind,
    DiagnosticSeverity as CadenceDiagnosticSeverity, DomainBridge as CadenceDomainBridge,
    SemanticOutputType, SemanticSnapshot,
};
use crate::domain::program::{CadenceSubgraphInput, CadenceSubgraphSignature};

pub(crate) fn graph_target_to_internal(
    target: dto::CadenceGraphTarget,
) -> Result<GraphTarget, String> {
    crate::adapter::transport::translate(target)
}

pub(crate) fn init_stage_ops_to_internal(
    ops: Vec<dto::InitStageOp>,
) -> Result<Vec<InitStageOp>, String> {
    ops.into_iter().map(init_stage_op_to_internal).collect()
}

fn init_stage_op_to_internal(op: dto::InitStageOp) -> Result<InitStageOp, String> {
    Ok(match op {
        dto::InitStageOp::SetCps { expr } => InitStageOp::SetCps { expr },
        dto::InitStageOp::SampleLoadUpsert {
            id,
            source,
            aliases,
        } => InitStageOp::SampleLoadUpsert {
            id,
            source,
            aliases,
        },
        dto::InitStageOp::SampleLoadRemove { id } => InitStageOp::SampleLoadRemove { id },
        dto::InitStageOp::TrickCreate { id, name, graph } => {
            InitStageOp::TrickCreate { id, name, graph }
        }
        dto::InitStageOp::TrickRename { id, name } => InitStageOp::TrickRename { id, name },
        dto::InitStageOp::TrickDelete { id } => InitStageOp::TrickDelete { id },
    })
}

pub(crate) fn graph_ops_to_internal(ops: Vec<GraphOp>) -> Result<Vec<TesseraGraphOp>, String> {
    ops.into_iter().map(graph_op_to_internal).collect()
}

pub(crate) fn encode_graph_snapshot(graph: &TesseraGraph) -> dto::GraphSnapshotDto {
    dto::GraphSnapshotDto {
        nodes: graph
            .nodes
            .iter()
            .map(|(position, node)| encode_graph_node(*position, node))
            .collect(),
        edges: graph.edges.values().map(encode_graph_edge).collect(),
        name: graph.name.clone(),
        cols: graph.cols,
        rows: graph.rows,
    }
}

pub(crate) fn decode_graph_snapshot(graph: dto::GraphSnapshotDto) -> Result<TesseraGraph, String> {
    let nodes = graph
        .nodes
        .into_iter()
        .map(|node| {
            let position = grid_pos_to_tessera(node.position);
            let input_sides = node
                .input_sides
                .into_iter()
                .map(|(param_id, side)| parse_tile_side(side).map(|side| (param_id, side)))
                .collect::<Result<BTreeMap<_, _>, _>>()?;
            let output_side = node.output_side.map(parse_tile_side).transpose()?;
            let pattern_source = crate::adapter::transport::translate(node.pattern_source)?;
            Ok((
                position,
                TesseraNode {
                    piece_id: node.piece_id,
                    inline_params: node.inline_params,
                    pattern_source,
                    input_sides,
                    output_side,
                    label: node.label,
                    node_state: node.node_state,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;

    let edges = graph
        .edges
        .into_iter()
        .map(|edge| {
            let edge_id = edge_id_to_tessera(dto::EdgeId(edge.id))?;
            Ok((
                edge_id.clone(),
                TesseraEdge {
                    id: edge_id,
                    from: grid_pos_to_tessera(edge.from),
                    to_node: grid_pos_to_tessera(edge.to_node),
                    to_param: edge.to_param,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;

    Ok(TesseraGraph {
        nodes,
        edges,
        name: graph.name,
        cols: graph.cols,
        rows: graph.rows,
    })
}

pub(crate) fn cadence_diagnostic(value: TesseraDiagnostic) -> CadenceDiagnostic {
    CadenceDiagnostic {
        kind: cadence_diagnostic_kind(value.kind),
        site: value.site.map(Into::into),
        edge_id: value.edge_id.map(Into::into),
        severity: cadence_diagnostic_severity(value.severity),
    }
}

pub(crate) fn cadence_diagnostics(values: Vec<TesseraDiagnostic>) -> Vec<CadenceDiagnostic> {
    values.into_iter().map(cadence_diagnostic).collect()
}

pub(crate) fn cadence_domain_bridge(value: tessera::types::DomainBridge) -> CadenceDomainBridge {
    CadenceDomainBridge {
        edge_id: value.edge_id.into(),
        source_pos: value.source_pos.into(),
        target_pos: value.target_pos.into(),
        param: value.param,
        kind: value.kind.into(),
    }
}

pub(crate) fn encode_semantic_snapshot(analyzed: &AnalyzedGraph) -> SemanticSnapshot {
    SemanticSnapshot {
        diagnostics: cadence_diagnostics(analyzed.diagnostics.clone()),
        eval_order: analyzed
            .eval_order
            .iter()
            .copied()
            .map(Into::into)
            .collect(),
        outputs: analyzed.outputs.iter().copied().map(Into::into).collect(),
        output_types: analyzed
            .output_types
            .iter()
            .map(|(position, port_type)| SemanticOutputType {
                position: (*position).into(),
                port_type: port_type.clone().into(),
            })
            .collect(),
        domain_bridges: analyzed
            .domain_bridges
            .values()
            .cloned()
            .map(cadence_domain_bridge)
            .collect(),
        delay_edges: analyzed
            .delay_edges
            .iter()
            .cloned()
            .map(Into::into)
            .collect(),
    }
}

pub(crate) fn cadence_subgraph_signature(
    value: tessera::subgraph::SubgraphSignature,
) -> CadenceSubgraphSignature {
    CadenceSubgraphSignature {
        inputs: value
            .inputs
            .into_iter()
            .map(|input| CadenceSubgraphInput {
                slot: input.slot,
                pos: input.pos.into(),
                label: input.label,
                port_type: input.port_type.into(),
                required: input.required,
                is_receiver: input.is_receiver,
                default_value: input.default_value,
            })
            .collect(),
        output_pos: value.output_pos.into(),
        output_type: value.output_type.map(Into::into),
    }
}

pub(crate) fn encode_graph_edge(edge: &TesseraEdge) -> dto::GraphEdgeDto {
    dto::GraphEdgeDto {
        id: edge.id.0.to_string(),
        from: edge.from.into(),
        to_node: edge.to_node.into(),
        to_param: edge.to_param.clone(),
    }
}

fn encode_graph_node(position: TesseraGridPos, node: &TesseraNode) -> dto::GraphNodeDto {
    dto::GraphNodeDto {
        position: position.into(),
        piece_id: node.piece_id.clone(),
        inline_params: node.inline_params.clone(),
        pattern_source: crate::adapter::transport::translate(node.pattern_source.clone())
            .expect("pattern surface should translate"),
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

pub(crate) fn graph_op_to_internal(op: GraphOp) -> Result<TesseraGraphOp, String> {
    Ok(match op {
        GraphOp::NodePlace {
            position,
            piece_id,
            inline_params,
            pattern_source,
        } => TesseraGraphOp::NodePlace {
            position: grid_pos_to_tessera(position),
            piece_id,
            inline_params,
            pattern_source: crate::adapter::transport::translate(pattern_source)?,
        },
        GraphOp::NodeMove { from, to } => TesseraGraphOp::NodeMove {
            from: grid_pos_to_tessera(from),
            to: grid_pos_to_tessera(to),
        },
        GraphOp::NodeSwap { a, b } => TesseraGraphOp::NodeSwap {
            a: grid_pos_to_tessera(a),
            b: grid_pos_to_tessera(b),
        },
        GraphOp::NodeRemove { position } => TesseraGraphOp::NodeRemove {
            position: grid_pos_to_tessera(position),
        },
        GraphOp::EdgeConnect {
            edge_id,
            from,
            to_node,
            to_param,
        } => TesseraGraphOp::EdgeConnect {
            edge_id: edge_id.map(edge_id_to_tessera).transpose()?,
            from: grid_pos_to_tessera(from),
            to_node: grid_pos_to_tessera(to_node),
            to_param,
        },
        GraphOp::EdgeDisconnect { edge_id } => TesseraGraphOp::EdgeDisconnect {
            edge_id: edge_id_to_tessera(edge_id)?,
        },
        GraphOp::ParamSetInline {
            position,
            param_id,
            value,
        } => TesseraGraphOp::ParamSetInline {
            position: grid_pos_to_tessera(position),
            param_id,
            value,
        },
        GraphOp::ParamClearInline { position, param_id } => TesseraGraphOp::ParamClearInline {
            position: grid_pos_to_tessera(position),
            param_id,
        },
        GraphOp::ParamSetSide {
            position,
            param_id,
            side,
        } => TesseraGraphOp::ParamSetSide {
            position: grid_pos_to_tessera(position),
            param_id,
            side: parse_tile_side(side)?,
        },
        GraphOp::ParamClearSide { position, param_id } => TesseraGraphOp::ParamClearSide {
            position: grid_pos_to_tessera(position),
            param_id,
        },
        GraphOp::OutputSetSide { position, side } => TesseraGraphOp::OutputSetSide {
            position: grid_pos_to_tessera(position),
            side: parse_tile_side(side)?,
        },
        GraphOp::OutputClearSide { position } => TesseraGraphOp::OutputClearSide {
            position: grid_pos_to_tessera(position),
        },
        GraphOp::NodeAutoWire { position } => TesseraGraphOp::NodeAutoWire {
            position: grid_pos_to_tessera(position),
        },
        GraphOp::NodeSetLabel { position, label } => TesseraGraphOp::NodeSetLabel {
            position: grid_pos_to_tessera(position),
            label,
        },
        GraphOp::NodeSetState { position, state } => TesseraGraphOp::NodeSetState {
            position: grid_pos_to_tessera(position),
            state,
        },
        GraphOp::NodeSetPatternSurface {
            position,
            pattern_source,
        } => TesseraGraphOp::NodeSetPatternSurface {
            position: grid_pos_to_tessera(position),
            pattern_source: crate::adapter::transport::translate(pattern_source)?,
        },
        GraphOp::ResizeGrid { cols, rows } => TesseraGraphOp::ResizeGrid { cols, rows },
    })
}

pub(crate) fn grid_pos_to_tessera(value: dto::GridPos) -> TesseraGridPos {
    TesseraGridPos {
        col: value.col,
        row: value.row,
    }
}

pub(crate) fn edge_id_to_tessera(value: dto::EdgeId) -> Result<TesseraEdgeId, String> {
    let uuid = Uuid::parse_str(value.0.as_str())
        .map_err(|error| format!("invalid edge id `{}`: {error}", value.0))?;
    Ok(TesseraEdgeId(uuid))
}

fn parse_tile_side(label: String) -> Result<TesseraTileSide, String> {
    match label.as_str() {
        "top" => Ok(TesseraTileSide::TOP),
        "right" => Ok(TesseraTileSide::RIGHT),
        "bottom" => Ok(TesseraTileSide::BOTTOM),
        "left" => Ok(TesseraTileSide::LEFT),
        other => Err(format!("unknown tile side `{other}`")),
    }
}

pub(crate) fn encode_diagnostics(
    diagnostics: Vec<tessera::diagnostics::Diagnostic>,
) -> Result<Vec<dto::DiagnosticDto>, String> {
    crate::adapter::transport::translate(cadence_diagnostics(diagnostics))
}

pub(crate) fn encode_piece_catalog(
    defs: Vec<tessera::piece::PieceDef>,
) -> Result<Vec<dto::PieceDef>, String> {
    let semantics = defs
        .iter()
        .map(|piece| {
            piece
                .params
                .iter()
                .map(|param| {
                    ScriptInputContext::for_piece_param(
                        piece.id.as_str(),
                        param.id.as_str(),
                        Some(param.text_semantics.as_str()),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut encoded: Vec<dto::PieceDef> = crate::adapter::transport::translate(defs)?;
    for (piece, contexts) in encoded.iter_mut().zip(semantics) {
        for (param, context) in piece.params.iter_mut().zip(contexts) {
            param.input_context = context;
        }
        attach_piece_editor_metadata(piece);
    }
    Ok(encoded)
}

fn attach_piece_editor_metadata(piece: &mut dto::PieceDef) {
    match piece.id.as_str() {
        "cadence.container.basic"
        | "cadence.container.subdivide"
        | "cadence.container.alternate"
        | "cadence.container.parallel"
        | "cadence.atom.note"
        | "cadence.atom.scalar"
        | "cadence.atom.rest"
        | "cadence.atom.operator.elongation"
        | "cadence.atom.operator.pitch_shift"
        | "cadence.atom.operator.slow"
        | "cadence.atom.operator.fast"
        | "cadence.sound"
        | "cadence.note"
        | "cadence.add"
        | "cadence.scale" => {
            piece.stream_kind = Some(dto::StreamKind::Pattern);
        }

        "args_connector" => {
            piece.stream_kind = Some(dto::StreamKind::Control);
            piece.category = "connector".to_string();
        }
        "cadence.reverb_send" => {
            piece.bundle_input = Some(dto::BundleInputDef {
                param_id: "config".to_string(),
                slots: vec![
                    bundle_slot("amount", "amount"),
                    bundle_slot("decay", "decay"),
                    bundle_slot("damping", "damping"),
                ],
            });
        }
        "cadence.delay" => {
            piece.bundle_input = Some(dto::BundleInputDef {
                param_id: "config".to_string(),
                slots: vec![
                    bundle_slot("amount", "amount"),
                    bundle_slot("time", "time"),
                    bundle_slot("feedback", "feedback"),
                    bundle_slot("damping", "damping"),
                ],
            });
        }
        "cadence.compressor" => {
            piece.bundle_input = Some(dto::BundleInputDef {
                param_id: "config".to_string(),
                slots: vec![
                    bundle_slot("threshold", "threshold"),
                    bundle_slot("ratio", "ratio"),
                    bundle_slot("attack", "attack"),
                    bundle_slot("release", "release"),
                ],
            });
        }
        _ => {}
    }
}

fn bundle_slot(param_id: &str, label: &str) -> dto::BundleSlotDef {
    dto::BundleSlotDef {
        param_id: param_id.to_string(),
        label: label.to_string(),
        stream_kind: Some(dto::StreamKind::Control),
        text_semantics: None,
        input_context: None,
    }
}

fn tile_side_label(side: TesseraTileSide) -> String {
    match side {
        TesseraTileSide::TOP => "top".to_string(),
        TesseraTileSide::RIGHT => "right".to_string(),
        TesseraTileSide::BOTTOM => "bottom".to_string(),
        TesseraTileSide::LEFT => "left".to_string(),
    }
}

fn cadence_diagnostic_severity(value: TesseraDiagnosticSeverity) -> CadenceDiagnosticSeverity {
    match value {
        TesseraDiagnosticSeverity::Error => CadenceDiagnosticSeverity::Error,
        TesseraDiagnosticSeverity::Warning => CadenceDiagnosticSeverity::Warning,
        TesseraDiagnosticSeverity::Info => CadenceDiagnosticSeverity::Info,
    }
}

fn cadence_diagnostic_kind(value: TesseraDiagnosticKind) -> CadenceDiagnosticKind {
    match value {
        TesseraDiagnosticKind::PieceSemantic {
            piece_id,
            code,
            message,
        } => CadenceDiagnosticKind::PieceSemantic {
            piece_id,
            code,
            message,
        },
        TesseraDiagnosticKind::UnknownPiece { piece_id } => {
            CadenceDiagnosticKind::UnknownPiece { piece_id }
        }
        TesseraDiagnosticKind::UnknownNode { pos } => {
            CadenceDiagnosticKind::UnknownNode { pos: pos.into() }
        }
        TesseraDiagnosticKind::UnknownParam { piece_id, param } => {
            CadenceDiagnosticKind::UnknownParam { piece_id, param }
        }
        TesseraDiagnosticKind::InvalidOperation { reason } => {
            CadenceDiagnosticKind::InvalidOperation { reason }
        }
        TesseraDiagnosticKind::DuplicateConnection { to_node, to_param } => {
            CadenceDiagnosticKind::DuplicateConnection {
                to_node: to_node.into(),
                to_param,
            }
        }
        TesseraDiagnosticKind::DuplicateInputSide { side, params } => {
            CadenceDiagnosticKind::DuplicateInputSide {
                side: side.into(),
                params,
            }
        }
        TesseraDiagnosticKind::Cycle { involved } => CadenceDiagnosticKind::Cycle {
            involved: involved.into_iter().map(Into::into).collect(),
        },
        TesseraDiagnosticKind::NoOutputNode => CadenceDiagnosticKind::NoOutputNode,
        TesseraDiagnosticKind::UnreachableNode { position } => {
            CadenceDiagnosticKind::UnreachableNode {
                position: position.into(),
            }
        }
        TesseraDiagnosticKind::TypeMismatch {
            expected,
            got,
            param,
        } => CadenceDiagnosticKind::TypeMismatch {
            expected: expected.into(),
            got: got.into(),
            param,
        },
        TesseraDiagnosticKind::UnsupportedDomainCrossing {
            expected,
            got,
            param,
        } => CadenceDiagnosticKind::UnsupportedDomainCrossing {
            expected: expected.into(),
            got: got.into(),
            param,
        },
        TesseraDiagnosticKind::DelayTypeMismatch { default, feedback } => {
            CadenceDiagnosticKind::DelayTypeMismatch {
                default: default.into(),
                feedback: feedback.into(),
            }
        }
        TesseraDiagnosticKind::SideMismatch {
            from_pos,
            to_pos,
            expected_side,
        } => CadenceDiagnosticKind::SideMismatch {
            from_pos: from_pos.into(),
            to_pos: to_pos.into(),
            expected_side: expected_side.into(),
        },
        TesseraDiagnosticKind::NotAdjacent { from_pos, to_pos } => {
            CadenceDiagnosticKind::NotAdjacent {
                from_pos: from_pos.into(),
                to_pos: to_pos.into(),
            }
        }
        TesseraDiagnosticKind::OutputFromTerminal { position } => {
            CadenceDiagnosticKind::OutputFromTerminal {
                position: position.into(),
            }
        }
        TesseraDiagnosticKind::MissingRequiredParam { param } => {
            CadenceDiagnosticKind::MissingRequiredParam { param }
        }
        TesseraDiagnosticKind::InlineNotAllowed { param } => {
            CadenceDiagnosticKind::InlineNotAllowed { param }
        }
        TesseraDiagnosticKind::InlineTypeMismatch {
            param,
            expected,
            got_value,
        } => CadenceDiagnosticKind::InlineTypeMismatch {
            param,
            expected: expected.into(),
            got_value,
        },
        TesseraDiagnosticKind::RoleMismatch {
            expected,
            got,
            param,
        } => CadenceDiagnosticKind::InvalidOperation {
            reason: format!(
                "role mismatch on `{param}`: expected {:?}, got {:?}",
                expected, got
            ),
        },
        TesseraDiagnosticKind::PatternOperatorMissingTarget { path } => {
            CadenceDiagnosticKind::InvalidOperation {
                reason: format!("pattern operator at {:?} is missing a target", path),
            }
        }
        TesseraDiagnosticKind::PatternOperatorMissingArgument { path } => {
            CadenceDiagnosticKind::InvalidOperation {
                reason: format!("pattern operator at {:?} is missing an argument", path),
            }
        }
        TesseraDiagnosticKind::PatternConsecutiveOperators { path } => {
            CadenceDiagnosticKind::InvalidOperation {
                reason: format!("pattern contains consecutive operators at {:?}", path),
            }
        }
        TesseraDiagnosticKind::PatternTypeMismatch { path, reason } => {
            CadenceDiagnosticKind::InvalidOperation {
                reason: format!("pattern type mismatch at {:?}: {}", path, reason),
            }
        }
        TesseraDiagnosticKind::InvalidPatternPlacement { path, reason } => {
            CadenceDiagnosticKind::InvalidOperation {
                reason: format!("invalid pattern placement at {:?}: {}", path, reason),
            }
        }
    }
}
