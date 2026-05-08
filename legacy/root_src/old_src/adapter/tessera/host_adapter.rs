//! Cadence-owned orchestration around the Tessera kernel.

use std::sync::Arc;

use crate::adapter::tessera::{
    graph_defaults::normalize_graph_piece_sides, piece_registry::register_cadence_pieces,
};
use crate::domain::project::CadenceProjectDocument;
use tessera::{
    AnalysisCache, AnalyzedGraph, Diagnostic, Graph, GraphOp,
    ops::{
        ApplyOpsOutcome, EdgeTargetParamProbe, apply_ops_to_graph, apply_ops_to_graph_cached,
        probe_edge_connect,
    },
    piece_registry::PieceRegistry,
    subgraph::{SubgraphSignature, analyze_subgraph, subgraph_editor_pieces, subgraph_pieces},
};

#[derive(Debug, Clone)]
pub(crate) struct AnalyzedTrick {
    pub id: String,
    pub name: String,
    pub signature: SubgraphSignature,
}

pub(crate) struct CadenceGraphEngine {
    registry: PieceRegistry,
}

impl CadenceGraphEngine {
    pub(crate) fn new(registry: PieceRegistry) -> Self {
        Self { registry }
    }

    pub(crate) fn registry(&self) -> &PieceRegistry {
        &self.registry
    }

    pub(crate) fn analyze(&self, graph: &Graph) -> AnalyzedGraph {
        tessera::semantic::semantic_pass(graph, &self.registry)
    }

    pub(crate) fn analyze_cached(&self, graph: &Graph, cache: &mut AnalysisCache) -> AnalyzedGraph {
        tessera::semantic::analyze_cached(graph, &self.registry, cache)
    }

    pub(crate) fn apply_ops_cached(
        &self,
        graph: &mut Graph,
        ops: &[GraphOp],
        cache: &mut AnalysisCache,
    ) -> Result<ApplyOpsOutcome, Vec<Diagnostic>> {
        apply_ops_to_graph_cached(graph, &self.registry, ops, cache)
    }

    pub(crate) fn probe_edge(
        &self,
        graph: &Graph,
        from: &tessera::types::GridPos,
        to_node: &tessera::types::GridPos,
        to_param: Option<&str>,
    ) -> EdgeTargetParamProbe {
        probe_edge_connect(graph, &self.registry, from, to_node, to_param)
    }

    pub(crate) fn apply_ops(
        &self,
        graph: &mut Graph,
        ops: &[GraphOp],
    ) -> Result<ApplyOpsOutcome, Vec<Diagnostic>> {
        apply_ops_to_graph(graph, &self.registry, ops)
    }
}

/// Build a runtime graph engine using tricks defined on the current project.
pub(crate) fn runtime_engine(project: &CadenceProjectDocument) -> CadenceGraphEngine {
    CadenceGraphEngine::new(build_runtime_registry(analyze_tricks(project).as_slice()))
}

/// Build the specialized engine used while editing a trick graph.
pub(crate) fn subgraph_editor_engine() -> CadenceGraphEngine {
    CadenceGraphEngine::new(build_subgraph_editor_registry())
}

/// Analyze the current trick definitions into stable subgraph signatures.
pub(crate) fn analyze_tricks(project: &CadenceProjectDocument) -> Vec<AnalyzedTrick> {
    let editor_registry = build_subgraph_editor_registry();
    project
        .tricks()
        .iter()
        .filter_map(|trick| {
            analyze_subgraph(&trick.graph, &editor_registry)
                .ok()
                .map(|signature| AnalyzedTrick {
                    id: trick.id.clone(),
                    name: trick.name.clone(),
                    signature,
                })
        })
        .collect()
}

/// Backfill missing per-node side assignments for runtime and trick graphs.
pub(crate) fn normalize_project_piece_sides(project: &mut CadenceProjectDocument) -> bool {
    let runtime = runtime_engine(project);
    let mut changed = normalize_graph_piece_sides(project.runtime_graph_mut(), runtime.registry());

    let subgraph = subgraph_editor_engine();
    for trick in project.tricks_mut() {
        changed |= normalize_graph_piece_sides(&mut trick.graph, subgraph.registry());
    }

    changed
}

fn build_runtime_registry(tricks: &[AnalyzedTrick]) -> PieceRegistry {
    let mut registry = PieceRegistry::new();
    register_cadence_pieces(&mut registry);
    let defs = tricks
        .iter()
        .map(|trick| {
            (
                trick.id.as_str(),
                trick.name.as_str(),
                trick.signature.clone(),
            )
        })
        .collect::<Vec<_>>();
    for piece in subgraph_pieces(defs.as_slice()) {
        registry.register(piece);
    }
    registry
}

fn build_subgraph_editor_registry() -> PieceRegistry {
    let mut registry = PieceRegistry::new();
    register_cadence_pieces(&mut registry);
    for piece in subgraph_editor_pieces() {
        let id = piece.def().id.clone();
        let piece: Arc<dyn tessera::piece::Piece> = Arc::from(piece);
        registry.register_arc(id, piece);
    }
    registry
}
