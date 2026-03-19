use super::*;

pub(super) fn toggle_picker_panel(state: &mut EditorShellState) {
    state.picker_open = !state.picker_open;
    if state.picker_open {
        close_command_palette(state);
        state.picker_target = preferred_picker_target(state);
        if let Some(target) = state.picker_target {
            state.selected_cell = Some(target);
            state.selected_node = None;
        }
    } else {
        state.picker_target = None;
    }
}

pub(super) async fn place_piece(
    mut state: Signal<EditorShellState>,
    piece_id: String,
    target: Option<GridPos>,
) {
    if !state.read().workspace_mode.picker_allowed() {
        let mut current = state.write();
        close_picker(&mut current);
        current.status_message =
            Some("Tile placement is only available in Runtime or Trick workspaces.".to_string());
        return;
    }

    let position = target
        .or_else(|| state.read().picker_target)
        .or_else(|| first_free_position(&state.read().graph));

    let Some(position) = position else {
        state.write().status_message = Some("No free grid cell is available.".to_string());
        return;
    };

    if is_cell_occupied(&state.read().graph, &position) {
        state.write().status_message = Some("Selected cell is already occupied.".to_string());
        return;
    }

    apply_graph_ops(
        state,
        "native_place_piece",
        vec![GraphOp::NodePlace {
            position,
            piece_id,
            inline_params: BTreeMap::new(),
        }],
        Some(position),
    )
    .await;
}

pub(super) async fn delete_selected_node(mut state: Signal<EditorShellState>) {
    let Some(position) = state.read().selected_node else {
        return;
    };

    state.write().inspector_open = false;
    apply_graph_ops(
        state,
        "native_delete_node",
        vec![GraphOp::NodeRemove { position }],
        None,
    )
    .await;
}

pub(super) async fn move_node(state: Signal<EditorShellState>, from: GridPos, to: GridPos) {
    if from == to {
        return;
    }

    let occupied = is_cell_occupied(&state.read().graph, &to);
    let mut ops = if occupied {
        vec![GraphOp::NodeSwap { a: from, b: to }]
    } else {
        vec![GraphOp::NodeMove { from, to }]
    };
    ops.push(GraphOp::NodeAutoWire { position: to });
    if occupied {
        ops.push(GraphOp::NodeAutoWire { position: from });
    }

    apply_graph_ops(
        state,
        if occupied {
            "native_swap_node"
        } else {
            "native_move_node"
        },
        ops,
        Some(to),
    )
    .await;
}

pub(super) async fn connect_nodes(
    mut state: Signal<EditorShellState>,
    from: GridPos,
    to_node: GridPos,
) {
    if from == to_node {
        return;
    }

    if !state.read().tauri_available {
        let mut current = state.write();
        current.pending_probe = None;
        current.status_message = Some(BACKEND_REQUIRED_MESSAGE.to_string());
        return;
    }

    let graph_target = state.read().workspace_mode.graph_target();
    match tauri::graph_pick_target_param(from, to_node, graph_target, None).await {
        Ok(probe) => {
            let Some(to_param) = probe.to_param else {
                let mut current = state.write();
                current.pending_probe = Some(probe.clone());
                current.status_message = Some(
                    probe
                        .detail
                        .clone()
                        .or_else(|| probe.reason.as_ref().map(|reason| format!("{reason:?}")))
                        .unwrap_or_else(|| "Unable to connect the selected tiles.".to_string()),
                );
                return;
            };
            state.write().pending_probe = None;
            apply_graph_ops(
                state,
                "native_connect_edge",
                vec![GraphOp::EdgeConnect {
                    edge_id: None,
                    from,
                    to_node,
                    to_param,
                }],
                Some(to_node),
            )
            .await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

pub(super) async fn apply_graph_ops(
    mut state: Signal<EditorShellState>,
    request_id: &str,
    ops: Vec<GraphOp>,
    next_selected: Option<GridPos>,
) {
    if ops.is_empty() {
        return;
    }

    if !state.read().tauri_available {
        let mut current = state.write();
        current.pending_probe = None;
        current.status_message = Some(BACKEND_REQUIRED_MESSAGE.to_string());
        return;
    }

    let graph_target = state.read().workspace_mode.graph_target();
    let previous = state.read().clone();
    {
        let mut current = state.write();
        if let Err(error) = apply_graph_ops_locally(&mut current, &ops, next_selected) {
            current.status_message = Some(error);
            return;
        }
        current.pending_probe = None;
    }
    match tauri::graph_apply_ops(ops, Some(request_id.to_string()), graph_target).await {
        Ok(result) => {
            let mut current = state.write();
            apply_graph_result(&mut current, result, next_selected);
            drop(current);
            schedule_recovery_write(state);
            refresh_runtime_bridge_state(state);
        }
        Err(error) => {
            let mut rollback = previous;
            close_picker(&mut rollback);
            clear_drag_state(&mut rollback);
            rollback.status_message = Some(error);
            state.set(rollback);
        }
    }
}

pub(super) async fn upsert_sample_load(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        state.write().status_message =
            Some("Sample loads require the desktop backend.".to_string());
        return;
    }

    let sample_id = state.read().sample_id_input.trim().to_string();
    let source = state.read().sample_source_input.trim().to_string();
    if sample_id.is_empty() || source.is_empty() {
        state.write().status_message =
            Some("Sample loads need both an id and a source.".to_string());
        return;
    }

    let aliases_input = state.read().sample_aliases_input.clone();
    let aliases = if aliases_input.trim().is_empty() {
        BTreeMap::new()
    } else {
        match serde_json::from_str::<BTreeMap<String, String>>(&aliases_input) {
            Ok(value) => value,
            Err(error) => {
                state.write().status_message =
                    Some(format!("Sample aliases must be valid JSON: {error}"));
                return;
            }
        }
    };

    apply_init_ops(
        state,
        vec![InitStageOp::SampleLoadUpsert {
            id: sample_id,
            source,
            aliases,
        }],
    )
    .await;
}

pub(super) async fn create_trick(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        state.write().status_message =
            Some("Trick creation requires the desktop backend.".to_string());
        return;
    }

    let trick_name = state.read().trick_name_input.trim().to_string();
    if trick_name.is_empty() {
        state.write().status_message = Some("Trick name cannot be empty.".to_string());
        return;
    }

    let trick_id = slugify_identifier(&trick_name);
    apply_init_ops(
        state,
        vec![InitStageOp::TrickCreate {
            id: trick_id,
            name: trick_name,
            graph: None,
        }],
    )
    .await;
}

pub(super) async fn apply_init_ops(mut state: Signal<EditorShellState>, ops: Vec<InitStageOp>) {
    if ops.is_empty() {
        return;
    }
    match tauri::project_init_apply(ops).await {
        Ok(_) => {
            state.write().sample_id_input.clear();
            state.write().sample_source_input.clear();
            state.write().sample_aliases_input.clear();
            state.write().trick_name_input.clear();
            load_snapshot(state, false).await;
            schedule_recovery_write(state);
            refresh_runtime_bridge_state(state);
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

pub(super) async fn set_param_side(
    state: Signal<EditorShellState>,
    position: GridPos,
    param_id: String,
    side: String,
) {
    let next_selected = state.read().selected_node;
    apply_graph_ops(
        state,
        "native_param_set_side",
        vec![GraphOp::ParamSetSide {
            position,
            param_id,
            side,
        }],
        next_selected,
    )
    .await;
}

pub(super) async fn set_output_side(
    state: Signal<EditorShellState>,
    position: GridPos,
    side: String,
) {
    let next_selected = state.read().selected_node;
    apply_graph_ops(
        state,
        "native_output_set_side",
        vec![GraphOp::OutputSetSide { position, side }],
        next_selected,
    )
    .await;
}

pub(super) async fn disconnect_edge(state: Signal<EditorShellState>, edge_id: String) {
    let next_selected = state.read().selected_node;
    apply_graph_ops(
        state,
        "native_disconnect_edge",
        vec![GraphOp::EdgeDisconnect {
            edge_id: tauri::EdgeId(edge_id),
        }],
        next_selected,
    )
    .await;
}

pub(super) fn normalize_graph_view(raw: &tauri::GraphSnapshotDto) -> GraphView {
    let mut nodes = raw
        .nodes
        .iter()
        .map(|node| GraphNodeView {
            position: node.position,
            piece_id: node.piece_id.clone(),
            inline_params: node.inline_params.clone(),
            input_sides: node.input_sides.clone(),
            output_side: node.output_side.clone(),
            label: node.label.clone(),
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

pub(super) fn apply_graph_result(
    current: &mut EditorShellState,
    result: tauri::GraphApplyResultDto,
    next_selected: Option<GridPos>,
) {
    current.graph = normalize_graph_view(&result.graph);
    if matches!(current.workspace_mode, WorkspaceMode::Runtime) {
        current.project.node_count = current.graph.nodes.len();
        current.project.edge_count = current.graph.edges.len();
    }
    current.project.dirty = true;
    current.graph_preview = result.preview;
    close_picker(current);
    clear_drag_state(current);
    current.selected_node = next_selected
        .filter(|pos| current.graph.nodes.iter().any(|node| &node.position == pos))
        .or_else(|| current.graph.nodes.first().map(|node| node.position));
    current.selected_cell = next_selected
        .or(current.selected_node)
        .or_else(|| first_free_position(&current.graph));
    current.status_message = None;
    sync_editor_inputs(current);
}

pub(super) fn apply_graph_ops_locally(
    current: &mut EditorShellState,
    ops: &[GraphOp],
    next_selected: Option<GridPos>,
) -> Result<(), String> {
    for op in ops {
        match op {
            GraphOp::NodePlace {
                position,
                piece_id,
                inline_params,
            } => {
                if is_cell_occupied(&current.graph, position) {
                    return Err("Selected cell is already occupied.".to_string());
                }
                let piece = piece_def_for_id(&current.catalog, piece_id);
                current.graph.nodes.push(GraphNodeView {
                    position: *position,
                    piece_id: piece_id.clone(),
                    inline_params: inline_params.clone(),
                    input_sides: default_input_sides(piece),
                    output_side: piece.and_then(|piece| piece.output_side.clone()),
                    label: None,
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
            GraphOp::NodeAutoWire { .. } | GraphOp::NodeSetState { .. } => {}
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
    close_picker(current);
    clear_drag_state(current);
    current.selected_node = next_selected
        .filter(|pos| current.graph.nodes.iter().any(|node| &node.position == pos))
        .or_else(|| current.graph.nodes.first().map(|node| node.position));
    current.selected_cell = next_selected
        .or(current.selected_node)
        .or_else(|| first_free_position(&current.graph));
    sync_editor_inputs(current);
    Ok(())
}

pub(super) fn clear_drag_state(state: &mut EditorShellState) {
    state.drag_session = None;
    state.drag_hover = None;
}

pub(super) fn close_picker(state: &mut EditorShellState) {
    state.picker_open = false;
    state.picker_target = None;
}
