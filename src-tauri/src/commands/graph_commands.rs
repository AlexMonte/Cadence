use serde::{Deserialize, Serialize};

use crate::commands::TerminalStrategy;
use crate::commands::project_commands::{
    SharedAppState, active_project, active_project_mut, mark_store_dirty,
};
use crate::commands::runtime_commands::render_terminals;
use crate::core::piece_registry::{runtime_registry, trick_editor_registry};
use crate::core::project_compile::compile_project;
use crate::core::terminal_strategy::StackRenderer;
use crate::model::{CadenceGraphTarget, ProjectCompilePreviewDto, TargetedGraphOpRecord};
use crate::store::history::record_graph_mutation;
use tile_graph::code_expr::CodeExpr;
use tile_graph::compiler::{CompileMode, compile_graph};
use tile_graph::diagnostics::{Diagnostic, SemanticResult};
use tile_graph::graph::{Edge, Graph, GraphOp, GraphOpRecord};
use tile_graph::ops::{
    ApplyOpsOutcome, EdgeConnectProbeReason, EdgeTargetParamProbe, apply_ops_to_graph,
    probe_edge_connect,
};
use tile_graph::piece::PieceDef;
use tile_graph::piece_registry::PieceRegistry;
use tile_graph::semantic::semantic_pass;
use tile_graph::types::GridPos;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCompilePreviewDto {
    pub can_compile: bool,
    pub code: Option<String>,
    pub exprs: Vec<CodeExpr>,
    pub diagnostics: Vec<Diagnostic>,
    pub eval_order: Vec<GridPos>,
    pub terminals: Vec<GridPos>,
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
    #[serde(default)]
    pub target: CadenceGraphTarget,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphPickTargetParamArgs {
    pub from: GridPos,
    pub to_node: GridPos,
    #[serde(default)]
    pub to_param: Option<String>,
    #[serde(default)]
    pub target: CadenceGraphTarget,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GraphTargetArgs {
    #[serde(default)]
    pub target: CadenceGraphTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPickTargetParamDto {
    pub to_param: Option<String>,
    pub reason: Option<EdgeConnectProbeReason>,
    pub detail: Option<String>,
}

impl From<EdgeTargetParamProbe> for GraphPickTargetParamDto {
    fn from(value: EdgeTargetParamProbe) -> Self {
        Self {
            to_param: value.to_param,
            reason: value.reason,
            detail: value.detail,
        }
    }
}

pub(crate) fn registry_for_target(
    project: &crate::model::CadenceProjectDocument,
    target: &CadenceGraphTarget,
) -> PieceRegistry {
    match target {
        CadenceGraphTarget::Runtime => runtime_registry(project),
        CadenceGraphTarget::Trick { .. } => trick_editor_registry(),
    }
}

pub(crate) fn registry() -> PieceRegistry {
    crate::core::piece_registry::default_strudel_registry()
}

fn graph_for_target<'a>(
    project: &'a crate::model::CadenceProjectDocument,
    target: &CadenceGraphTarget,
) -> Result<&'a Graph, String> {
    project
        .graph(target)
        .ok_or_else(|| format!("unknown graph target: {:?}", target))
}

fn compile_preview(graph: &Graph, registry: &PieceRegistry) -> GraphCompilePreviewDto {
    let sem = semantic_pass(graph, registry);
    if !sem.is_valid() {
        return GraphCompilePreviewDto {
            can_compile: false,
            code: None,
            exprs: Vec::new(),
            diagnostics: sem.diagnostics,
            eval_order: sem.eval_order,
            terminals: sem.terminals,
        };
    }

    match compile_graph(graph, registry, &sem, CompileMode::Preview) {
        Ok(program) => GraphCompilePreviewDto {
            can_compile: true,
            code: Some(render_terminals(
                program.terminals.as_slice(),
                &StackRenderer,
            )),
            exprs: program.terminals,
            diagnostics: Vec::new(),
            eval_order: sem.eval_order,
            terminals: sem.terminals,
        },
        Err(errors) => GraphCompilePreviewDto {
            can_compile: false,
            code: None,
            exprs: Vec::new(),
            diagnostics: errors,
            eval_order: sem.eval_order,
            terminals: sem.terminals,
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
        compile_graph(&candidate, registry, &sem, CompileMode::Preview)
            .ok()
            .map(|program| render_terminals(program.terminals.as_slice(), &StackRenderer))
    } else {
        None
    };
    Ok((candidate, outcome, sem, preview_code))
}

#[tauri::command]
pub fn graph_snapshot(
    state: tauri::State<'_, SharedAppState>,
    args: Option<GraphTargetArgs>,
) -> Result<Graph, String> {
    let target = args.unwrap_or_default().target;
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    Ok(graph_for_target(project, &target)?.clone())
}

#[tauri::command]
pub fn graph_piece_catalog(
    state: tauri::State<'_, SharedAppState>,
    args: Option<GraphTargetArgs>,
) -> Result<Vec<PieceDef>, String> {
    let target = args.unwrap_or_default().target;
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    let registry = registry_for_target(project, &target);
    let mut defs = registry.all_defs();
    defs.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(defs)
}

#[tauri::command]
pub fn graph_pick_target_param(
    state: tauri::State<'_, SharedAppState>,
    args: GraphPickTargetParamArgs,
) -> Result<GraphPickTargetParamDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    let registry = registry_for_target(project, &args.target);
    let probe = probe_edge_connect(
        graph_for_target(project, &args.target)?,
        &registry,
        &args.from,
        &args.to_node,
        args.to_param.as_deref(),
    );
    Ok(GraphPickTargetParamDto::from(probe))
}

#[tauri::command]
pub fn graph_compile_preview(
    state: tauri::State<'_, SharedAppState>,
    args: Option<GraphTargetArgs>,
) -> Result<GraphCompilePreviewDto, String> {
    let target = args.unwrap_or_default().target;
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    let registry = registry_for_target(project, &target);
    Ok(compile_preview(
        graph_for_target(project, &target)?,
        &registry,
    ))
}

#[tauri::command]
pub fn project_compile_preview(
    state: tauri::State<'_, SharedAppState>,
) -> Result<ProjectCompilePreviewDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    let compiled = compile_project(project, TerminalStrategy::Stack, CompileMode::Preview);
    Ok(ProjectCompilePreviewDto {
        can_render: compiled.can_render,
        can_play: compiled.can_play,
        code: compiled.full_code,
        diagnostics: compiled.diagnostics,
    })
}

#[tauri::command]
pub fn graph_apply_ops(
    state: tauri::State<'_, SharedAppState>,
    args: GraphApplyArgs,
) -> Result<GraphApplyResultDto, Vec<Diagnostic>> {
    let mut store = state.store.lock().map_err(|_| Vec::<Diagnostic>::new())?;
    let target = args.target.clone();
    let registry = {
        let project = active_project(&store).map_err(|_| Vec::<Diagnostic>::new())?;
        registry_for_target(project, &target)
    };

    if args.ops.is_empty() {
        let project = active_project(&store).map_err(|_| Vec::<Diagnostic>::new())?;
        let graph = graph_for_target(project, &target).map_err(|_| Vec::<Diagnostic>::new())?;
        let sem = semantic_pass(graph, &registry);
        let preview_code = if sem.is_valid() {
            compile_graph(graph, &registry, &sem, CompileMode::Preview)
                .ok()
                .map(|program| render_terminals(program.terminals.as_slice(), &StackRenderer))
        } else {
            None
        };
        return Ok(GraphApplyResultDto {
            graph: graph.clone(),
            semantic: sem,
            preview_code,
            removed_edges: Vec::new(),
        });
    }

    let (graph, sem, preview_code, removed_edges, record, did_mutate) = {
        let project = active_project_mut(&mut store).map_err(|_| Vec::<Diagnostic>::new())?;
        let current = project
            .graph_mut(&target)
            .ok_or_else(Vec::<Diagnostic>::new)?;
        let (candidate, outcome, sem, preview_code) =
            apply_ops_transaction(current, &registry, args.ops.as_slice())?;
        let did_mutate = !outcome.applied_ops.is_empty() || !outcome.removed_edges.is_empty();
        let record = GraphOpRecord {
            do_ops: outcome.applied_ops.clone(),
            undo_ops: outcome.undo_ops.clone(),
            removed_edges: outcome.removed_edges.clone(),
        };
        *current = candidate.clone();
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
        record_graph_mutation(&mut store, TargetedGraphOpRecord { target, record });
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
    use tile_graph::diagnostics::DiagnosticKind;
    use tile_graph::graph::{Edge, Node, ProjectDocument};
    use tile_graph::types::{EdgeId, TileSide};

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
        Graph {
            nodes,
            edges: BTreeMap::from([(edge.id.clone(), edge)]),
            name: "graph".to_string(),
            cols: 9,
            rows: 9,
        }
    }

    fn canonical_json(graph: &Graph) -> String {
        serde_json::to_string(graph).expect("serialize")
    }

    #[test]
    fn node_place_accepts_piece_ids_from_catalog() {
        let mut graph = Graph {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            name: "catalog-node-place".to_string(),
            cols: 9,
            rows: 9,
        };
        let registry = registry();
        let mut defs = registry.all_defs();
        defs.sort_by(|left, right| left.id.cmp(&right.id));

        for (index, def) in defs.iter().enumerate() {
            let position = GridPos {
                col: (index % 9) as i32,
                row: (index / 9) as i32,
            };
            apply_ops_to_graph(
                &mut graph,
                &registry,
                &[GraphOp::NodePlace {
                    position,
                    piece_id: def.id.clone(),
                    inline_params: BTreeMap::new(),
                }],
            )
            .unwrap_or_else(|errors| {
                panic!(
                    "node_place should accept catalog piece '{}' but got diagnostics: {:?}",
                    def.id, errors
                )
            });
        }

        assert_eq!(graph.nodes.len(), defs.len());
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
                label: None,
                node_state: None,
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
    fn node_swap_outcome_has_deterministic_do_and_undo_ops() {
        let mut graph = sample_graph();
        let registry = registry();

        let outcome = apply_ops_to_graph(
            &mut graph,
            &registry,
            &[GraphOp::NodeSwap {
                a: GridPos { col: 0, row: 0 },
                b: GridPos { col: 1, row: 0 },
            }],
        )
        .expect("swap");

        assert_eq!(outcome.applied_ops.len(), 1);
        assert!(matches!(
            outcome.applied_ops.first(),
            Some(GraphOp::NodeSwap { a, b })
            if *a == GridPos { col: 0, row: 0 } && *b == GridPos { col: 1, row: 0 }
        ));
        assert!(matches!(
            outcome.undo_ops.first(),
            Some(GraphOp::NodeSwap { a, b })
            if *a == GridPos { col: 0, row: 0 } && *b == GridPos { col: 1, row: 0 }
        ));
    }

    #[test]
    fn graph_snapshot_reflects_semantic_validity() {
        let project = ProjectDocument::new("demo".to_string(), sample_graph());
        let registry = registry();
        let sem = semantic_pass(&project.graph, &registry);
        assert!(sem.is_valid());
        assert!(sem.diagnostics.is_empty());
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

        assert!(!sem.diagnostics.is_empty());
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
    fn probe_selects_pattern_param_for_chainable_links() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 0, row: 0 },
            Node {
                piece_id: "strudel.sound".to_string(),
                inline_params: BTreeMap::new(),
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
                piece_id: "strudel.gain".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: Default::default(),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        let graph = Graph {
            nodes,
            edges: BTreeMap::new(),
            name: "probe-chain".to_string(),
            cols: 9,
            rows: 9,
        };
        let registry = registry();

        let first = probe_edge_connect(
            &graph,
            &registry,
            &GridPos { col: 0, row: 0 },
            &GridPos { col: 1, row: 0 },
            None,
        );
        assert_eq!(first.to_param.as_deref(), Some("pattern"));
        assert!(first.reason.is_none());

        let second = probe_edge_connect(
            &graph,
            &registry,
            &GridPos { col: 1, row: 0 },
            &GridPos { col: 2, row: 0 },
            None,
        );
        assert_eq!(second.to_param.as_deref(), Some("pattern"));
        assert!(second.reason.is_none());
    }

    #[test]
    fn probe_rejects_type_mismatch_for_number_into_gain_pattern() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 1, row: 1 },
            Node {
                piece_id: "strudel.number".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: Default::default(),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "strudel.gain".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: BTreeMap::from([("pattern".to_string(), TileSide::South)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        let graph = Graph {
            nodes,
            edges: BTreeMap::new(),
            name: "probe-type".to_string(),
            cols: 9,
            rows: 9,
        };
        let registry = registry();

        let probe = probe_edge_connect(
            &graph,
            &registry,
            &GridPos { col: 1, row: 1 },
            &GridPos { col: 1, row: 0 },
            Some("pattern"),
        );
        assert!(probe.to_param.is_none());
        assert_eq!(probe.reason, Some(EdgeConnectProbeReason::TypeMismatch));
    }

    #[test]
    fn probe_respects_adjacency_and_side_overrides() {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            GridPos { col: 1, row: 1 },
            Node {
                piece_id: "strudel.sound".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: Default::default(),
                output_side: Some(TileSide::North),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "strudel.fast".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: BTreeMap::from([("pattern".to_string(), TileSide::South)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 4, row: 4 },
            Node {
                piece_id: "strudel.sound".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: Default::default(),
                output_side: None,
                label: None,
                node_state: None,
            },
        );
        let graph = Graph {
            nodes,
            edges: BTreeMap::new(),
            name: "probe-side".to_string(),
            cols: 9,
            rows: 9,
        };
        let registry = registry();

        let override_ok = probe_edge_connect(
            &graph,
            &registry,
            &GridPos { col: 1, row: 1 },
            &GridPos { col: 1, row: 0 },
            None,
        );
        assert_eq!(override_ok.to_param.as_deref(), Some("pattern"));

        let non_adjacent = probe_edge_connect(
            &graph,
            &registry,
            &GridPos { col: 4, row: 4 },
            &GridPos { col: 1, row: 0 },
            None,
        );
        assert!(non_adjacent.to_param.is_none());
        assert_eq!(
            non_adjacent.reason,
            Some(EdgeConnectProbeReason::NotAdjacent)
        );
    }

    #[test]
    fn probe_and_edge_connect_reject_with_same_outcome_for_invalid_link() {
        let mut graph = Graph {
            nodes: BTreeMap::from([
                (
                    GridPos { col: 1, row: 1 },
                    Node {
                        piece_id: "strudel.number".to_string(),
                        inline_params: BTreeMap::new(),
                        input_sides: Default::default(),
                        output_side: None,
                        label: None,
                        node_state: None,
                    },
                ),
                (
                    GridPos { col: 1, row: 0 },
                    Node {
                        piece_id: "strudel.gain".to_string(),
                        inline_params: BTreeMap::new(),
                        input_sides: BTreeMap::from([("pattern".to_string(), TileSide::South)]),
                        output_side: None,
                        label: None,
                        node_state: None,
                    },
                ),
            ]),
            edges: BTreeMap::new(),
            name: "probe-vs-apply".to_string(),
            cols: 9,
            rows: 9,
        };
        let registry = registry();
        let probe = probe_edge_connect(
            &graph,
            &registry,
            &GridPos { col: 1, row: 1 },
            &GridPos { col: 1, row: 0 },
            Some("pattern"),
        );
        assert_eq!(probe.reason, Some(EdgeConnectProbeReason::TypeMismatch));

        let apply = apply_ops_to_graph(
            &mut graph,
            &registry,
            &[GraphOp::EdgeConnect {
                edge_id: None,
                from: GridPos { col: 1, row: 1 },
                to_node: GridPos { col: 1, row: 0 },
                to_param: "pattern".to_string(),
            }],
        );
        assert!(
            apply.is_err(),
            "edge_connect should reject same invalid link"
        );
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
