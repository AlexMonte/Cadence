use crate::errors::{AppError, AppResult};
use crate::model::TargetedGraphOpRecord;
use crate::store::app_state::{AppStore, HistorySnapshot};

fn history_limit(store: &AppStore) -> usize {
    store.history_limit.max(1)
}

fn trim_front_to_limit(limit: usize, snapshots: &mut Vec<HistorySnapshot>) {
    if snapshots.len() > limit {
        let drain = snapshots.len() - limit;
        snapshots.drain(0..drain);
    }
}

fn trim_front_to_limit_graph(limit: usize, records: &mut Vec<TargetedGraphOpRecord>) {
    if records.len() > limit {
        let drain = records.len() - limit;
        records.drain(0..drain);
    }
}

fn restore_snapshot(store: &mut AppStore, snapshot: HistorySnapshot) {
    store.current_project = Some(snapshot.project);
    store.current_path = snapshot.current_path;
    store.selection = snapshot.selection;
    store.dirty = snapshot.dirty;
    store.last_saved_snapshot_hash = snapshot.last_saved_snapshot_hash;
    clear_graph_history(store);
}

pub fn capture_snapshot(store: &AppStore) -> Option<HistorySnapshot> {
    let project = store.current_project.as_ref()?.clone();
    Some(HistorySnapshot {
        project,
        current_path: store.current_path.clone(),
        selection: store.selection.clone(),
        dirty: store.dirty,
        last_saved_snapshot_hash: store.last_saved_snapshot_hash.clone(),
    })
}

pub fn record_successful_mutation(store: &mut AppStore, before: Option<HistorySnapshot>) {
    let Some(snapshot) = before else {
        return;
    };
    let limit = history_limit(store);
    store.history_past.push(snapshot);
    trim_front_to_limit(limit, &mut store.history_past);
    store.history_future.clear();
}

pub fn record_graph_mutation(store: &mut AppStore, record: TargetedGraphOpRecord) {
    if record.record.do_ops.is_empty()
        && record.record.undo_ops.is_empty()
        && record.record.removed_edges.is_empty()
    {
        return;
    }
    let limit = history_limit(store);
    store.graph_history_past.push(record);
    trim_front_to_limit_graph(limit, &mut store.graph_history_past);
    store.graph_history_future.clear();
}

pub fn clear_graph_history(store: &mut AppStore) {
    store.graph_history_past.clear();
    store.graph_history_future.clear();
}

pub fn undo(store: &mut AppStore) -> AppResult<()> {
    let Some(previous) = store.history_past.pop() else {
        return Err(AppError::InvalidInput("nothing to undo".to_string()));
    };
    let current = capture_snapshot(store).ok_or_else(|| {
        AppError::InvalidInput("no active project; create or open a project first".to_string())
    })?;
    let limit = history_limit(store);
    store.history_future.push(current);
    trim_front_to_limit(limit, &mut store.history_future);
    restore_snapshot(store, previous);
    Ok(())
}

pub fn redo(store: &mut AppStore) -> AppResult<()> {
    let Some(next) = store.history_future.pop() else {
        return Err(AppError::InvalidInput("nothing to redo".to_string()));
    };
    let current = capture_snapshot(store).ok_or_else(|| {
        AppError::InvalidInput("no active project; create or open a project first".to_string())
    })?;
    let limit = history_limit(store);
    store.history_past.push(current);
    trim_front_to_limit(limit, &mut store.history_past);
    restore_snapshot(store, next);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use crate::model::{CadenceGraphTarget, CadenceProjectDocument};
    use tile_graph::graph::{Edge, Graph, GraphOpRecord, Node};
    use tile_graph::types::{EdgeId, GridPos};

    use super::*;

    fn seed_store(name: &str) -> AppStore {
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
                piece_id: "strudel.output".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: Default::default(),
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
        AppStore {
            current_project: Some(CadenceProjectDocument::new(
                name.to_string(),
                Graph {
                    nodes,
                    edges: BTreeMap::from([(edge.id.clone(), edge)]),
                    name: "history".to_string(),
                    cols: 9,
                    rows: 9,
                },
            )),
            ..Default::default()
        }
    }

    #[test]
    fn undo_redo_roundtrip_restores_project_name() {
        let mut store = seed_store("A");
        let before = capture_snapshot(&store);
        store.current_project.as_mut().expect("project").name = "B".to_string();
        record_successful_mutation(&mut store, before);

        undo(&mut store).expect("undo should work");
        assert_eq!(
            store.current_project.as_ref().expect("project").name,
            "A".to_string()
        );

        redo(&mut store).expect("redo should work");
        assert_eq!(
            store.current_project.as_ref().expect("project").name,
            "B".to_string()
        );
    }

    #[test]
    fn record_graph_mutation_ignores_empty_records() {
        let mut store = seed_store("demo");
        record_graph_mutation(
            &mut store,
            TargetedGraphOpRecord {
                target: CadenceGraphTarget::Runtime,
                record: GraphOpRecord {
                    do_ops: Vec::new(),
                    undo_ops: Vec::new(),
                    removed_edges: Vec::new(),
                },
            },
        );
        assert!(store.graph_history_past.is_empty());
        assert!(store.graph_history_future.is_empty());
    }
}
