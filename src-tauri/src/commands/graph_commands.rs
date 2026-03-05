use serde::{Deserialize, Serialize};

use crate::commands::project_commands::{
    SharedAppState, active_project, active_project_mut, mark_store_dirty,
};
use crate::core::code_expr::CodeExpr;
use crate::core::compiler::compile_graph;
use crate::core::diagnostics::{Diagnostic, SemanticResult};
use crate::core::graph::{Edge, Graph, GraphOp, GraphOpRecord};
use crate::core::ops::{ApplyOpsOutcome, apply_ops_to_graph};
use crate::core::piece::PieceDef;
use crate::core::piece_registry::PieceRegistry;
use crate::core::semantic::semantic_pass;
use crate::core::types::GridPos;
use crate::store::history::record_graph_mutation;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCompilePreviewDto {
    pub can_compile: bool,
    pub code: Option<String>,
    pub expr: Option<CodeExpr>,
    pub diagnostics: Vec<Diagnostic>,
    pub eval_order: Vec<GridPos>,
    pub terminal: Option<GridPos>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphApplyResultDto {
    pub graph: Graph,
    pub semantic: SemanticResult,
    pub preview_code: Option<String>,
    pub removed_edges: Vec<Edge>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphApplyArgs {
    pub ops: Vec<GraphOp>,
    pub request_id: Option<String>,
}

pub(crate) fn registry() -> PieceRegistry {
    PieceRegistry::default_strudel()
}

fn compile_preview(graph: &Graph, registry: &PieceRegistry) -> GraphCompilePreviewDto {
    let sem = semantic_pass(graph, registry);
    if !sem.is_valid() {
        return GraphCompilePreviewDto {
            can_compile: false,
            code: None,
            expr: None,
            diagnostics: sem.errors,
            eval_order: sem.eval_order,
            terminal: sem.terminal,
        };
    }

    match compile_graph(graph, registry, &sem) {
        Ok(expr) => GraphCompilePreviewDto {
            can_compile: true,
            code: Some(expr.render()),
            expr: Some(expr),
            diagnostics: Vec::new(),
            eval_order: sem.eval_order,
            terminal: sem.terminal,
        },
        Err(errors) => GraphCompilePreviewDto {
            can_compile: false,
            code: None,
            expr: None,
            diagnostics: errors,
            eval_order: sem.eval_order,
            terminal: sem.terminal,
        },
    }
}

fn apply_ops_transaction(
    current: &Graph,
    registry: &PieceRegistry,
    ops: &[GraphOp],
) -> Result<(Graph, ApplyOpsOutcome, SemanticResult, Option<String>), Vec<Diagnostic>> {
    let mut candidate = current.clone();
    let outcome = apply_ops_to_graph(&mut candidate, registry, ops)?;
    let sem = semantic_pass(&candidate, registry);
    let preview_code = if sem.is_valid() {
        compile_graph(&candidate, registry, &sem)
            .ok()
            .map(|expr| expr.render())
    } else {
        None
    };
    Ok((candidate, outcome, sem, preview_code))
}

#[tauri::command]
pub fn graph_snapshot(state: tauri::State<'_, SharedAppState>) -> Result<Graph, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    Ok(project.graph.clone())
}

#[tauri::command]
pub fn graph_piece_catalog() -> Result<Vec<PieceDef>, String> {
    let registry = registry();
    let mut defs = registry.all_defs();
    defs.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(defs)
}

#[tauri::command]
pub fn graph_compile_preview(
    state: tauri::State<'_, SharedAppState>,
) -> Result<GraphCompilePreviewDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    let registry = registry();
    Ok(compile_preview(&project.graph, &registry))
}

#[tauri::command]
pub fn graph_apply_ops(
    state: tauri::State<'_, SharedAppState>,
    args: GraphApplyArgs,
) -> Result<GraphApplyResultDto, Vec<Diagnostic>> {
    let mut store = state.store.lock().map_err(|_| Vec::<Diagnostic>::new())?;
    let registry = registry();

    if args.ops.is_empty() {
        let project = active_project(&store).map_err(|_| Vec::<Diagnostic>::new())?;
        let sem = semantic_pass(&project.graph, &registry);
        let preview_code = if sem.is_valid() {
            compile_graph(&project.graph, &registry, &sem)
                .ok()
                .map(|expr| expr.render())
        } else {
            None
        };
        return Ok(GraphApplyResultDto {
            graph: project.graph.clone(),
            semantic: sem,
            preview_code,
            removed_edges: Vec::new(),
        });
    }

    let (graph, sem, preview_code, removed_edges, record, did_mutate) = {
        let project = active_project_mut(&mut store).map_err(|_| Vec::<Diagnostic>::new())?;
        let (candidate, outcome, sem, preview_code) =
            apply_ops_transaction(&project.graph, &registry, args.ops.as_slice())?;
        let did_mutate = !outcome.applied_ops.is_empty() || !outcome.removed_edges.is_empty();
        let record = GraphOpRecord {
            do_ops: outcome.applied_ops.clone(),
            undo_ops: outcome.undo_ops.clone(),
            removed_edges: outcome.removed_edges.clone(),
        };
        project.graph = candidate.clone();
        (
            candidate,
            sem,
            preview_code,
            outcome.removed_edges,
            record,
            did_mutate,
        )
    };

    if did_mutate {
        mark_store_dirty(&mut store);
        record_graph_mutation(&mut store, record);
    }
    if let Some(request_id) = args.request_id.as_ref() {
        store.push_diagnostic("graph_apply_ops", format!("request_id={request_id}"));
    } else {
        store.push_diagnostic("graph_apply_ops", "request_id=none".to_string());
    }

    Ok(GraphApplyResultDto {
        graph,
        semantic: sem,
        preview_code,
        removed_edges,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;

    use super::*;
    use crate::core::diagnostics::DiagnosticKind;
    use crate::core::graph::{Edge, Node, ProjectDocument};
    use crate::core::types::EdgeId;

    fn sample_graph() -> Graph {
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
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "strudel.output".to_string(),
                inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
},
        );
        let edge = Edge {
            id: EdgeId::new(),
            from: GridPos { col: 0, row: 0 },
            to_node: GridPos { col: 1, row: 0 },
            to_param: "pattern".to_string(),
        };
        Graph {
            nodes,
            edges: BTreeMap::from([(edge.id.clone(), edge)]),
            name: "graph".to_string(),
        }
    }

    fn canonical_json(graph: &Graph) -> String {
        serde_json::to_string(graph).expect("serialize")
    }

    #[test]
    fn node_place_has_clean_inverse_with_node_remove() {
        let mut graph = sample_graph();
        let before = canonical_json(&graph);
        let registry = registry();

        apply_ops_to_graph(
            &mut graph,
            &registry,
            &[GraphOp::NodePlace {
                position: GridPos { col: 2, row: 0 },
                piece_id: "strudel.number".to_string(),
                inline_params: BTreeMap::new(),
            }],
        )
        .expect("place");
        apply_ops_to_graph(
            &mut graph,
            &registry,
            &[GraphOp::NodeRemove {
                position: GridPos { col: 2, row: 0 },
            }],
        )
        .expect("remove");

        assert_eq!(before, canonical_json(&graph));
    }

    #[test]
    fn node_move_has_clean_inverse_with_reverse_move() {
        let mut graph = sample_graph();
        graph.edges.clear();
        let before = canonical_json(&graph);
        let registry = registry();

        apply_ops_to_graph(
            &mut graph,
            &registry,
            &[GraphOp::NodeMove {
                from: GridPos { col: 0, row: 0 },
                to: GridPos { col: 0, row: 1 },
            }],
        )
        .expect("move forward");
        apply_ops_to_graph(
            &mut graph,
            &registry,
            &[GraphOp::NodeMove {
                from: GridPos { col: 0, row: 1 },
                to: GridPos { col: 0, row: 0 },
            }],
        )
        .expect("move backward");

        assert_eq!(before, canonical_json(&graph));
    }

    #[test]
    fn node_move_removes_edges_that_become_non_adjacent() {
        let mut graph = sample_graph();
        graph.nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "strudel.fast".to_string(),
                inline_params: BTreeMap::new(),
            input_sides: Default::default(),
            output_side: None,
},
        );
        let extra = Edge {
            id: EdgeId::new(),
            from: GridPos { col: 0, row: 0 },
            to_node: GridPos { col: 2, row: 0 },
            to_param: "pattern".to_string(),
        };
        graph.edges.insert(extra.id.clone(), extra);

        let registry = registry();
        let outcome = apply_ops_to_graph(
            &mut graph,
            &registry,
            &[GraphOp::NodeMove {
                from: GridPos { col: 0, row: 0 },
                to: GridPos { col: 0, row: 2 },
            }],
        )
        .expect("move");

        assert!(!outcome.removed_edges.is_empty());
        assert!(
            graph
                .edges
                .values()
                .all(|edge| edge.from != GridPos { col: 0, row: 0 })
        );
    }

    #[test]
    fn graph_snapshot_reflects_semantic_validity() {
        let project = ProjectDocument::new("demo".to_string(), sample_graph());
        let registry = registry();
        let sem = semantic_pass(&project.graph, &registry);
        assert!(sem.is_valid());
        assert!(sem.errors.is_empty());
    }

    #[test]
    fn apply_ops_transaction_returns_preview_code_for_valid_mutation() {
        let graph = sample_graph();
        let registry = registry();
        let (next_graph, _outcome, sem, preview) = apply_ops_transaction(
            &graph,
            &registry,
            &[GraphOp::ParamSetInline {
                position: GridPos { col: 0, row: 0 },
                param_id: "value".to_string(),
                value: Value::String("sd".to_string()),
            }],
        )
        .expect("valid op batch should compile");

        assert!(sem.is_valid());
        assert_eq!(preview, Some("s(\"sd\")".to_string()));
        let value = next_graph
            .nodes
            .get(&GridPos { col: 0, row: 0 })
            .and_then(|node| node.inline_params.get("value"));
        assert_eq!(value, Some(&Value::String("sd".to_string())));
    }

    #[test]
    fn apply_ops_transaction_returns_diagnostics_for_invalid_op() {
        let graph = sample_graph();
        let registry = registry();
        let errors = apply_ops_transaction(
            &graph,
            &registry,
            &[GraphOp::NodePlace {
                position: GridPos { col: 2, row: 0 },
                piece_id: "strudel.unknown_piece".to_string(),
                inline_params: BTreeMap::new(),
            }],
        )
        .expect_err("invalid op should be rejected");

        assert!(
            errors
                .iter()
                .any(|diag| matches!(diag.kind, DiagnosticKind::InvalidOperation { .. }))
        );
    }

    #[test]
    fn apply_ops_transaction_is_atomic_for_mixed_valid_invalid_batches() {
        let graph = sample_graph();
        let before = canonical_json(&graph);
        let registry = registry();
        let errors = apply_ops_transaction(
            &graph,
            &registry,
            &[
                GraphOp::NodePlace {
                    position: GridPos { col: 2, row: 0 },
                    piece_id: "strudel.number".to_string(),
                    inline_params: BTreeMap::new(),
                },
                GraphOp::EdgeConnect {
                    edge_id: None,
                    from: GridPos { col: 8, row: 8 },
                    to_node: GridPos { col: 2, row: 0 },
                    to_param: "value".to_string(),
                },
            ],
        )
        .expect_err("mixed batch should fail");

        assert!(
            errors
                .iter()
                .any(|diag| matches!(diag.kind, DiagnosticKind::InvalidOperation { .. }))
        );
        assert_eq!(before, canonical_json(&graph));
    }

    #[test]
    fn apply_ops_transaction_commits_op_valid_semantic_invalid_graph() {
        let graph = sample_graph();
        let registry = registry();
        let (next_graph, _outcome, sem, preview) = apply_ops_transaction(
            &graph,
            &registry,
            &[GraphOp::NodePlace {
                position: GridPos { col: 2, row: 0 },
                piece_id: "strudel.fast".to_string(),
                inline_params: BTreeMap::new(),
            }],
        )
        .expect("op-valid batch should commit even with semantic errors");

        assert!(!sem.errors.is_empty());
        assert!(!sem.is_valid());
        assert!(preview.is_none());
        assert!(next_graph.nodes.contains_key(&GridPos { col: 2, row: 0 }));
    }

    #[test]
    fn apply_ops_rejects_out_of_bounds_positions() {
        let mut graph = sample_graph();
        let registry = registry();
        let errors = apply_ops_to_graph(
            &mut graph,
            &registry,
            &[GraphOp::NodePlace {
                position: GridPos { col: -1, row: 0 },
                piece_id: "strudel.number".to_string(),
                inline_params: BTreeMap::new(),
            }],
        )
        .expect_err("out-of-bounds placement must fail");

        assert!(
            errors
                .iter()
                .any(|diag| matches!(diag.kind, DiagnosticKind::InvalidOperation { .. }))
        );
        assert!(!graph.nodes.contains_key(&GridPos { col: -1, row: 0 }));
    }

    #[test]
    fn apply_ops_reports_noop_move_and_clear_without_mutation() {
        let mut graph = sample_graph();
        let registry = registry();
        let before = canonical_json(&graph);

        let outcome = apply_ops_to_graph(
            &mut graph,
            &registry,
            &[
                GraphOp::NodeMove {
                    from: GridPos { col: 0, row: 0 },
                    to: GridPos { col: 0, row: 0 },
                },
                GraphOp::ParamClearInline {
                    position: GridPos { col: 1, row: 0 },
                    param_id: "pattern".to_string(),
                },
            ],
        )
        .expect("no-op operations should be accepted");

        assert!(outcome.applied_ops.is_empty());
        assert!(outcome.undo_ops.is_empty());
        assert!(outcome.removed_edges.is_empty());
        assert_eq!(before, canonical_json(&graph));
    }
}
