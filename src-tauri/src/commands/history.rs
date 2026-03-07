use serde::{Deserialize, Serialize};

use crate::commands::graph_commands::registry_for_target;
use crate::commands::project_commands::SharedAppState;
use crate::errors::AppError;
use crate::errors::AppResult;
use crate::store::app_state::AppStore;
use crate::store::history;
use tile_graph::ops::apply_ops_to_graph;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryStatusDto {
    pub can_undo: bool,
    pub can_redo: bool,
    pub past_len: usize,
    pub future_len: usize,
}

fn history_status_internal(store: &AppStore) -> HistoryStatusDto {
    let can_graph_undo = !store.graph_history_past.is_empty();
    let can_graph_redo = !store.graph_history_future.is_empty();
    let past_len = store.history_past.len() + store.graph_history_past.len();
    let future_len = store.history_future.len() + store.graph_history_future.len();
    HistoryStatusDto {
        can_undo: can_graph_undo || !store.history_past.is_empty(),
        can_redo: can_graph_redo || !store.history_future.is_empty(),
        past_len,
        future_len,
    }
}

fn trim_graph_history_to_limit(store: &mut AppStore) {
    let limit = store.history_limit.max(1);
    if store.graph_history_past.len() > limit {
        let drain = store.graph_history_past.len() - limit;
        store.graph_history_past.drain(0..drain);
    }
    if store.graph_history_future.len() > limit {
        let drain = store.graph_history_future.len() - limit;
        store.graph_history_future.drain(0..drain);
    }
}

fn undo_graph_op_internal(store: &mut AppStore) -> AppResult<HistoryStatusDto> {
    let Some(record) = store.graph_history_past.pop() else {
        return Err(AppError::InvalidInput("nothing to undo".to_string()));
    };
    let Some(project) = store.current_project.as_ref() else {
        return Err(AppError::InvalidInput(
            "no active project; create or open a project first".to_string(),
        ));
    };
    let mut candidate = project
        .graph(&record.target)
        .ok_or_else(|| AppError::InvalidInput("unknown graph target".to_string()))?
        .clone();
    let piece_registry = registry_for_target(project, &record.target);
    if let Err(errors) =
        apply_ops_to_graph(&mut candidate, &piece_registry, record.record.undo_ops.as_slice())
    {
        store.graph_history_past.push(record);
        return Err(AppError::InvalidInput(format!(
            "graph undo failed: {}",
            errors
                .first()
                .map(|diag| format!("{:?}", diag.kind))
                .unwrap_or_else(|| "unknown error".to_string())
        )));
    }
    if let Some(project) = store.current_project.as_mut() {
        if let Some(graph) = project.graph_mut(&record.target) {
            *graph = candidate;
        }
    }
    store.graph_history_future.push(record);
    trim_graph_history_to_limit(store);
    store.dirty = true;
    let status = history_status_internal(store);
    store.push_diagnostic(
        "history_undo_graph",
        format!("past={} future={}", status.past_len, status.future_len),
    );
    Ok(status)
}

fn redo_graph_op_internal(store: &mut AppStore) -> AppResult<HistoryStatusDto> {
    let Some(record) = store.graph_history_future.pop() else {
        return Err(AppError::InvalidInput("nothing to redo".to_string()));
    };
    let Some(project) = store.current_project.as_ref() else {
        return Err(AppError::InvalidInput(
            "no active project; create or open a project first".to_string(),
        ));
    };
    let mut candidate = project
        .graph(&record.target)
        .ok_or_else(|| AppError::InvalidInput("unknown graph target".to_string()))?
        .clone();
    let piece_registry = registry_for_target(project, &record.target);
    if let Err(errors) =
        apply_ops_to_graph(&mut candidate, &piece_registry, record.record.do_ops.as_slice())
    {
        store.graph_history_future.push(record);
        return Err(AppError::InvalidInput(format!(
            "graph redo failed: {}",
            errors
                .first()
                .map(|diag| format!("{:?}", diag.kind))
                .unwrap_or_else(|| "unknown error".to_string())
        )));
    }
    if let Some(project) = store.current_project.as_mut() {
        if let Some(graph) = project.graph_mut(&record.target) {
            *graph = candidate;
        }
    }
    store.graph_history_past.push(record);
    trim_graph_history_to_limit(store);
    store.dirty = true;
    let status = history_status_internal(store);
    store.push_diagnostic(
        "history_redo_graph",
        format!("past={} future={}", status.past_len, status.future_len),
    );
    Ok(status)
}

fn history_undo_internal(store: &mut AppStore) -> AppResult<HistoryStatusDto> {
    if !store.graph_history_past.is_empty() {
        return undo_graph_op_internal(store);
    }
    history::undo(store)?;
    let status = history_status_internal(store);
    store.push_diagnostic(
        "history_undo",
        format!("past={} future={}", status.past_len, status.future_len),
    );
    Ok(status)
}

fn history_redo_internal(store: &mut AppStore) -> AppResult<HistoryStatusDto> {
    if !store.graph_history_future.is_empty() {
        return redo_graph_op_internal(store);
    }
    history::redo(store)?;
    let status = history_status_internal(store);
    store.push_diagnostic(
        "history_redo",
        format!("past={} future={}", status.past_len, status.future_len),
    );
    Ok(status)
}

#[tauri::command]
pub fn history_status(state: tauri::State<'_, SharedAppState>) -> Result<HistoryStatusDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    Ok(history_status_internal(&store))
}

#[tauri::command]
pub fn history_undo(state: tauri::State<'_, SharedAppState>) -> Result<HistoryStatusDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    history_undo_internal(&mut store).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn history_redo(state: tauri::State<'_, SharedAppState>) -> Result<HistoryStatusDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    history_redo_internal(&mut store).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use super::*;
    use crate::core::piece_registry::runtime_registry;
    use crate::model::{CadenceGraphTarget, CadenceProjectDocument, TargetedGraphOpRecord};
    use crate::store::history::record_graph_mutation;
    use tile_graph::graph::{Edge, Graph, GraphOp, GraphOpRecord, Node};
    use tile_graph::types::{EdgeId, GridPos};

    fn seeded_store() -> AppStore {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "strudel.sound".to_string(),
                inline_params: BTreeMap::from([(
                    "value".to_string(),
                    Value::String("bd".to_string()),
                )]),
                input_sides: Default::default(),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "strudel.fast".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: Default::default(),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "strudel.output".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: Default::default(),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        let edge_a = Edge {
            id: EdgeId::new(),
            from: GridPos { col: 0, row: 0 },
            to_node: GridPos { col: 1, row: 0 },
            to_param: "pattern".to_string(),
        };
        let edge_b = Edge {
            id: EdgeId::new(),
            from: GridPos { col: 1, row: 0 },
            to_node: GridPos { col: 2, row: 0 },
            to_param: "pattern".to_string(),
        };
        AppStore {
            current_project: Some(CadenceProjectDocument::new(
                "undo-redo".to_string(),
                Graph {
                    nodes,
                    edges: BTreeMap::from([
                        (edge_a.id.clone(), edge_a),
                        (edge_b.id.clone(), edge_b),
                    ]),
                    name: "undo-redo".to_string(),
                    cols: 9,
                    rows: 9,
                },
            )),
            ..Default::default()
        }
    }

    #[test]
    fn graph_history_undo_redo_replays_inverse_ops() {
        let mut store = seeded_store();
        let registry = runtime_registry(store.current_project.as_ref().expect("project"));
        let mut candidate = store
            .current_project
            .as_ref()
            .expect("project")
            .graph
            .clone();

        let outcome = apply_ops_to_graph(
            &mut candidate,
            &registry,
            &[
                GraphOp::NodePlace {
                    position: GridPos { col: 1, row: 1 },
                    piece_id: "strudel.number".to_string(),
                    inline_params: BTreeMap::from([("value".to_string(), Value::Number(4.into()))]),
                },
                GraphOp::EdgeConnect {
                    edge_id: None,
                    from: GridPos { col: 1, row: 1 },
                    to_node: GridPos { col: 1, row: 0 },
                    to_param: "factor".to_string(),
                },
            ],
        )
        .expect("apply batch");
        let moved_edge_count = candidate.edges.len();
        store.current_project.as_mut().expect("project").graph = candidate;

        record_graph_mutation(
            &mut store,
            TargetedGraphOpRecord {
                target: CadenceGraphTarget::Runtime,
                record: GraphOpRecord {
                    do_ops: outcome.applied_ops.clone(),
                    undo_ops: outcome.undo_ops.clone(),
                    removed_edges: outcome.removed_edges.clone(),
                },
            },
        );

        let undo_status = history_undo_internal(&mut store).expect("undo");
        assert!(undo_status.can_redo);
        let undone_graph = &store.current_project.as_ref().expect("project").graph;
        assert!(!undone_graph.nodes.contains_key(&GridPos { col: 1, row: 1 }));
        assert!(undone_graph.edges.len() < moved_edge_count);

        let redo_status = history_redo_internal(&mut store).expect("redo");
        assert!(redo_status.can_undo);
        let redone_graph = &store.current_project.as_ref().expect("project").graph;
        assert!(redone_graph.nodes.contains_key(&GridPos { col: 1, row: 1 }));
        assert!(redone_graph.edges.len() == moved_edge_count);
    }
}
