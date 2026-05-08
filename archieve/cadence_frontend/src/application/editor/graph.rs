use std::collections::BTreeMap;

use crate::adapter::backend;
use crate::adapter::{GraphOp, GridPos, ParamDef, PieceDef};
use crate::domain::{
    GraphEdgeView, GraphNodeView, GraphView, canonical_side, first_free_position, is_cell_occupied,
    remap_edges_for_position, swap_edges_for_positions,
};

use super::{EditorState, ResolvedParamAuthoringMode, ResolvedTargetParamBinding, WorkspaceMode};

pub fn normalize_graph_view(raw: &backend::GraphSnapshotDto) -> GraphView {
    let mut nodes = raw
        .nodes
        .iter()
        .map(|node| GraphNodeView {
            position: node.position,
            piece_id: node.piece_id.clone(),
            inline_params: node.inline_params.clone(),
            pattern_source: node.pattern_source.clone(),
            input_sides: node.input_sides.clone(),
            output_side: node.output_side.clone(),
            label: node.label.clone(),
            node_state: node.node_state.clone(),
        })
        .collect::<Vec<_>>();
    nodes.sort_by_key(|node| (node.position.row, node.position.col));

    let mut edges = raw
        .edges
        .iter()
        .map(|edge| GraphEdgeView {
            id: edge.id.clone(),
            from: edge.from,
            to_node: edge.to_node,
            to_param: edge.to_param.clone(),
        })
        .collect::<Vec<_>>();
    edges.sort_by_key(|edge| {
        (
            edge.from.row,
            edge.from.col,
            edge.to_node.row,
            edge.to_node.col,
        )
    });

    GraphView {
        name: raw.name.clone(),
        cols: raw.cols.max(1),
        rows: raw.rows.max(1),
        nodes,
        edges,
    }
}

pub fn selected_node_view(state: &EditorState) -> Option<&GraphNodeView> {
    let selected = state.selected_node.as_ref()?;
    state
        .graph
        .nodes
        .iter()
        .find(|node| &node.position == selected)
}

pub fn normalize_selected_nodes(state: &mut EditorState) {
    state
        .selected_nodes
        .retain(|pos| state.graph.nodes.iter().any(|node| node.position == *pos));

    if let Some(primary) = state.selected_node {
        if !state.selected_nodes.contains(&primary) {
            state.selected_nodes.insert(0, primary);
        }
    } else {
        state.selected_node = state.selected_nodes.first().copied();
    }

    if state.selected_node.is_none() {
        state.selected_node = state.selected_nodes.first().copied();
    }
}

pub fn piece_def_for_id<'a>(catalog: &'a [PieceDef], piece_id: &str) -> Option<&'a PieceDef> {
    catalog.iter().find(|piece| piece.id == piece_id)
}

pub fn selected_piece_def(state: &EditorState) -> Option<&PieceDef> {
    let node = selected_node_view(state)?;
    piece_def_for_id(&state.catalog, &node.piece_id)
}

pub fn direct_downstream_param<'a>(
    state: &'a EditorState,
    position: GridPos,
) -> Option<(
    &'a GraphNodeView,
    &'a PieceDef,
    &'a ParamDef,
    &'a GraphEdgeView,
)> {
    let edge = state
        .graph
        .edges
        .iter()
        .find(|edge| edge.from == position)?;
    let target_node = state
        .graph
        .nodes
        .iter()
        .find(|node| node.position == edge.to_node)?;
    let target_piece = piece_def_for_id(&state.catalog, &target_node.piece_id)?;
    let target_param = target_piece
        .params
        .iter()
        .find(|param| param.id == edge.to_param)?;
    Some((target_node, target_piece, target_param, edge))
}

fn connector_bundle_signature<'a>(
    state: &'a EditorState,
    position: GridPos,
) -> Option<(
    &'a GraphNodeView,
    &'a PieceDef,
    &'a crate::adapter::backend::BundleInputDef,
)> {
    let edge = state
        .graph
        .edges
        .iter()
        .find(|edge| edge.from == position)?;
    let target_node = state
        .graph
        .nodes
        .iter()
        .find(|node| node.position == edge.to_node)?;
    let target_piece = piece_def_for_id(&state.catalog, &target_node.piece_id)?;
    let bundle = target_piece.bundle_input.as_ref()?;
    (bundle.param_id == edge.to_param).then_some((target_node, target_piece, bundle))
}

pub fn resolve_target_param_binding(
    state: &EditorState,
    position: GridPos,
) -> Option<ResolvedTargetParamBinding> {
    let (_target_node, target_piece, target_param, edge) =
        direct_downstream_param(state, position)?;
    if target_piece.id == "args_connector" {
        let (_bundle_target_node, bundle_target_piece, bundle) =
            connector_bundle_signature(state, edge.to_node)?;
        let slot_index = edge
            .to_param
            .strip_prefix("arg")
            .and_then(|raw| raw.parse::<usize>().ok())?
            .saturating_sub(1);
        let slot = bundle.slots.get(slot_index)?;
        let target_param = bundle_target_piece
            .params
            .iter()
            .find(|param| param.id == slot.param_id)?;
        return Some(ResolvedTargetParamBinding {
            target_piece_id: bundle_target_piece.id.clone(),
            target_piece_label: bundle_target_piece.label.clone(),
            target_param_id: target_param.id.clone(),
            target_param_label: target_param.label.clone(),
            slot_label: Some(slot.label.clone()),
            mode: resolve_param_authoring_mode(target_param),
        });
    }

    Some(ResolvedTargetParamBinding {
        target_piece_id: target_piece.id.clone(),
        target_piece_label: target_piece.label.clone(),
        target_param_id: target_param.id.clone(),
        target_param_label: target_param.label.clone(),
        slot_label: None,
        mode: resolve_param_authoring_mode(target_param),
    })
}

pub fn resolve_param_authoring_mode(param: &ParamDef) -> ResolvedParamAuthoringMode {
    if param.id == "pattern" {
        ResolvedParamAuthoringMode::PatternPort
    } else {
        ResolvedParamAuthoringMode::ControlValue
    }
}

pub fn apply_graph_result(
    current: &mut EditorState,
    result: backend::GraphApplyResultDto,
    next_selected: Option<GridPos>,
    next_selected_nodes: Option<Vec<GridPos>>,
) {
    current.graph = normalize_graph_view(&result.graph);
    if matches!(current.workspace_mode, WorkspaceMode::Runtime) {
        current.project.node_count = current.graph.nodes.len();
        current.project.edge_count = current.graph.edges.len();
    }
    current.project.dirty = true;
    current.graph_preview = result.preview;
    current.selected_node = next_selected
        .filter(|pos| current.graph.nodes.iter().any(|node| &node.position == pos))
        .or_else(|| current.graph.nodes.first().map(|node| node.position));
    current.selected_nodes =
        next_selected_nodes.unwrap_or_else(|| current.selected_node.into_iter().collect());
    normalize_selected_nodes(current);
    current.selected_cell = next_selected
        .or(current.selected_node)
        .or_else(|| first_free_position(&current.graph));
}

pub fn apply_graph_ops_locally(
    current: &mut EditorState,
    ops: &[GraphOp],
    next_selected: Option<GridPos>,
    next_selected_nodes: Option<Vec<GridPos>>,
) -> Result<(), String> {
    for op in ops {
        match op {
            GraphOp::NodePlace {
                position,
                piece_id,
                inline_params,
                pattern_source,
            } => {
                if is_cell_occupied(&current.graph, position) {
                    return Err("Selected cell is already occupied.".to_string());
                }
                let piece = piece_def_for_id(&current.catalog, piece_id);
                current.graph.nodes.push(GraphNodeView {
                    position: *position,
                    piece_id: piece_id.clone(),
                    inline_params: inline_params.clone(),
                    pattern_source: pattern_source.clone(),
                    input_sides: default_input_sides(piece),
                    output_side: piece.and_then(|piece| piece.output_side.clone()),
                    label: None,
                    node_state: None,
                });
            }
            GraphOp::NodeMove { from, to } => {
                if is_cell_occupied(&current.graph, to) {
                    return Err("Selected cell is already occupied.".to_string());
                }
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *from)
                else {
                    return Err("Source tile is missing.".to_string());
                };
                node.position = *to;
                remap_edges_for_position(&mut current.graph.edges, from, to);
            }
            GraphOp::NodeSwap { a, b } => {
                let a_index = current
                    .graph
                    .nodes
                    .iter()
                    .position(|node| node.position == *a);
                let b_index = current
                    .graph
                    .nodes
                    .iter()
                    .position(|node| node.position == *b);
                let (Some(a_index), Some(b_index)) = (a_index, b_index) else {
                    return Err("Swap target is missing.".to_string());
                };
                current.graph.nodes[a_index].position = *b;
                current.graph.nodes[b_index].position = *a;
                swap_edges_for_positions(&mut current.graph.edges, a, b);
            }
            GraphOp::NodeRemove { position } => {
                current
                    .graph
                    .nodes
                    .retain(|node| node.position != *position);
                current
                    .graph
                    .edges
                    .retain(|edge| edge.from != *position && edge.to_node != *position);
            }
            GraphOp::EdgeConnect {
                edge_id: _,
                from,
                to_node,
                to_param,
            } => {
                if current.graph.edges.iter().any(|edge| {
                    edge.from == *from && edge.to_node == *to_node && edge.to_param == *to_param
                }) {
                    continue;
                }
                current
                    .graph
                    .edges
                    .retain(|edge| !(edge.to_node == *to_node && edge.to_param == *to_param));
                current.graph.edges.push(GraphEdgeView {
                    id: format!(
                        "edge-{}-{}-{}",
                        from.col,
                        from.row,
                        current.graph.edges.len() + 1
                    ),
                    from: *from,
                    to_node: *to_node,
                    to_param: to_param.clone(),
                });
            }
            GraphOp::EdgeDisconnect { edge_id } => {
                current.graph.edges.retain(|edge| edge.id != edge_id.0);
            }
            GraphOp::ParamSetInline {
                position,
                param_id,
                value,
            } => {
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *position)
                else {
                    return Err("Target tile is missing.".to_string());
                };
                node.inline_params.insert(param_id.clone(), value.clone());
            }
            GraphOp::ParamClearInline { position, param_id } => {
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *position)
                else {
                    return Err("Target tile is missing.".to_string());
                };
                node.inline_params.remove(param_id);
            }
            GraphOp::ParamSetSide {
                position,
                param_id,
                side,
            } => {
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *position)
                else {
                    return Err("Target tile is missing.".to_string());
                };
                node.input_sides.insert(param_id.clone(), side.clone());
            }
            GraphOp::ParamClearSide { position, param_id } => {
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *position)
                else {
                    return Err("Target tile is missing.".to_string());
                };
                node.input_sides.remove(param_id);
            }
            GraphOp::OutputSetSide { position, side } => {
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *position)
                else {
                    return Err("Target tile is missing.".to_string());
                };
                node.output_side = Some(side.clone());
            }
            GraphOp::OutputClearSide { position } => {
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *position)
                else {
                    return Err("Target tile is missing.".to_string());
                };
                node.output_side = None;
            }
            GraphOp::NodeSetLabel { position, label } => {
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *position)
                else {
                    return Err("Target tile is missing.".to_string());
                };
                node.label = label.clone();
            }
            GraphOp::ResizeGrid { cols, rows } => {
                current.graph.cols = (*cols).max(1);
                current.graph.rows = (*rows).max(1);
            }
            GraphOp::NodeSetState { position, state } => {
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *position)
                else {
                    return Err("Target tile is missing.".to_string());
                };
                node.node_state = state.clone();
            }
            GraphOp::NodeSetPatternSurface {
                position,
                pattern_source,
            } => {
                let Some(node) = current
                    .graph
                    .nodes
                    .iter_mut()
                    .find(|node| node.position == *position)
                else {
                    return Err("Target tile is missing.".to_string());
                };
                node.pattern_source = pattern_source.clone();
            }
            GraphOp::NodeAutoWire { .. } => {}
        }
    }

    current
        .graph
        .nodes
        .sort_by_key(|node| (node.position.row, node.position.col));
    current.graph.edges.sort_by_key(|edge| {
        (
            edge.from.row,
            edge.from.col,
            edge.to_node.row,
            edge.to_node.col,
        )
    });
    current.project.node_count = current.graph.nodes.len();
    current.project.edge_count = current.graph.edges.len();
    current.project.dirty = true;
    current.selected_node = next_selected
        .filter(|pos| current.graph.nodes.iter().any(|node| &node.position == pos))
        .or_else(|| current.graph.nodes.first().map(|node| node.position));
    current.selected_nodes =
        next_selected_nodes.unwrap_or_else(|| current.selected_node.into_iter().collect());
    normalize_selected_nodes(current);
    current.selected_cell = next_selected
        .or(current.selected_node)
        .or_else(|| first_free_position(&current.graph));
    Ok(())
}

fn default_input_sides(piece: Option<&PieceDef>) -> BTreeMap<String, String> {
    piece
        .map(|piece| {
            piece
                .params
                .iter()
                .map(|param| {
                    (
                        param.id.clone(),
                        canonical_side(param.side.as_str()).to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}
