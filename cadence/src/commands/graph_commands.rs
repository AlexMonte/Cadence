//! Tauri commands for graph snapshots, mutation, probing, and compile previews.

use serde::{Deserialize, Serialize};

use crate::commands::TerminalStrategy;
use crate::commands::project_commands::{
    SharedAppState, active_project, active_project_mut, mark_store_dirty,
};
use crate::core::compile_support::{has_error_diagnostics, merge_diagnostics};
use crate::core::graph_defaults::normalize_graph_piece_sides;
use crate::core::host_adapter::{
    CadenceHostAdapter, normalize_project_piece_sides, rendered_output, runtime_engine,
    subgraph_editor_engine,
};
use crate::core::project_compile::compile_project;
use crate::model::{
    CadenceGraphTarget, CompileMetaDto, DiagnosticDto, GraphEdgeDto, GraphSnapshotDto,
    ProjectCompilePreviewDto, SemanticSnapshotDto, TargetedGraphOpRecord,
};
use crate::store::app_state::AppStore;
use crate::store::history::record_graph_mutation;
use tessera::Backend;
use tessera::JsBackend;
use tessera::compiler::{CompileCache, CompileMode, CompileProgram};
use tessera::diagnostics::{Diagnostic, SemanticResult};
use tessera::graph::{Graph, GraphOp, GraphOpRecord};
use tessera::host::{GraphEngine, HostAdapter};
use tessera::ops::{
    ApplyOpsOutcome, EdgeConnectProbeReason, EdgeTargetParamProbe, RepairSuggestion,
};
use tessera::piece::PieceDef;
use tessera::types::{DomainBridgeKind, GridPos};

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Preview of one graph compiled in isolation.
pub struct GraphCompilePreviewDto {
    pub can_compile: bool,
    pub code: Option<String>,
    pub exprs: Vec<String>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub eval_order: Vec<GridPos>,
    pub terminals: Vec<GridPos>,
    #[serde(default)]
    pub compile_meta: CompileMetaDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Result of applying graph operations and re-analyzing the target graph.
pub struct GraphApplyResultDto {
    pub graph: GraphSnapshotDto,
    pub semantic: SemanticSnapshotDto,
    pub preview: GraphCompilePreviewDto,
    pub removed_edges: Vec<GraphEdgeDto>,
}

#[derive(Debug, Clone, Deserialize)]
/// Batch graph mutation request issued by the editor.
pub struct GraphApplyArgs {
    pub ops: Vec<GraphOp>,
    pub request_id: Option<String>,
    #[serde(default)]
    pub target: CadenceGraphTarget,
}

#[derive(Debug, Clone, Deserialize)]
/// Edge-probe request used while wiring nodes together.
pub struct GraphPickTargetParamArgs {
    pub from: GridPos,
    pub to_node: GridPos,
    #[serde(default)]
    pub to_param: Option<String>,
    #[serde(default)]
    pub target: CadenceGraphTarget,
}

#[derive(Debug, Clone, Deserialize, Default)]
/// Optional graph target wrapper for commands that default to the runtime graph.
pub struct GraphTargetArgs {
    #[serde(default)]
    pub target: CadenceGraphTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Edge-probe result including an accepted param or a structured rejection reason.
pub struct GraphPickTargetParamDto {
    pub to_param: Option<String>,
    pub implicit_bridge: Option<DomainBridgeKind>,
    pub reason: Option<EdgeConnectProbeReason>,
    pub detail: Option<String>,
    pub suggestions: Vec<RepairSuggestion>,
}

impl From<EdgeTargetParamProbe> for GraphPickTargetParamDto {
    fn from(value: EdgeTargetParamProbe) -> Self {
        Self {
            to_param: value.to_param,
            implicit_bridge: value.implicit_bridge,
            reason: value.reason,
            detail: value.detail,
            suggestions: value.suggestions,
        }
    }
}

/// Select the appropriate Tessera engine for the runtime graph or a trick graph.
pub(crate) fn engine_for_target(
    project: &crate::model::CadenceProjectDocument,
    target: &CadenceGraphTarget,
) -> GraphEngine<CadenceHostAdapter> {
    match target {
        CadenceGraphTarget::Runtime => runtime_engine(project, TerminalStrategy::Stack),
        CadenceGraphTarget::Trick { .. } => subgraph_editor_engine(TerminalStrategy::Stack),
    }
}

fn graph_for_target<'a>(
    project: &'a crate::model::CadenceProjectDocument,
    target: &CadenceGraphTarget,
) -> Result<&'a Graph, String> {
    project
        .graph(target)
        .ok_or_else(|| format!("unknown graph target: {:?}", target))
}

fn normalize_graph_for_target(
    project: &mut crate::model::CadenceProjectDocument,
    target: &CadenceGraphTarget,
) -> Result<bool, String> {
    let engine = engine_for_target(project, target);
    let graph = project
        .graph_mut(target)
        .ok_or_else(|| format!("unknown graph target: {:?}", target))?;
    Ok(normalize_graph_piece_sides(graph, engine.registry()))
}

/// Normalize graph piece sides for the target and invalidate the compile cache if anything changed.
fn normalize_and_invalidate(
    store: &mut AppStore,
    target: &CadenceGraphTarget,
) -> Result<(), String> {
    let changed = {
        let project = active_project_mut(store).map_err(|err| err.to_string())?;
        normalize_graph_for_target(project, target)?
    };
    if changed {
        store.clear_compile_cache_for_target(target);
    }
    Ok(())
}

/// Apply a batch of ops against a cloned graph before committing the result.
fn apply_ops_transaction<H: HostAdapter>(
    current: &Graph,
    engine: &GraphEngine<H>,
    ops: &[GraphOp],
    cache: &mut CompileCache,
) -> Result<
    (
        Graph,
        ApplyOpsOutcome,
        SemanticResult,
        GraphCompilePreviewDto,
    ),
    Vec<DiagnosticDto>,
> {
    let mut candidate = current.clone();
    normalize_graph_piece_sides(&mut candidate, engine.registry());
    let outcome = engine
        .apply_ops_cached(&mut candidate, ops, cache)
        .map_err(|diags| {
            diags
                .into_iter()
                .map(DiagnosticDto::from)
                .collect::<Vec<_>>()
        })?;
    normalize_graph_piece_sides(&mut candidate, engine.registry());
    let (sem, preview) = compile_preview_cached(&candidate, engine, cache);
    Ok((candidate, outcome, sem, preview))
}

fn compile_preview_cached<H: HostAdapter>(
    graph: &Graph,
    engine: &GraphEngine<H>,
    cache: &mut CompileCache,
) -> (SemanticResult, GraphCompilePreviewDto) {
    let mut graph = graph.clone();
    normalize_graph_piece_sides(&mut graph, engine.registry());
    let sem = engine.analyze(&graph);
    let dto = match engine.compile_cached(&graph, CompileMode::Preview, cache) {
        Ok(program) => preview_dto_from_program(&sem, &program, engine),
        Err(errors) => preview_dto_from_errors(&sem, errors),
    };
    (sem, dto)
}

fn preview_dto_from_program<H: HostAdapter>(
    sem: &SemanticResult,
    program: &CompileProgram,
    engine: &GraphEngine<H>,
) -> GraphCompilePreviewDto {
    let code = rendered_output(engine.render_terminals(program.terminals.as_slice()));
    let merged = merge_diagnostics([sem.diagnostics.clone(), program.diagnostics.clone()]);
    let can_compile = code.is_some() && !has_error_diagnostics(&merged);
    let diagnostics = merged.into_iter().map(DiagnosticDto::from).collect();
    GraphCompilePreviewDto {
        can_compile,
        code,
        exprs: program
            .terminals
            .iter()
            .map(|expr| JsBackend.render(expr))
            .collect(),
        diagnostics,
        eval_order: sem.eval_order.clone(),
        terminals: sem.terminals.clone(),
        compile_meta: CompileMetaDto::from(program),
    }
}

fn preview_dto_from_errors(
    sem: &SemanticResult,
    errors: Vec<Diagnostic>,
) -> GraphCompilePreviewDto {
    GraphCompilePreviewDto {
        can_compile: false,
        code: None,
        exprs: Vec::new(),
        diagnostics: merge_diagnostics([sem.diagnostics.clone(), errors])
            .into_iter()
            .map(DiagnosticDto::from)
            .collect(),
        eval_order: sem.eval_order.clone(),
        terminals: sem.terminals.clone(),
        compile_meta: CompileMetaDto::default(),
    }
}

#[tauri::command]
/// Return a snapshot of the requested graph target.
pub fn graph_snapshot(
    state: tauri::State<'_, SharedAppState>,
    args: Option<GraphTargetArgs>,
) -> Result<GraphSnapshotDto, String> {
    let target = args.unwrap_or_default().target;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    normalize_and_invalidate(&mut store, &target)?;
    let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
    Ok(GraphSnapshotDto::from(graph_for_target(project, &target)?))
}

#[tauri::command]
/// Return the piece catalog available for the requested graph target.
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
    let engine = engine_for_target(project, &target);
    let mut defs = engine.registry().visible_defs("strudel");
    defs.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(defs)
}

#[tauri::command]
/// Probe which target param an edge should connect to, including repair suggestions.
pub fn graph_pick_target_param(
    state: tauri::State<'_, SharedAppState>,
    args: GraphPickTargetParamArgs,
) -> Result<GraphPickTargetParamDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    normalize_and_invalidate(&mut store, &args.target)?;
    let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
    let engine = engine_for_target(project, &args.target);
    let probe = engine.probe_edge(
        graph_for_target(project, &args.target)?,
        &args.from,
        &args.to_node,
        args.to_param.as_deref(),
    );
    Ok(GraphPickTargetParamDto::from(probe))
}

#[tauri::command]
/// Compile the requested graph target in preview mode.
pub fn graph_compile_preview(
    state: tauri::State<'_, SharedAppState>,
    args: Option<GraphTargetArgs>,
) -> Result<GraphCompilePreviewDto, String> {
    let target = args.unwrap_or_default().target;
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let (engine, graph) = {
        normalize_and_invalidate(&mut store, &target)?;
        let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
        let engine = engine_for_target(project, &target);
        let graph = graph_for_target(project, &target)?.clone();
        (engine, graph)
    };
    let cache = store.compile_cache_for_target_mut(&target);
    Ok(compile_preview_cached(&graph, &engine, cache).1)
}

#[tauri::command]
/// Compile the full project, including init-stage setup and trick declarations.
pub fn project_compile_preview(
    state: tauri::State<'_, SharedAppState>,
) -> Result<ProjectCompilePreviewDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let changed = {
        let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
        normalize_project_piece_sides(project)
    };
    if changed {
        store.clear_compile_caches();
    }
    let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
    let compiled = compile_project(project, TerminalStrategy::Stack, CompileMode::Preview);
    Ok(ProjectCompilePreviewDto {
        can_render: compiled.can_render,
        can_play: compiled.can_play,
        code: compiled.full_code,
        diagnostics: compiled
            .diagnostics
            .into_iter()
            .map(DiagnosticDto::from)
            .collect(),
        compile_meta: compiled.compile_meta,
    })
}

#[tauri::command]
/// Apply graph mutations, refresh semantic state, and record undo history.
pub fn graph_apply_ops(
    state: tauri::State<'_, SharedAppState>,
    args: GraphApplyArgs,
) -> Result<GraphApplyResultDto, Vec<DiagnosticDto>> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| Vec::<DiagnosticDto>::new())?;
    let target = args.target.clone();
    let engine = {
        let project = active_project(&store).map_err(|_| Vec::<DiagnosticDto>::new())?;
        engine_for_target(project, &target)
    };

    if args.ops.is_empty() {
        let graph = {
            let changed = {
                let project =
                    active_project_mut(&mut store).map_err(|_| Vec::<DiagnosticDto>::new())?;
                normalize_graph_for_target(project, &target)
                    .map_err(|_| Vec::<DiagnosticDto>::new())?
            };
            if changed {
                store.clear_compile_cache_for_target(&target);
            }
            let project =
                active_project_mut(&mut store).map_err(|_| Vec::<DiagnosticDto>::new())?;
            graph_for_target(project, &target)
                .map_err(|_| Vec::<DiagnosticDto>::new())?
                .clone()
        };
        let cache = store.compile_cache_for_target_mut(&target);
        let (sem, preview) = compile_preview_cached(&graph, &engine, cache);
        return Ok(GraphApplyResultDto {
            graph: GraphSnapshotDto::from(&graph),
            semantic: SemanticSnapshotDto::from(&sem),
            preview,
            removed_edges: Vec::new(),
        });
    }

    let (graph, sem, preview, removed_edges, record, did_mutate) = {
        let current = {
            let changed = {
                let project =
                    active_project_mut(&mut store).map_err(|_| Vec::<DiagnosticDto>::new())?;
                normalize_graph_for_target(project, &target)
                    .map_err(|_| Vec::<DiagnosticDto>::new())?
            };
            if changed {
                store.clear_compile_cache_for_target(&target);
            }
            let project =
                active_project_mut(&mut store).map_err(|_| Vec::<DiagnosticDto>::new())?;
            project
                .graph(&target)
                .ok_or_else(Vec::<DiagnosticDto>::new)?
                .clone()
        };
        let cache = store.compile_cache_for_target_mut(&target);
        let (candidate, outcome, sem, preview) =
            apply_ops_transaction(&current, &engine, args.ops.as_slice(), cache)?;
        let project = active_project_mut(&mut store).map_err(|_| Vec::<DiagnosticDto>::new())?;
        let current = project
            .graph_mut(&target)
            .ok_or_else(Vec::<DiagnosticDto>::new)?;
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
            preview,
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
        graph: GraphSnapshotDto::from(&graph),
        semantic: SemanticSnapshotDto::from(&sem),
        preview,
        removed_edges: removed_edges.iter().map(GraphEdgeDto::from).collect(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::Value;
    use serde_json::json;

    use super::*;
    use crate::core::host_adapter::subgraph_editor_engine;
    use crate::core::piece_registry::default_strudel_registry;
    use tessera::diagnostics::DiagnosticKind;
    use tessera::graph::{Edge, Node, ProjectDocument};
    use tessera::ops::{apply_ops_to_graph, probe_edge_connect};
    use tessera::semantic::semantic_pass;
    use tessera::types::{EdgeId, TileSide};

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
                output_side: Some(TileSide::RIGHT),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "strudel.output".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: BTreeMap::from([("pattern".to_string(), TileSide::LEFT)]),
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

    fn test_engine() -> GraphEngine<CadenceHostAdapter> {
        subgraph_editor_engine(TerminalStrategy::Stack)
    }

    #[test]
    fn graph_compile_preview_exprs_serialize_as_plain_strings() {
        let dto = GraphCompilePreviewDto {
            can_compile: true,
            code: Some("s(\"bd\")".to_string()),
            exprs: vec!["s(\"bd\")".to_string()],
            diagnostics: Vec::new(),
            eval_order: Vec::new(),
            terminals: Vec::new(),
            compile_meta: CompileMetaDto::default(),
        };

        let payload = serde_json::to_value(&dto).expect("serialize preview dto");
        assert_eq!(payload.get("exprs"), Some(&json!(["s(\"bd\")"])));
    }

    #[test]
    fn graph_pick_target_param_dto_round_trips_implicit_bridge_metadata() {
        let dto = GraphPickTargetParamDto {
            to_param: Some("value".to_string()),
            implicit_bridge: Some(DomainBridgeKind::ControlToAudio),
            reason: None,
            detail: Some("bridgeable".to_string()),
            suggestions: Vec::new(),
        };

        let payload = serde_json::to_value(&dto).expect("serialize probe dto");
        assert_eq!(
            payload.get("implicit_bridge"),
            Some(&json!("control_to_audio"))
        );

        let round_trip =
            serde_json::from_value::<GraphPickTargetParamDto>(payload).expect("deserialize");
        assert_eq!(round_trip.to_param.as_deref(), Some("value"));
        assert_eq!(
            round_trip.implicit_bridge,
            Some(DomainBridgeKind::ControlToAudio)
        );
        assert_eq!(round_trip.reason, None);
    }

    #[test]
    fn graph_pick_target_param_dto_round_trips_unsupported_domain_reason() {
        let dto = GraphPickTargetParamDto {
            to_param: None,
            implicit_bridge: None,
            reason: Some(EdgeConnectProbeReason::UnsupportedDomain),
            detail: Some("unsupported domain crossing".to_string()),
            suggestions: Vec::new(),
        };

        let payload = serde_json::to_value(&dto).expect("serialize probe dto");
        assert_eq!(payload.get("reason"), Some(&json!("unsupported_domain")));

        let round_trip =
            serde_json::from_value::<GraphPickTargetParamDto>(payload).expect("deserialize");
        assert_eq!(
            round_trip.reason,
            Some(EdgeConnectProbeReason::UnsupportedDomain)
        );
        assert!(round_trip.to_param.is_none());
        assert!(round_trip.implicit_bridge.is_none());
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
        let registry = default_strudel_registry();
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
        let registry = default_strudel_registry();

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
        let registry = default_strudel_registry();

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
                input_sides: BTreeMap::from([("pattern".to_string(), TileSide::LEFT)]),
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

        let registry = default_strudel_registry();
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
        let registry = default_strudel_registry();

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
        let registry = default_strudel_registry();
        let sem = semantic_pass(&project.graph, &registry);
        assert!(sem.is_valid());
        assert!(sem.diagnostics.is_empty());
    }

    #[test]
    fn apply_ops_transaction_returns_preview_for_valid_mutation() {
        let graph = sample_graph();
        let engine = test_engine();
        let mut cache = CompileCache::new();
        let (next_graph, _outcome, sem, preview) = apply_ops_transaction(
            &graph,
            &engine,
            &[GraphOp::ParamSetInline {
                position: GridPos { col: 0, row: 0 },
                param_id: "value".to_string(),
                value: Value::String("sd".to_string()),
            }],
            &mut cache,
        )
        .expect("valid op batch should compile");

        assert!(sem.is_valid());
        assert_eq!(preview.code.as_deref(), Some("s('sd')"));
        assert!(preview.diagnostics.is_empty());
        let value = next_graph
            .nodes
            .get(&GridPos { col: 0, row: 0 })
            .and_then(|node| node.inline_params.get("value"));
        assert_eq!(value, Some(&Value::String("sd".to_string())));
    }

    #[test]
    fn apply_ops_transaction_returns_diagnostics_for_invalid_op() {
        let graph = sample_graph();
        let engine = test_engine();
        let mut cache = CompileCache::new();
        let errors = apply_ops_transaction(
            &graph,
            &engine,
            &[GraphOp::NodePlace {
                position: GridPos { col: 2, row: 0 },
                piece_id: "strudel.unknown_piece".to_string(),
                inline_params: BTreeMap::new(),
            }],
            &mut cache,
        )
        .expect_err("invalid op should be rejected");

        assert!(errors.iter().any(|diag| matches!(
            diag.kind,
            crate::model::DiagnosticKindDto::InvalidOperation { .. }
        )));
    }

    #[test]
    fn apply_ops_transaction_is_atomic_for_mixed_valid_invalid_batches() {
        let graph = sample_graph();
        let before = canonical_json(&graph);
        let engine = test_engine();
        let mut cache = CompileCache::new();
        let errors = apply_ops_transaction(
            &graph,
            &engine,
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
            &mut cache,
        )
        .expect_err("mixed batch should fail");

        assert!(errors.iter().any(|diag| matches!(
            diag.kind,
            crate::model::DiagnosticKindDto::InvalidOperation { .. }
        )));
        assert_eq!(before, canonical_json(&graph));
    }

    #[test]
    fn apply_ops_transaction_commits_op_valid_semantic_invalid_graph() {
        let graph = sample_graph();
        let engine = test_engine();
        let mut cache = CompileCache::new();
        let (next_graph, _outcome, sem, preview) = apply_ops_transaction(
            &graph,
            &engine,
            &[GraphOp::NodePlace {
                position: GridPos { col: 2, row: 0 },
                piece_id: "strudel.fast".to_string(),
                inline_params: BTreeMap::new(),
            }],
            &mut cache,
        )
        .expect("op-valid batch should commit even with semantic errors");

        assert!(!sem.diagnostics.is_empty());
        assert!(!sem.is_valid());
        assert_eq!(preview.code.as_deref(), Some("s('bd')"));
        assert!(!preview.diagnostics.is_empty());
        assert!(next_graph.nodes.contains_key(&GridPos { col: 2, row: 0 }));
    }

    #[test]
    fn compile_preview_cached_returns_partial_code_for_semantic_errors() {
        let mut graph = sample_graph();
        graph.nodes.insert(
            GridPos { col: 2, row: 0 },
            Node {
                piece_id: "strudel.fast".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: BTreeMap::from([("pattern".to_string(), TileSide::LEFT)]),
                output_side: None,
                label: None,
                node_state: None,
            },
        );

        let engine = test_engine();
        let mut cache = CompileCache::new();
        let (sem, preview) = compile_preview_cached(&graph, &engine, &mut cache);

        assert!(!sem.is_valid());
        assert!(!preview.can_compile);
        assert_eq!(preview.code.as_deref(), Some("s('bd')"));
        assert!(!preview.diagnostics.is_empty());
    }

    #[test]
    fn apply_ops_rejects_out_of_bounds_positions() {
        let mut graph = sample_graph();
        let registry = default_strudel_registry();
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
                input_sides: BTreeMap::from([("pattern".to_string(), TileSide::LEFT)]),
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
                input_sides: BTreeMap::from([("pattern".to_string(), TileSide::LEFT)]),
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
        let registry = default_strudel_registry();

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
                input_sides: BTreeMap::from([("pattern".to_string(), TileSide::BOTTOM)]),
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
        let registry = default_strudel_registry();

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
                output_side: Some(TileSide::TOP),
                label: None,
                node_state: None,
            },
        );
        nodes.insert(
            GridPos { col: 1, row: 0 },
            Node {
                piece_id: "strudel.fast".to_string(),
                inline_params: BTreeMap::new(),
                input_sides: BTreeMap::from([("pattern".to_string(), TileSide::BOTTOM)]),
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
        let registry = default_strudel_registry();

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
                        input_sides: BTreeMap::from([("pattern".to_string(), TileSide::BOTTOM)]),
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
        let registry = default_strudel_registry();
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
        let registry = default_strudel_registry();
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
