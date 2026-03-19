//! Cadence's concrete Tessera host adapter and registry construction rules.

use std::sync::Arc;

use crate::commands::TerminalStrategy;
use crate::core::graph_defaults::normalize_graph_piece_sides;
use crate::core::piece_registry::register_strudel_pieces;
use crate::model::{CadenceProjectDocument, CadenceTrickDef};
use tessera::Expr;
use tessera::core_expression_pieces;
use tessera::host::{GraphEngine, HostAdapter};
use tessera::piece_registry::PieceRegistry;
use tessera::subgraph::{
    CompiledSubgraph, compile_subgraphs, subgraph_editor_pieces, subgraph_pieces,
};

#[derive(Debug, Clone)]
enum RuntimeSubgraphSource {
    Definitions(Vec<CadenceTrickDef>),
    Compiled(Vec<CompiledSubgraph>),
}

#[derive(Debug, Clone)]
enum CadenceHostMode {
    Runtime(RuntimeSubgraphSource),
    SubgraphEditor,
}

#[derive(Debug, Clone)]
/// Host adapter that decides which pieces exist and how terminal code is rendered.
pub(crate) struct CadenceHostAdapter {
    mode: CadenceHostMode,
    strategy: TerminalStrategy,
}

impl CadenceHostAdapter {
    fn runtime(project: &CadenceProjectDocument, strategy: TerminalStrategy) -> Self {
        Self {
            mode: CadenceHostMode::Runtime(RuntimeSubgraphSource::Definitions(
                project.init_stage.tricks.clone(),
            )),
            strategy,
        }
    }

    fn runtime_from_compiled(compiled: &[CompiledSubgraph], strategy: TerminalStrategy) -> Self {
        Self {
            mode: CadenceHostMode::Runtime(RuntimeSubgraphSource::Compiled(compiled.to_vec())),
            strategy,
        }
    }

    fn subgraph_editor(strategy: TerminalStrategy) -> Self {
        Self {
            mode: CadenceHostMode::SubgraphEditor,
            strategy,
        }
    }
}

impl HostAdapter for CadenceHostAdapter {
    fn create_registry(&self) -> PieceRegistry {
        match &self.mode {
            CadenceHostMode::Runtime(RuntimeSubgraphSource::Definitions(defs)) => {
                let editor_registry = build_subgraph_editor_registry();
                let (compiled, _) = compile_subgraphs(defs.as_slice(), &editor_registry);
                build_runtime_registry(compiled.as_slice())
            }
            CadenceHostMode::Runtime(RuntimeSubgraphSource::Compiled(compiled)) => {
                build_runtime_registry(compiled.as_slice())
            }
            CadenceHostMode::SubgraphEditor => build_subgraph_editor_registry(),
        }
    }

    fn render_terminals(&self, terminals: &[Expr]) -> Vec<String> {
        let rendered = self.strategy.as_renderer().render(terminals);
        if rendered.trim().is_empty() {
            Vec::new()
        } else {
            vec![rendered]
        }
    }
}

/// Build a runtime graph engine using tricks defined on the current project.
pub(crate) fn runtime_engine(
    project: &CadenceProjectDocument,
    strategy: TerminalStrategy,
) -> GraphEngine<CadenceHostAdapter> {
    GraphEngine::new(CadenceHostAdapter::runtime(project, strategy))
}

/// Build a runtime graph engine from already compiled trick definitions.
pub(crate) fn runtime_engine_from_compiled(
    compiled: &[CompiledSubgraph],
    strategy: TerminalStrategy,
) -> GraphEngine<CadenceHostAdapter> {
    GraphEngine::new(CadenceHostAdapter::runtime_from_compiled(
        compiled, strategy,
    ))
}

/// Build the specialized engine used while editing a trick graph.
pub(crate) fn subgraph_editor_engine(
    strategy: TerminalStrategy,
) -> GraphEngine<CadenceHostAdapter> {
    GraphEngine::new(CadenceHostAdapter::subgraph_editor(strategy))
}

/// Return the first non-empty rendered terminal section, if any.
pub(crate) fn rendered_output(rendered: Vec<String>) -> Option<String> {
    rendered.into_iter().find(|entry| !entry.trim().is_empty())
}

/// Backfill missing per-node side assignments for runtime and trick graphs.
pub(crate) fn normalize_project_piece_sides(project: &mut CadenceProjectDocument) -> bool {
    let runtime = runtime_engine(project, TerminalStrategy::Stack);
    let mut changed = normalize_graph_piece_sides(&mut project.graph, runtime.registry());

    let subgraph = subgraph_editor_engine(TerminalStrategy::Stack);
    for trick in &mut project.init_stage.tricks {
        changed |= normalize_graph_piece_sides(&mut trick.graph, subgraph.registry());
    }

    changed
}

fn build_runtime_registry(compiled: &[CompiledSubgraph]) -> PieceRegistry {
    let mut registry = PieceRegistry::new();
    register_boxed_pieces(&mut registry, core_expression_pieces());
    register_strudel_pieces(&mut registry);
    for piece in subgraph_pieces(compiled) {
        registry.register(piece);
    }
    registry
}

fn build_subgraph_editor_registry() -> PieceRegistry {
    let mut registry = PieceRegistry::new();
    register_boxed_pieces(&mut registry, core_expression_pieces());
    register_strudel_pieces(&mut registry);
    for piece in subgraph_editor_pieces() {
        let id = piece.def().id.clone();
        let piece: Arc<dyn tessera::piece::Piece> = Arc::from(piece);
        registry.register_arc(id, piece);
    }
    registry
}

fn register_boxed_pieces(
    registry: &mut PieceRegistry,
    pieces: Vec<Box<dyn tessera::piece::Piece>>,
) {
    for piece in pieces {
        let id = piece.def().id.clone();
        let piece: Arc<dyn tessera::piece::Piece> = Arc::from(piece);
        registry.register_arc(id, piece);
    }
}
