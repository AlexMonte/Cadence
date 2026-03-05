use crate::core::diagnostics::{Diagnostic, DiagnosticKind};
use crate::core::graph::{Edge, Graph, GraphOp, Node};
use crate::core::piece::PieceDef;
use crate::core::piece_registry::PieceRegistry;
use crate::core::types::{EdgeId, GridPos, TileSide, adjacent_in_direction};

const GRID_COLS: i32 = 9;
const GRID_ROWS: i32 = 9;

fn invalid_op(site: Option<GridPos>, reason: impl Into<String>) -> Diagnostic {
    Diagnostic {
        kind: DiagnosticKind::InvalidOperation {
            reason: reason.into(),
        },
        site,
        edge_id: None,
    }
}

fn in_bounds(pos: &GridPos) -> bool {
    (0..GRID_COLS).contains(&pos.col) && (0..GRID_ROWS).contains(&pos.row)
}

fn ensure_in_bounds(errors: &mut Vec<Diagnostic>, pos: &GridPos, label: &str) -> bool {
    if in_bounds(pos) {
        return true;
    }
    errors.push(invalid_op(
        Some(pos.clone()),
        format!(
            "{} out of bounds at ({}, {}), allowed cols=[0..{}), rows=[0..{})",
            label, pos.col, pos.row, GRID_COLS, GRID_ROWS
        ),
    ));
    false
}

#[derive(Debug, Default)]
pub struct ApplyOpsOutcome {
    pub removed_edges: Vec<Edge>,
    pub applied_ops: Vec<GraphOp>,
    pub undo_ops: Vec<GraphOp>,
}

fn edge_is_still_adjacent(edge: &Edge, graph: &Graph, registry: &PieceRegistry) -> bool {
    let Some(target_node) = graph.nodes.get(&edge.to_node) else {
        return false;
    };
    let Some(target_piece) = registry.get(target_node.piece_id.as_str()) else {
        return true;
    };
    let Some(param_def) = target_piece
        .def()
        .params
        .iter()
        .find(|param| param.id == edge.to_param)
    else {
        return true;
    };
    let expected = adjacent_in_direction(
        &edge.to_node,
        &node_param_side(target_node, edge.to_param.as_str(), param_def.side),
    );
    expected == edge.from
}

fn node_param_side(node: &Node, param_id: &str, fallback: TileSide) -> TileSide {
    node.input_sides.get(param_id).copied().unwrap_or(fallback)
}

fn node_output_side(node: &Node, piece: &PieceDef) -> Option<TileSide> {
    if piece.output_type.is_none() {
        None
    } else {
        node.output_side.or(piece.output_side)
    }
}

fn prune_invalid_edges_for_node(
    graph: &mut Graph,
    registry: &PieceRegistry,
    node_pos: &GridPos,
    removed_edges: &mut Vec<Edge>,
) {
    let mut remove_ids = Vec::new();
    for (edge_id, edge) in &graph.edges {
        if (edge.from == *node_pos || edge.to_node == *node_pos)
            && !edge_is_still_adjacent(edge, graph, registry)
        {
            remove_ids.push(edge_id.clone());
        }
    }
    for edge_id in remove_ids {
        if let Some(edge) = graph.edges.remove(&edge_id) {
            removed_edges.push(edge);
        }
    }
}

fn edge_connect_from(edge: &Edge) -> GraphOp {
    GraphOp::EdgeConnect {
        edge_id: Some(edge.id.clone()),
        from: edge.from.clone(),
        to_node: edge.to_node.clone(),
        to_param: edge.to_param.clone(),
    }
}

pub fn apply_ops_to_graph(
    graph: &mut Graph,
    registry: &PieceRegistry,
    ops: &[GraphOp],
) -> Result<ApplyOpsOutcome, Vec<Diagnostic>> {
    let mut errors = Vec::<Diagnostic>::new();
    let mut outcome = ApplyOpsOutcome::default();
    let mut inverse_chunks = Vec::<Vec<GraphOp>>::new();

    for op in ops {
        match op {
            GraphOp::NodePlace {
                position,
                piece_id,
                inline_params,
            } => {
                if !ensure_in_bounds(&mut errors, position, "node_place") {
                    continue;
                }
                if graph.nodes.contains_key(position) {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!(
                            "node already exists at ({}, {})",
                            position.col, position.row
                        ),
                    ));
                    continue;
                }
                let Some(piece) = registry.get(piece_id.as_str()) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown piece id '{}'", piece_id),
                    ));
                    continue;
                };
                let mut inline_is_valid = true;
                for (param_id, value) in inline_params {
                    let Some(param_def) = piece
                        .def()
                        .params
                        .iter()
                        .find(|param| param.id == *param_id)
                    else {
                        errors.push(invalid_op(
                            Some(position.clone()),
                            format!("unknown inline param '{}'", param_id),
                        ));
                        inline_is_valid = false;
                        continue;
                    };
                    if !param_def.schema.can_inline() {
                        errors.push(invalid_op(
                            Some(position.clone()),
                            format!("inline value is not allowed for '{}'", param_id),
                        ));
                        inline_is_valid = false;
                        continue;
                    }
                    if !param_def.schema.validate_inline_value(value) {
                        errors.push(invalid_op(
                            Some(position.clone()),
                            format!("inline value has wrong type for '{}'", param_id),
                        ));
                        inline_is_valid = false;
                    }
                }
                if !inline_is_valid {
                    continue;
                }
                graph.nodes.insert(
                    position.clone(),
                    Node {
                        piece_id: piece_id.clone(),
                        inline_params: inline_params.clone(),
                        input_sides: Default::default(),
                        output_side: None,
                    },
                );
                outcome.applied_ops.push(GraphOp::NodePlace {
                    position: position.clone(),
                    piece_id: piece_id.clone(),
                    inline_params: inline_params.clone(),
                });
                inverse_chunks.push(vec![GraphOp::NodeRemove {
                    position: position.clone(),
                }]);
            }
            GraphOp::NodeMove { from, to } => {
                if from == to {
                    continue;
                }
                if !ensure_in_bounds(&mut errors, from, "node_move source") {
                    continue;
                }
                if !ensure_in_bounds(&mut errors, to, "node_move target") {
                    continue;
                }
                if graph.nodes.contains_key(to) {
                    errors.push(invalid_op(
                        Some(to.clone()),
                        format!("target cell already occupied at ({}, {})", to.col, to.row),
                    ));
                    continue;
                }
                let Some(node) = graph.nodes.remove(from) else {
                    errors.push(invalid_op(
                        Some(from.clone()),
                        format!("missing source node at ({}, {})", from.col, from.row),
                    ));
                    continue;
                };
                graph.nodes.insert(to.clone(), node);

                for edge in graph.edges.values_mut() {
                    if edge.from == *from {
                        edge.from = to.clone();
                    }
                    if edge.to_node == *from {
                        edge.to_node = to.clone();
                    }
                }
                let mut removed_for_move = Vec::new();
                prune_invalid_edges_for_node(graph, registry, to, &mut removed_for_move);
                outcome
                    .removed_edges
                    .extend(removed_for_move.iter().cloned());

                let mut inverse = vec![GraphOp::NodeMove {
                    from: to.clone(),
                    to: from.clone(),
                }];
                for removed in &removed_for_move {
                    let mut restored = removed.clone();
                    if restored.from == *to {
                        restored.from = from.clone();
                    }
                    if restored.to_node == *to {
                        restored.to_node = from.clone();
                    }
                    inverse.push(edge_connect_from(&restored));
                }
                inverse_chunks.push(inverse);

                outcome.applied_ops.push(GraphOp::NodeMove {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
            GraphOp::NodeRemove { position } => {
                if !ensure_in_bounds(&mut errors, position, "node_remove") {
                    continue;
                }
                let Some(removed_node) = graph.nodes.remove(position) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("missing node at ({}, {})", position.col, position.row),
                    ));
                    continue;
                };
                let mut remove_ids = Vec::new();
                for (edge_id, edge) in &graph.edges {
                    if edge.from == *position || edge.to_node == *position {
                        remove_ids.push(edge_id.clone());
                    }
                }
                let mut removed_for_node = Vec::new();
                for edge_id in remove_ids {
                    if let Some(edge) = graph.edges.remove(&edge_id) {
                        removed_for_node.push(edge);
                    }
                }
                outcome
                    .removed_edges
                    .extend(removed_for_node.iter().cloned());

                let mut inverse = vec![GraphOp::NodePlace {
                    position: position.clone(),
                    piece_id: removed_node.piece_id.clone(),
                    inline_params: removed_node.inline_params.clone(),
                }];
                for (param_id, side) in &removed_node.input_sides {
                    inverse.push(GraphOp::ParamSetSide {
                        position: position.clone(),
                        param_id: param_id.clone(),
                        side: *side,
                    });
                }
                if let Some(side) = removed_node.output_side {
                    inverse.push(GraphOp::OutputSetSide {
                        position: position.clone(),
                        side,
                    });
                }
                inverse.extend(removed_for_node.iter().map(edge_connect_from));
                inverse_chunks.push(inverse);

                outcome.applied_ops.push(GraphOp::NodeRemove {
                    position: position.clone(),
                });
            }
            GraphOp::EdgeConnect {
                edge_id,
                from,
                to_node,
                to_param,
            } => {
                let source_ok = ensure_in_bounds(&mut errors, from, "edge_connect source");
                let target_ok = ensure_in_bounds(&mut errors, to_node, "edge_connect target");
                if !(source_ok && target_ok) {
                    continue;
                }
                if !graph.nodes.contains_key(from) {
                    errors.push(invalid_op(
                        Some(from.clone()),
                        format!("missing source node at ({}, {})", from.col, from.row),
                    ));
                    continue;
                }
                if !graph.nodes.contains_key(to_node) {
                    errors.push(invalid_op(
                        Some(to_node.clone()),
                        format!("missing target node at ({}, {})", to_node.col, to_node.row),
                    ));
                    continue;
                }
                if to_param.trim().is_empty() {
                    errors.push(invalid_op(
                        Some(to_node.clone()),
                        "edge target param cannot be empty",
                    ));
                    continue;
                }
                if graph
                    .edges
                    .values()
                    .any(|edge| edge.to_node == *to_node && edge.to_param == *to_param)
                {
                    errors.push(invalid_op(
                        Some(to_node.clone()),
                        format!("target param '{}' already connected", to_param),
                    ));
                    continue;
                }
                let Some(from_node) = graph.nodes.get(from) else {
                    errors.push(invalid_op(
                        Some(from.clone()),
                        format!("missing source node at ({}, {})", from.col, from.row),
                    ));
                    continue;
                };
                let Some(to_node_ref) = graph.nodes.get(to_node) else {
                    errors.push(invalid_op(
                        Some(to_node.clone()),
                        format!("missing target node at ({}, {})", to_node.col, to_node.row),
                    ));
                    continue;
                };
                let Some(from_piece) = registry.get(from_node.piece_id.as_str()) else {
                    errors.push(invalid_op(
                        Some(from.clone()),
                        format!("unknown source piece '{}'", from_node.piece_id),
                    ));
                    continue;
                };
                let Some(to_piece) = registry.get(to_node_ref.piece_id.as_str()) else {
                    errors.push(invalid_op(
                        Some(to_node.clone()),
                        format!("unknown target piece '{}'", to_node_ref.piece_id),
                    ));
                    continue;
                };
                let Some(param_def) = to_piece
                    .def()
                    .params
                    .iter()
                    .find(|param| param.id == *to_param)
                else {
                    errors.push(invalid_op(
                        Some(to_node.clone()),
                        format!("unknown target param '{}'", to_param),
                    ));
                    continue;
                };
                let target_side = node_param_side(to_node_ref, to_param.as_str(), param_def.side);
                let expected = adjacent_in_direction(to_node, &target_side);
                if expected != *from {
                    errors.push(invalid_op(
                        Some(to_node.clone()),
                        format!(
                            "edge must come from adjacent {:?} cell ({}, {})",
                            target_side, expected.col, expected.row
                        ),
                    ));
                    continue;
                }
                if let Some(output_side) = node_output_side(from_node, from_piece.def()) {
                    if !output_side.faces(target_side) {
                        errors.push(invalid_op(
                            Some(to_node.clone()),
                            format!(
                                "side mismatch: source {:?} does not face target {:?}",
                                output_side, target_side
                            ),
                        ));
                        continue;
                    }
                }
                let Some(output_type) = from_piece.def().output_type.as_ref() else {
                    errors.push(invalid_op(
                        Some(from.clone()),
                        "cannot connect output from terminal piece",
                    ));
                    continue;
                };
                if !param_def.schema.accepts(output_type) {
                    errors.push(invalid_op(
                        Some(to_node.clone()),
                        format!(
                            "type mismatch: expected {:?}, got {:?}",
                            param_def.schema.expected_port_type(),
                            output_type
                        ),
                    ));
                    continue;
                }
                let edge_id = edge_id.clone().unwrap_or_else(EdgeId::new);
                if graph.edges.contains_key(&edge_id) {
                    errors.push(invalid_op(
                        Some(to_node.clone()),
                        format!("edge id '{}' already exists", edge_id.0),
                    ));
                    continue;
                }
                let edge = Edge {
                    id: edge_id.clone(),
                    from: from.clone(),
                    to_node: to_node.clone(),
                    to_param: to_param.clone(),
                };
                graph.edges.insert(edge.id.clone(), edge);
                outcome.applied_ops.push(GraphOp::EdgeConnect {
                    edge_id: Some(edge_id.clone()),
                    from: from.clone(),
                    to_node: to_node.clone(),
                    to_param: to_param.clone(),
                });
                inverse_chunks.push(vec![GraphOp::EdgeDisconnect { edge_id }]);
            }
            GraphOp::EdgeDisconnect { edge_id } => {
                let Some(disconnected) = graph.edges.remove(edge_id) else {
                    errors.push(invalid_op(None, format!("missing edge '{}'", edge_id.0)));
                    continue;
                };
                outcome.applied_ops.push(GraphOp::EdgeDisconnect {
                    edge_id: edge_id.clone(),
                });
                inverse_chunks.push(vec![edge_connect_from(&disconnected)]);
            }
            GraphOp::ParamSetInline {
                position,
                param_id,
                value,
            } => {
                if !ensure_in_bounds(&mut errors, position, "param_set_inline") {
                    continue;
                }
                let Some(target_node) = graph.nodes.get_mut(position) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("missing node at ({}, {})", position.col, position.row),
                    ));
                    continue;
                };
                let Some(piece) = registry.get(target_node.piece_id.as_str()) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown piece '{}'", target_node.piece_id),
                    ));
                    continue;
                };
                let Some(param_def) = piece.def().params.iter().find(|item| item.id == *param_id)
                else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown param '{}'", param_id),
                    ));
                    continue;
                };
                if !param_def.schema.can_inline() {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("inline value is not allowed for '{}'", param_id),
                    ));
                    continue;
                }
                if !param_def.schema.validate_inline_value(value) {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("inline value has wrong type for '{}'", param_id),
                    ));
                    continue;
                }
                if target_node
                    .inline_params
                    .get(param_id)
                    .is_some_and(|existing| existing == value)
                {
                    continue;
                }
                let previous = target_node
                    .inline_params
                    .insert(param_id.clone(), value.clone());
                outcome.applied_ops.push(GraphOp::ParamSetInline {
                    position: position.clone(),
                    param_id: param_id.clone(),
                    value: value.clone(),
                });
                if let Some(previous) = previous {
                    inverse_chunks.push(vec![GraphOp::ParamSetInline {
                        position: position.clone(),
                        param_id: param_id.clone(),
                        value: previous,
                    }]);
                } else {
                    inverse_chunks.push(vec![GraphOp::ParamClearInline {
                        position: position.clone(),
                        param_id: param_id.clone(),
                    }]);
                }
            }
            GraphOp::ParamClearInline { position, param_id } => {
                if !ensure_in_bounds(&mut errors, position, "param_clear_inline") {
                    continue;
                }
                let Some(target_node) = graph.nodes.get_mut(position) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("missing node at ({}, {})", position.col, position.row),
                    ));
                    continue;
                };
                let Some(piece) = registry.get(target_node.piece_id.as_str()) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown piece '{}'", target_node.piece_id),
                    ));
                    continue;
                };
                if !piece.def().params.iter().any(|item| item.id == *param_id) {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown param '{}'", param_id),
                    ));
                    continue;
                }
                let previous = target_node.inline_params.remove(param_id);
                let Some(previous) = previous else {
                    continue;
                };
                outcome.applied_ops.push(GraphOp::ParamClearInline {
                    position: position.clone(),
                    param_id: param_id.clone(),
                });
                inverse_chunks.push(vec![GraphOp::ParamSetInline {
                    position: position.clone(),
                    param_id: param_id.clone(),
                    value: previous,
                }]);
            }
            GraphOp::ParamSetSide {
                position,
                param_id,
                side,
            } => {
                if !ensure_in_bounds(&mut errors, position, "param_set_side") {
                    continue;
                }
                let Some(target_node) = graph.nodes.get_mut(position) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("missing node at ({}, {})", position.col, position.row),
                    ));
                    continue;
                };
                let Some(piece) = registry.get(target_node.piece_id.as_str()) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown piece '{}'", target_node.piece_id),
                    ));
                    continue;
                };
                if !piece.def().params.iter().any(|param| param.id == *param_id) {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown param '{}'", param_id),
                    ));
                    continue;
                }
                if target_node
                    .input_sides
                    .get(param_id)
                    .is_some_and(|existing| existing == side)
                {
                    continue;
                }
                let previous = target_node.input_sides.insert(param_id.clone(), *side);
                outcome.applied_ops.push(GraphOp::ParamSetSide {
                    position: position.clone(),
                    param_id: param_id.clone(),
                    side: *side,
                });
                if let Some(previous) = previous {
                    inverse_chunks.push(vec![GraphOp::ParamSetSide {
                        position: position.clone(),
                        param_id: param_id.clone(),
                        side: previous,
                    }]);
                } else {
                    inverse_chunks.push(vec![GraphOp::ParamClearSide {
                        position: position.clone(),
                        param_id: param_id.clone(),
                    }]);
                }
            }
            GraphOp::ParamClearSide { position, param_id } => {
                if !ensure_in_bounds(&mut errors, position, "param_clear_side") {
                    continue;
                }
                let Some(target_node) = graph.nodes.get_mut(position) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("missing node at ({}, {})", position.col, position.row),
                    ));
                    continue;
                };
                let Some(piece) = registry.get(target_node.piece_id.as_str()) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown piece '{}'", target_node.piece_id),
                    ));
                    continue;
                };
                if !piece.def().params.iter().any(|param| param.id == *param_id) {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown param '{}'", param_id),
                    ));
                    continue;
                }
                let previous = target_node.input_sides.remove(param_id);
                let Some(previous) = previous else {
                    continue;
                };
                outcome.applied_ops.push(GraphOp::ParamClearSide {
                    position: position.clone(),
                    param_id: param_id.clone(),
                });
                inverse_chunks.push(vec![GraphOp::ParamSetSide {
                    position: position.clone(),
                    param_id: param_id.clone(),
                    side: previous,
                }]);
            }
            GraphOp::OutputSetSide { position, side } => {
                if !ensure_in_bounds(&mut errors, position, "output_set_side") {
                    continue;
                }
                let Some(target_node) = graph.nodes.get_mut(position) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("missing node at ({}, {})", position.col, position.row),
                    ));
                    continue;
                };
                let Some(piece) = registry.get(target_node.piece_id.as_str()) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown piece '{}'", target_node.piece_id),
                    ));
                    continue;
                };
                if piece.def().output_type.is_none() {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        "cannot set output side on terminal piece",
                    ));
                    continue;
                }
                if target_node.output_side == Some(*side) {
                    continue;
                }
                let previous = target_node.output_side.replace(*side);
                outcome.applied_ops.push(GraphOp::OutputSetSide {
                    position: position.clone(),
                    side: *side,
                });
                if let Some(previous) = previous {
                    inverse_chunks.push(vec![GraphOp::OutputSetSide {
                        position: position.clone(),
                        side: previous,
                    }]);
                } else {
                    inverse_chunks.push(vec![GraphOp::OutputClearSide {
                        position: position.clone(),
                    }]);
                }
            }
            GraphOp::OutputClearSide { position } => {
                if !ensure_in_bounds(&mut errors, position, "output_clear_side") {
                    continue;
                }
                let Some(target_node) = graph.nodes.get_mut(position) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("missing node at ({}, {})", position.col, position.row),
                    ));
                    continue;
                };
                let Some(piece) = registry.get(target_node.piece_id.as_str()) else {
                    errors.push(invalid_op(
                        Some(position.clone()),
                        format!("unknown piece '{}'", target_node.piece_id),
                    ));
                    continue;
                };
                if piece.def().output_type.is_none() {
                    continue;
                }
                let previous = target_node.output_side.take();
                let Some(previous) = previous else {
                    continue;
                };
                outcome.applied_ops.push(GraphOp::OutputClearSide {
                    position: position.clone(),
                });
                inverse_chunks.push(vec![GraphOp::OutputSetSide {
                    position: position.clone(),
                    side: previous,
                }]);
            }
        }
    }

    if errors.is_empty() {
        outcome.undo_ops = inverse_chunks
            .into_iter()
            .rev()
            .flat_map(|chunk| chunk.into_iter())
            .collect();
        Ok(outcome)
    } else {
        Err(errors)
    }
}
