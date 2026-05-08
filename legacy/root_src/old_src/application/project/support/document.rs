use super::*;
use std::collections::BTreeSet;

use crate::domain::project::sample_selector_namespace;
use tessera::graph::{Graph, Node};

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
            piece_id: "cadence.sound".to_string(),
            inline_params: BTreeMap::from([("value".to_string(), Value::String("bd".to_string()))]),
            pattern_source: None,
            input_sides: Default::default(),
            output_side: Some(tessera::types::TileSide::RIGHT),
            label: None,
            node_state: None,
        },
    );
    nodes.insert(
        GridPos { col: 1, row: 0 },
        Node {
            piece_id: "cadence.output".to_string(),
            inline_params: BTreeMap::new(),
            pattern_source: None,
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

    let mut project = CadenceProjectDocument::new(
        name.to_string(),
        Graph {
            nodes,
            edges: BTreeMap::from([(edge.id.clone(), edge)]),
            name: name.to_string(),
            cols: STANDARD_GRAPH_COLS,
            rows: STANDARD_GRAPH_ROWS,
        },
    );
    project
        .init_stage
        .sample_loads
        .push(crate::domain::project::CadenceSampleLoad {
            id: "bd".to_string(),
            source: crate::adapter::samples::DEFAULT_KICK_SELECTOR.to_string(),
            aliases: BTreeMap::new(),
        });
    project
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

fn normalize_project_workspaces(project: &mut CadenceProjectDocument) -> bool {
    let mut changed = normalize_graph_workspace(project.runtime_graph_mut());
    for trick in project.tricks_mut() {
        changed |= normalize_graph_workspace(&mut trick.graph);
    }
    changed
}

fn normalize_project_init_stage(project: &mut CadenceProjectDocument) -> bool {
    let mut changed = false;

    let normalized_cps = project
        .init_stage
        .cps_expr
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if project.init_stage.cps_expr != normalized_cps {
        project.init_stage.cps_expr = normalized_cps;
        changed = true;
    }

    for sample in &mut project.init_stage.sample_loads {
        let trimmed_id = sample.id.trim();
        if sample.id != trimmed_id {
            sample.id = trimmed_id.to_string();
            changed = true;
        }

        let trimmed_source = sample.source.trim();
        if sample.source != trimmed_source {
            sample.source = trimmed_source.to_string();
            changed = true;
        }

        let normalized_aliases = sample
            .aliases
            .iter()
            .map(|(key, value)| (key.trim().to_string(), value.trim().to_string()))
            .collect::<BTreeMap<_, _>>();
        if sample.aliases != normalized_aliases {
            sample.aliases = normalized_aliases;
            changed = true;
        }
    }

    for trick in project.tricks_mut() {
        let trimmed_id = trick.id.trim();
        if trick.id != trimmed_id {
            trick.id = trimmed_id.to_string();
            changed = true;
        }

        let trimmed_name = trick.name.trim();
        if trick.name != trimmed_name {
            trick.name = trimmed_name.to_string();
            changed = true;
        }
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

pub(super) fn validate_sample_loads(
    sample_loads: &[crate::domain::project::CadenceSampleLoad],
) -> AppResult<()> {
    let mut sample_ids = BTreeSet::new();
    let mut selectors = BTreeMap::<String, String>::new();
    for sample in sample_loads {
        if sample.id.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "sample load id cannot be empty".to_string(),
            ));
        }
        if sample.source.trim().is_empty() {
            return Err(AppError::InvalidInput(format!(
                "sample load '{}' has an empty source",
                sample.id
            )));
        }
        if !sample_ids.insert(sample.id.clone()) {
            return Err(AppError::InvalidInput(format!(
                "duplicate sample load id '{}' found in project",
                sample.id
            )));
        }
        if sample.aliases.keys().any(|key| key.trim().is_empty()) {
            return Err(AppError::InvalidInput(format!(
                "sample load '{}' contains an empty alias key",
                sample.id
            )));
        }
        if sample.aliases.values().any(|value| value.trim().is_empty()) {
            return Err(AppError::InvalidInput(format!(
                "sample load '{}' contains an empty alias value",
                sample.id
            )));
        }
        for selector in sample_selector_namespace(sample) {
            if let Some(previous) = selectors.insert(selector.clone(), sample.id.clone())
                && previous != sample.id
            {
                return Err(AppError::InvalidInput(format!(
                    "ambiguous sample selector '{}' matches both '{}' and '{}'",
                    selector, previous, sample.id
                )));
            }
        }
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

    validate_graph_bounds(graph.runtime_graph(), "runtime graph")?;
    let mut trick_ids = BTreeSet::new();
    for trick in graph.tricks() {
        if trick.id.trim().is_empty() {
            return Err(AppError::InvalidInput(
                "trick id cannot be empty".to_string(),
            ));
        }
        if trick.name.trim().is_empty() {
            return Err(AppError::InvalidInput(format!(
                "trick '{}' has an empty name",
                trick.id
            )));
        }
        if !trick_ids.insert(trick.id.clone()) {
            return Err(AppError::InvalidInput(format!(
                "duplicate trick id '{}' found in project",
                trick.id
            )));
        }
        validate_graph_bounds(&trick.graph, format!("trick '{}'", trick.name).as_str())?;
    }

    validate_sample_loads(graph.init_stage.sample_loads.as_slice())?;

    Ok(())
}

pub(super) fn project_dto(graph: &CadenceProjectDocument) -> ProjectDto {
    ProjectDto {
        name: graph.name.clone(),
        node_count: graph.runtime_graph().nodes.len(),
        edge_count: graph.runtime_graph().edges.len(),
    }
}

pub(super) fn project_to_view(store: &AppStore, graph: &CadenceProjectDocument) -> ProjectViewDto {
    ProjectViewDto {
        name: graph.name.clone(),
        schema_version: graph.schema_version,
        node_count: graph.runtime_graph().nodes.len(),
        edge_count: graph.runtime_graph().edges.len(),
        dirty: store.dirty,
        path: store.current_path.clone(),
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

pub(super) fn resolve_path(path: &str) -> AppResult<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput("path cannot be empty".to_string()));
    }
    Ok(trimmed.to_string())
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
        4 => serde_json::from_value::<CadenceProjectDocument>(raw)?,
        other => {
            return Err(AppError::InvalidInput(format!(
                "unsupported schema_version: {} (expected {})",
                other,
                CadenceProjectDocument::SCHEMA_VERSION
            )));
        }
    };
    let normalized =
        normalize_project_workspaces(&mut graph) | normalize_project_init_stage(&mut graph);
    validate_project_document(&graph)?;
    Ok((graph, normalized))
}
