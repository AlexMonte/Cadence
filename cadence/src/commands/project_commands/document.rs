use super::*;

pub(super) fn validate_project_name(name: &str) -> AppResult<String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AppError::InvalidInput(
            "project name cannot be empty".to_string(),
        ));
    }
    Ok(normalized.to_string())
}

pub(super) fn default_project_graph(name: &str) -> CadenceProjectDocument {
    let mut nodes = BTreeMap::new();
    nodes.insert(
        GridPos { col: 0, row: 0 },
        Node {
            piece_id: "strudel.note".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("c3".to_string()))]),
            input_sides: Default::default(),
            output_side: Some(tessera::types::TileSide::RIGHT),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "strudel.output".to_string(),
            inline_params: BTreeMap::new(),
            input_sides: BTreeMap::from([("pattern".to_string(), tessera::types::TileSide::LEFT)]),
            output_side: None,
            label: None,
            node_state: None,
        },
    );
    let edge = Edge {
        id: EdgeId::new(),
        from: GridPos { col: 0, row: 0 },
        to_node: GridPos { col: 1, row: 0 },
        to_param: "pattern".to_string(),
    };

    CadenceProjectDocument::new(
        name.to_string(),
        Graph {
            nodes,
            edges: BTreeMap::from([(edge.id.clone(), edge)]),
            name: name.to_string(),
            cols: STANDARD_GRAPH_COLS,
            rows: STANDARD_GRAPH_ROWS,
        },
    )
}

fn normalize_graph_workspace(graph: &mut Graph) -> bool {
    if graph.nodes.is_empty() {
        let changed = graph.cols != STANDARD_GRAPH_COLS || graph.rows != STANDARD_GRAPH_ROWS;
        graph.cols = STANDARD_GRAPH_COLS;
        graph.rows = STANDARD_GRAPH_ROWS;
        return changed;
    }

    let min_col = graph.nodes.keys().map(|pos| pos.col).min().unwrap_or(0);
    let max_col = graph.nodes.keys().map(|pos| pos.col).max().unwrap_or(0);
    let min_row = graph.nodes.keys().map(|pos| pos.row).min().unwrap_or(0);
    let max_row = graph.nodes.keys().map(|pos| pos.row).max().unwrap_or(0);

    let shift_col = -min_col;
    let shift_row = -min_row;
    let bbox_cols = (max_col - min_col + 1) as u32;
    let bbox_rows = (max_row - min_row + 1) as u32;
    let next_cols = STANDARD_GRAPH_COLS.max(bbox_cols);
    let next_rows = STANDARD_GRAPH_ROWS.max(bbox_rows);

    let mut changed = graph.cols != next_cols || graph.rows != next_rows;
    if shift_col != 0 || shift_row != 0 {
        changed = true;
        graph.nodes = graph
            .nodes
            .iter()
            .map(|(pos, node)| {
                (
                    GridPos {
                        col: pos.col + shift_col,
                        row: pos.row + shift_row,
                    },
                    node.clone(),
                )
            })
            .collect();
        for edge in graph.edges.values_mut() {
            edge.from.col += shift_col;
            edge.from.row += shift_row;
            edge.to_node.col += shift_col;
            edge.to_node.row += shift_row;
        }
    }

    graph.cols = next_cols;
    graph.rows = next_rows;
    changed
}

fn normalize_legacy_subgraph_piece_ids(graph: &mut Graph) -> bool {
    let mut changed = false;
    let mut output_positions = Vec::new();
    for node in graph.nodes.values_mut() {
        let next_piece_id = match node.piece_id.as_str() {
            "cadence.trick_input_1" => Some(SUBGRAPH_INPUT_1_ID.to_string()),
            "cadence.trick_input_2" => Some(SUBGRAPH_INPUT_2_ID.to_string()),
            "cadence.trick_input_3" => Some(SUBGRAPH_INPUT_3_ID.to_string()),
            "cadence.trick_output" => Some(SUBGRAPH_OUTPUT_ID.to_string()),
            piece_id if piece_id.starts_with("cadence.trick.") => piece_id
                .strip_prefix("cadence.trick.")
                .map(|suffix| format!("tessera.subgraph.{suffix}")),
            _ => None,
        };
        if let Some(next_piece_id) = next_piece_id
            && node.piece_id != next_piece_id
        {
            node.piece_id = next_piece_id;
            changed = true;
        }
    }
    for (pos, node) in &graph.nodes {
        if node.piece_id == SUBGRAPH_OUTPUT_ID {
            output_positions.push(*pos);
        }
    }
    for edge in graph.edges.values_mut() {
        if edge.to_param == "pattern" && output_positions.iter().any(|pos| pos == &edge.to_node) {
            edge.to_param = "input".to_string();
            changed = true;
        }
    }
    changed
}

fn normalize_project_workspaces(project: &mut CadenceProjectDocument) -> bool {
    let mut changed = normalize_graph_workspace(&mut project.graph);
    changed |= normalize_legacy_subgraph_piece_ids(&mut project.graph);
    for trick in &mut project.init_stage.tricks {
        changed |= normalize_graph_workspace(&mut trick.graph);
        changed |= normalize_legacy_subgraph_piece_ids(&mut trick.graph);
    }
    changed
}

fn validate_graph_bounds(graph: &Graph, context: &str) -> AppResult<()> {
    if graph.cols == 0 || graph.rows == 0 {
        return Err(AppError::InvalidInput(format!(
            "{context} has invalid grid size {}x{} (minimum is 1x1)",
            graph.cols, graph.rows
        )));
    }
    let in_bounds = |pos: &GridPos| {
        (0..graph.cols as i32).contains(&pos.col) && (0..graph.rows as i32).contains(&pos.row)
    };
    if let Some(pos) = graph.nodes.keys().find(|pos| !in_bounds(pos)) {
        return Err(AppError::InvalidInput(format!(
            "{context} contains node outside declared grid bounds at ({}, {}) for grid {}x{}",
            pos.col, pos.row, graph.cols, graph.rows
        )));
    }
    if let Some(edge) = graph
        .edges
        .values()
        .find(|edge| !in_bounds(&edge.from) || !in_bounds(&edge.to_node))
    {
        return Err(AppError::InvalidInput(format!(
            "{context} contains edge outside declared grid bounds: from=({}, {}), to=({}, {}) for grid {}x{}",
            edge.from.col,
            edge.from.row,
            edge.to_node.col,
            edge.to_node.row,
            graph.cols,
            graph.rows
        )));
    }

    Ok(())
}

fn validate_project_document(graph: &CadenceProjectDocument) -> AppResult<()> {
    if graph.schema_version != CadenceProjectDocument::SCHEMA_VERSION {
        return Err(AppError::InvalidInput(format!(
            "unsupported schema_version: {} (expected {})",
            graph.schema_version,
            CadenceProjectDocument::SCHEMA_VERSION,
        )));
    }

    validate_graph_bounds(&graph.graph, "runtime graph")?;
    for trick in &graph.init_stage.tricks {
        validate_graph_bounds(&trick.graph, format!("trick '{}'", trick.name).as_str())?;
    }

    Ok(())
}

pub(super) fn project_dto(graph: &CadenceProjectDocument) -> ProjectDto {
    ProjectDto {
        name: graph.name.clone(),
        node_count: graph.graph.nodes.len(),
        edge_count: graph.graph.edges.len(),
    }
}

pub(super) fn project_to_view(store: &AppStore, graph: &CadenceProjectDocument) -> ProjectViewDto {
    ProjectViewDto {
        name: graph.name.clone(),
        schema_version: graph.schema_version,
        node_count: graph.graph.nodes.len(),
        edge_count: graph.graph.edges.len(),
        dirty: store.dirty,
        path: store
            .current_path
            .as_ref()
            .map(|path| path.display().to_string()),
    }
}

fn project_fingerprint(graph: &CadenceProjectDocument) -> AppResult<String> {
    use std::hash::{Hash, Hasher};

    let payload = serde_json::to_vec(graph)?;
    let mut hasher = DefaultHasher::new();
    payload.hash(&mut hasher);
    Ok(format!("{:016x}", hasher.finish()))
}

pub(super) fn mark_store_clean(store: &mut AppStore) -> AppResult<()> {
    let graph = active_project(store)?;
    store.last_saved_snapshot_hash = Some(project_fingerprint(graph)?);
    store.dirty = false;
    Ok(())
}

pub(super) fn resolve_path(path: &str) -> AppResult<PathBuf> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("path cannot be empty".to_string()));
    }
    Ok(PathBuf::from(trimmed))
}

pub(super) fn deserialize_project_document(
    payload: &str,
) -> AppResult<(CadenceProjectDocument, bool)> {
    let raw: Value = serde_json::from_str(payload)?;
    let schema_version = raw
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            AppError::InvalidInput("unsupported project format: missing schema_version".to_string())
        })?;

    let mut graph = match schema_version as u32 {
        2 => {
            let legacy: ProjectDocument = serde_json::from_value(raw)?;
            CadenceProjectDocument {
                schema_version: CadenceProjectDocument::SCHEMA_VERSION,
                name: legacy.name.clone(),
                graph: legacy.graph,
                init_stage: Default::default(),
            }
        }
        3 => serde_json::from_value::<CadenceProjectDocument>(raw)?,
        other => {
            return Err(AppError::InvalidInput(format!(
                "unsupported schema_version: {} (supported: 2, 3)",
                other
            )));
        }
    };
    let migrated = normalize_project_workspaces(&mut graph);
    validate_project_document(&graph)?;
    Ok((graph, migrated))
}
