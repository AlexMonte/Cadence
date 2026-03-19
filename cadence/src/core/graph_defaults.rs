use tessera::graph::Graph;
use tessera::piece_registry::PieceRegistry;

/// Seed missing persisted node sides from each piece definition.
///
/// Older Cadence graphs and some test fixtures relied on piece-definition
/// defaults rather than storing per-node side assignments. Tessera now expects
/// those assignments to be explicit on placed nodes, so we backfill them here
/// without overwriting any user-edited side choices.
pub fn normalize_graph_piece_sides(graph: &mut Graph, registry: &PieceRegistry) -> bool {
    let mut changed = false;

    for node in graph.nodes.values_mut() {
        let Some(piece) = registry.get(node.piece_id.as_str()) else {
            continue;
        };
        let def = piece.def();

        for param in &def.params {
            if node.input_sides.contains_key(param.id.as_str()) {
                continue;
            }
            node.input_sides.insert(param.id.clone(), param.side);
            changed = true;
        }

        if node.output_side.is_none() && def.output_side.is_some() {
            node.output_side = def.output_side;
            changed = true;
        }
    }

    changed
}
