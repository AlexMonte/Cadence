//! Backend helpers for graph snapshots, mutation, probing, and compile previews.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    adapter::tessera::{
        cadence_diagnostics, encode_diagnostics, encode_graph_edge, encode_graph_snapshot,
        encode_semantic_snapshot,
        graph_defaults::normalize_graph_piece_sides,
        host_adapter::{
            CadenceGraphEngine, normalize_project_piece_sides, runtime_engine,
            subgraph_editor_engine,
        },
        lowering::{LoweredGraph, lower_target_graph},
    },
    application::{
        authoring::{compile::compile_project, compile_support::has_error_diagnostics},
        history::record_graph_mutation,
        project::ops::{active_project, active_project_mut, mark_store_dirty},
        state::AppStore,
    },
    domain::project::{CadenceGraphTarget, TargetedGraphOpRecord},
    infrastructure::dto::SharedAppState,
    infrastructure::dto::{
        DiagnosticDto, GraphEdgeDto, GraphSnapshotDto, PreviewDocumentDto,
        ProjectCompilePreviewDto, SemanticSnapshotDto,
    },
};
use tessera::{
    analysis::{AnalysisCache, AnalyzedGraph},
    graph::{Graph, GraphOp, GraphOpRecord},
    ops::{ApplyOpsOutcome, EdgeConnectProbeReason, EdgeTargetParamProbe, RepairSuggestion},
    piece::PieceDef,
    types::{DomainBridgeKind, GridPos},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphCompilePreviewDto {
    pub can_compile: bool,
    pub diagnostics: Vec<DiagnosticDto>,
    pub eval_order: Vec<GridPos>,
    pub outputs: Vec<GridPos>,
    #[serde(default)]
    pub preview: PreviewDocumentDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphApplyResultDto {
    pub graph: GraphSnapshotDto,
    pub semantic: SemanticSnapshotDto,
    pub preview: GraphCompilePreviewDto,
    pub removed_edges: Vec<GraphEdgeDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub(crate) fn engine_for_target(
    project: &crate::domain::project::CadenceProjectDocument,
    target: &CadenceGraphTarget,
) -> CadenceGraphEngine {
    match target {
        CadenceGraphTarget::Runtime => runtime_engine(project),
        CadenceGraphTarget::Trick { .. } => subgraph_editor_engine(),
    }
}

fn graph_for_target<'a>(
    project: &'a crate::domain::project::CadenceProjectDocument,
    target: &CadenceGraphTarget,
) -> Result<&'a Graph, String> {
    project
        .graph(target)
        .ok_or_else(|| format!("unknown graph target: {:?}", target))
}

fn normalize_graph_for_target(
    project: &mut crate::domain::project::CadenceProjectDocument,
    target: &CadenceGraphTarget,
) -> Result<bool, String> {
    let engine = engine_for_target(project, target);
    let graph = project
        .graph_mut(target)
        .ok_or_else(|| format!("unknown graph target: {:?}", target))?;
    let mut changed = normalize_graph_piece_sides(graph, engine.registry());
    changed |= normalize_legacy_control_input_shape(graph);
    Ok(changed)
}

fn normalize_and_invalidate(
    store: &mut AppStore,
    _target: &CadenceGraphTarget,
) -> Result<(), String> {
    let changed = {
        let project = active_project_mut(store).map_err(|err| err.to_string())?;
        let mut changed = normalize_project_piece_sides(project);
        changed |= normalize_legacy_control_input_shape(project.runtime_graph_mut());
        for trick in project.tricks_mut() {
            changed |= normalize_legacy_control_input_shape(&mut trick.graph);
        }
        changed
    };
    if changed {
        store.clear_compile_caches();
    }
    Ok(())
}

fn preview_dto_from_lowered(
    analyzed: &AnalyzedGraph,
    lowered: LoweredGraph,
) -> GraphCompilePreviewDto {
    let can_compile = !lowered.program.outputs.is_empty()
        && !has_error_diagnostics(lowered.diagnostics.as_slice());
    GraphCompilePreviewDto {
        can_compile,
        diagnostics: crate::adapter::transport::translate(cadence_diagnostics(lowered.diagnostics))
            .expect("cadence diagnostics should serialize into DTO diagnostics"),
        eval_order: analyzed.eval_order.clone(),
        outputs: analyzed.outputs.clone(),
        preview: lowered.preview,
    }
}

fn compile_preview_cached(
    project: &crate::domain::project::CadenceProjectDocument,
    target: &CadenceGraphTarget,
    graph: &Graph,
    engine: &CadenceGraphEngine,
    cache: &mut AnalysisCache,
) -> (AnalyzedGraph, GraphCompilePreviewDto) {
    let analyzed = engine.analyze_cached(graph, cache);
    let mut candidate_project = project.clone();
    if let Some(current) = candidate_project.graph_mut(target) {
        *current = graph.clone();
    }
    let lowered = lower_target_graph(&candidate_project, target, &analyzed);
    let dto = preview_dto_from_lowered(&analyzed, lowered);
    (analyzed, dto)
}

fn apply_ops_transaction(
    current: &Graph,
    engine: &CadenceGraphEngine,
    ops: &[GraphOp],
    cache: &mut AnalysisCache,
) -> Result<(Graph, ApplyOpsOutcome, AnalyzedGraph), Vec<DiagnosticDto>> {
    let mut candidate = current.clone();
    normalize_graph_piece_sides(&mut candidate, engine.registry());
    normalize_legacy_control_input_shape(&mut candidate);
    let outcome = engine
        .apply_ops_cached(&mut candidate, ops, cache)
        .map_err(|diagnostics| encode_diagnostics(diagnostics).unwrap_or_default())?;
    normalize_graph_piece_sides(&mut candidate, engine.registry());
    normalize_legacy_control_input_shape(&mut candidate);
    let analyzed = engine.analyze_cached(&candidate, cache);
    Ok((candidate, outcome, analyzed))
}

fn normalize_legacy_control_input_shape(graph: &mut Graph) -> bool {
    let mut changed = false;
    let mut legacy_edges_by_target: BTreeMap<GridPos, Vec<String>> = BTreeMap::new();

    for (pos, node) in &mut graph.nodes {
        if node.piece_id != "cadence.control_input" {
            continue;
        }

        let mut shape = Map::new();
        for key in ["min", "max", "step"] {
            if let Some(value) = node.inline_params.remove(key) {
                shape.insert(key.to_string(), value);
                changed = true;
            }
        }

        if shape.is_empty() {
            continue;
        }

        let mut node_state = match node.node_state.take() {
            Some(Value::Object(map)) => map,
            Some(other) => {
                let mut map = Map::new();
                map.insert("legacy_node_state".to_string(), other);
                map
            }
            None => Map::new(),
        };
        node_state.insert("control_input_shape".to_string(), Value::Object(shape));
        node.node_state = Some(Value::Object(node_state));

        legacy_edges_by_target.insert(
            *pos,
            ["min", "max", "step"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
    }

    if legacy_edges_by_target.is_empty() {
        return changed;
    }

    let original_edge_count = graph.edges.len();
    graph.edges.retain(|_, edge| {
        !legacy_edges_by_target
            .get(&edge.to_node)
            .is_some_and(|params| params.iter().any(|param| param == &edge.to_param))
    });
    changed |= graph.edges.len() != original_edge_count;

    changed
}

fn piece_is_hidden_graph_catalog_entry(piece_id: &str) -> bool {
    matches!(
        piece_id,
        "cadence.number" | "cadence.text" | "cadence.pattern_notation"
    )
}

pub fn graph_snapshot(
    state: &SharedAppState,
    target: Option<CadenceGraphTarget>,
) -> Result<GraphSnapshotDto, String> {
    let target = target.unwrap_or_default();
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    normalize_and_invalidate(&mut store, &target)?;
    let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
    Ok(encode_graph_snapshot(graph_for_target(project, &target)?))
}

pub fn graph_piece_catalog(
    state: &SharedAppState,
    target: Option<CadenceGraphTarget>,
) -> Result<Vec<PieceDef>, String> {
    let target = target.unwrap_or_default();
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    let engine = engine_for_target(project, &target);
    let mut defs = engine.registry().visible_defs("cadence");
    defs.retain(|piece| !piece_is_hidden_graph_catalog_entry(piece.id.as_str()));
    defs.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(defs)
}

pub fn graph_pick_target_param(
    state: &SharedAppState,
    from: GridPos,
    to_node: GridPos,
    target: CadenceGraphTarget,
    to_param: Option<String>,
) -> Result<GraphPickTargetParamDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    normalize_and_invalidate(&mut store, &target)?;
    let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
    let engine = engine_for_target(project, &target);
    let probe = engine.probe_edge(
        graph_for_target(project, &target)?,
        &from,
        &to_node,
        to_param.as_deref(),
    );
    Ok(GraphPickTargetParamDto::from(probe))
}

pub fn graph_pick_target_param_on_graph(
    state: &SharedAppState,
    graph: GraphSnapshotDto,
    from: GridPos,
    to_node: GridPos,
    target: CadenceGraphTarget,
    to_param: Option<String>,
) -> Result<GraphPickTargetParamDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let project = active_project(&store).map_err(|err| err.to_string())?;
    let engine = engine_for_target(project, &target);
    let mut graph = crate::adapter::tessera::decode_graph_snapshot(graph)?;
    normalize_graph_piece_sides(&mut graph, engine.registry());
    let probe = engine.probe_edge(&graph, &from, &to_node, to_param.as_deref());
    Ok(GraphPickTargetParamDto::from(probe))
}

pub fn graph_compile_preview(
    state: &SharedAppState,
    target: Option<CadenceGraphTarget>,
) -> Result<GraphCompilePreviewDto, String> {
    let target = target.unwrap_or_default();
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let (project, graph, engine) = {
        normalize_and_invalidate(&mut store, &target)?;
        let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
        let graph = graph_for_target(project, &target)?.clone();
        let engine = engine_for_target(project, &target);
        (project.clone(), graph, engine)
    };
    let cache = store.compile_cache_for_target_mut(&target);
    Ok(compile_preview_cached(&project, &target, &graph, &engine, cache).1)
}

pub fn project_compile_preview(state: &SharedAppState) -> Result<ProjectCompilePreviewDto, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    let changed = {
        let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
        let mut changed = normalize_project_piece_sides(project);
        changed |= normalize_legacy_control_input_shape(project.runtime_graph_mut());
        for trick in project.tricks_mut() {
            changed |= normalize_legacy_control_input_shape(&mut trick.graph);
        }
        changed
    };
    if changed {
        store.clear_compile_caches();
    }
    let project = active_project_mut(&mut store).map_err(|err| err.to_string())?;
    let compiled = compile_project(project);
    let diagnostics =
        crate::adapter::transport::translate(cadence_diagnostics(compiled.diagnostics))
            .expect("cadence diagnostics should serialize into DTO diagnostics");
    Ok(ProjectCompilePreviewDto {
        can_render: compiled.can_render,
        can_play: compiled.can_play,
        diagnostics,
        preview: compiled.preview,
    })
}

pub fn graph_apply_ops(
    state: &SharedAppState,
    ops: Vec<GraphOp>,
    request_id: Option<String>,
    target: CadenceGraphTarget,
) -> Result<GraphApplyResultDto, Vec<DiagnosticDto>> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| Vec::<DiagnosticDto>::new())?;
    let engine = {
        let project = active_project(&store).map_err(|_| Vec::<DiagnosticDto>::new())?;
        engine_for_target(project, &target)
    };

    if ops.is_empty() {
        let (project, graph) = {
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
            (
                project.clone(),
                graph_for_target(project, &target)
                    .map_err(|_| Vec::<DiagnosticDto>::new())?
                    .clone(),
            )
        };
        let cache = store.compile_cache_for_target_mut(&target);
        let (analyzed, preview) = compile_preview_cached(&project, &target, &graph, &engine, cache);
        return Ok(GraphApplyResultDto {
            graph: encode_graph_snapshot(&graph),
            semantic: encode_semantic_snapshot(&analyzed),
            preview,
            removed_edges: Vec::new(),
        });
    }

    let (project, graph, analyzed, preview, removed_edges, record, did_mutate) = {
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
        let (candidate, outcome, analyzed) =
            apply_ops_transaction(&current, &engine, ops.as_slice(), cache)?;
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
        let preview =
            preview_dto_from_lowered(&analyzed, lower_target_graph(project, &target, &analyzed));
        (
            project.clone(),
            candidate,
            analyzed,
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
    store.push_diagnostic(
        "graph_apply_ops",
        request_id
            .map(|request_id| format!("request_id={request_id}"))
            .unwrap_or_else(|| "request_id=none".to_string()),
    );

    let _ = project;
    Ok(GraphApplyResultDto {
        graph: encode_graph_snapshot(&graph),
        semantic: encode_semantic_snapshot(&analyzed),
        preview,
        removed_edges: removed_edges.iter().map(encode_graph_edge).collect(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;
    use crate::{
        adapter::tessera::piece_registry::default_cadence_registry,
        application::project::ops::project_new_internal,
        domain::project::{CadenceGraphTarget, CadenceProjectDocument},
        infrastructure::dto::SharedAppState,
    };
    use serde_json::Value;
    use tessera::graph::{Edge, Graph, Node};
    use tessera::types::{EdgeId, GridPos, TileSide};

    #[test]
    fn graph_piece_catalog_matches_runtime_registry_and_exposes_new_authoring_tiles() {
        let state = SharedAppState::new();
        {
            let mut store = state.store.lock().expect("app state lock");
            project_new_internal(&mut store, Some("Catalog".to_string())).expect("seed project");
        }

        let ids = graph_piece_catalog(&state, None)
            .expect("catalog")
            .into_iter()
            .map(|piece| piece.id)
            .collect::<BTreeSet<_>>();

        let expected = default_cadence_registry()
            .visible_defs("cadence")
            .into_iter()
            .map(|piece| piece.id)
            .filter(|piece_id| !piece_is_hidden_graph_catalog_entry(piece_id.as_str()))
            .collect::<BTreeSet<_>>();

        assert_eq!(ids, expected, "graph catalog drifted from runtime registry");

        for supported in [
            "cadence.container.basic",
            "cadence.container.subdivide",
            "cadence.container.alternate",
            "cadence.container.parallel",
            "cadence.atom.note",
            "cadence.atom.scalar",
            "cadence.atom.rest",
            "cadence.atom.operator.elongation",
            "cadence.atom.operator.pitch_shift",
            "cadence.atom.operator.slow",
            "cadence.atom.operator.fast",
            "cadence.control_input",
            "args_connector",
            "cadence.add",
            "cadence.scale",
            "cadence.clip",
            "cadence.jux_by",
            "cadence.fast",
            "cadence.slow",
            "cadence.rev",
        ] {
            assert!(
                ids.contains(supported),
                "missing supported piece {supported}"
            );
        }

        for unsupported in ["cadence.number", "cadence.text", "cadence.pattern_notation"] {
            assert!(
                !ids.contains(unsupported),
                "unsupported piece leaked into catalog: {unsupported}"
            );
        }
    }

    #[test]
    fn graph_snapshot_normalizes_legacy_control_input_shape_and_edges() {
        let state = SharedAppState::new();
        {
            let mut store = state.store.lock().expect("app state lock");
            store.current_project = Some(CadenceProjectDocument::new(
                "Legacy Control".to_string(),
                Graph {
                    nodes: BTreeMap::from([
                        (
                            GridPos { col: 0, row: 0 },
                            Node {
                                piece_id: "cadence.control_input".to_string(),
                                inline_params: BTreeMap::from([
                                    ("value".to_string(), Value::String("0.25".to_string())),
                                    ("min".to_string(), Value::String("0.0".to_string())),
                                    ("max".to_string(), Value::String("1.0".to_string())),
                                    ("step".to_string(), Value::String("0.1".to_string())),
                                ]),
                                pattern_source: None,
                                input_sides: BTreeMap::new(),
                                output_side: Some(TileSide::TOP),
                                label: None,
                                node_state: None,
                            },
                        ),
                        (
                            GridPos { col: 1, row: 0 },
                            Node {
                                piece_id: "cadence.number".to_string(),
                                inline_params: BTreeMap::from([(
                                    "value".to_string(),
                                    Value::String("0.5".to_string()),
                                )]),
                                pattern_source: None,
                                input_sides: BTreeMap::new(),
                                output_side: Some(TileSide::TOP),
                                label: None,
                                node_state: None,
                            },
                        ),
                    ]),
                    edges: BTreeMap::from([(
                        EdgeId::new(),
                        Edge {
                            id: EdgeId::new(),
                            from: GridPos { col: 1, row: 0 },
                            to_node: GridPos { col: 0, row: 0 },
                            to_param: "min".to_string(),
                        },
                    )]),
                    name: "Legacy Control".to_string(),
                    cols: 4,
                    rows: 4,
                },
            ));
        }

        let snapshot = graph_snapshot(&state, Some(CadenceGraphTarget::Runtime)).expect("snapshot");
        let control = snapshot
            .nodes
            .iter()
            .find(|node| node.position == crate::domain::common::GridPos { col: 0, row: 0 })
            .expect("control node");

        assert!(
            snapshot.edges.is_empty(),
            "legacy min edge should be removed"
        );
        assert!(!control.inline_params.contains_key("min"));
        assert!(!control.inline_params.contains_key("max"));
        assert!(!control.inline_params.contains_key("step"));
        assert_eq!(
            control.node_state,
            Some(serde_json::json!({
                "control_input_shape": {
                    "min": "0.0",
                    "max": "1.0",
                    "step": "0.1",
                }
            }))
        );
    }
}
