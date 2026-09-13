//! Pure connection operations over the program derived from a Musaic document.

use tessera::prelude::{
    AuthoredTesseraProgram, BoardError, InputEndpoint, NodeId, OutputEndpoint, SpatialSide,
    StreamShape, TesseraCompiler,
};

use crate::domain::board::BoardSlot;

use super::PortSlotState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentConnectionView {
    pub kind: PortSlotState,
    pub from: NodeId,
    pub to: NodeId,
    pub spatial_side: SpatialSide,
}

pub fn spatial_side_between(from: BoardSlot, to: BoardSlot) -> SpatialSide {
    match (to.x - from.x, to.y - from.y) {
        (1, 0) => SpatialSide::East,
        (-1, 0) => SpatialSide::West,
        (0, 1) => SpatialSide::South,
        (0, -1) => SpatialSide::North,
        _ => SpatialSide::East,
    }
}

pub fn opposite_spatial_side(side: SpatialSide) -> SpatialSide {
    side.opposite()
}

pub fn neighbor_at_side(
    program: &AuthoredTesseraProgram,
    node: &NodeId,
    side: SpatialSide,
) -> Option<NodeId> {
    let placement = program.root_surface.placements.get(node)?;
    let adjacent_cells = placement.footprint.edge_adjacent(placement.slot, side);
    program
        .root_surface
        .placements
        .iter()
        .find_map(|(candidate, candidate_place)| {
            (candidate != node
                && adjacent_cells.iter().any(|cell| {
                    candidate_place
                        .footprint
                        .occupies(candidate_place.slot, *cell)
                }))
            .then(|| candidate.clone())
        })
}

pub fn connections_from_program(program: &AuthoredTesseraProgram) -> Vec<DocumentConnectionView> {
    let mut connections = Vec::new();
    let compiler = TesseraCompiler::new();
    for edge in super::connection_policy::endpoint_connections(program) {
        let side = super::connection_policy::effective_bindings(program, &edge.from)
            .outputs
            .get(&edge.output)
            .copied()
            .unwrap_or(SpatialSide::Off);
        if side.is_enabled()
            && !connections
                .iter()
                .any(|connection: &DocumentConnectionView| {
                    connection.from == edge.from
                        && connection.to == edge.to
                        && connection.spatial_side == side
                })
        {
            let kind = match compiler.authored_output_shape(program, &edge.from, &edge.output) {
                Some(StreamShape::ScalarPattern | StreamShape::ControlPattern) => {
                    PortSlotState::Input
                }
                _ => PortSlotState::Output,
            };
            connections.push(DocumentConnectionView {
                kind,
                from: edge.from,
                to: edge.to,
                spatial_side: side,
            });
        }
    }
    connections
}

pub fn connection_exists(program: &AuthoredTesseraProgram, from: &NodeId, to: &NodeId) -> bool {
    connections_from_program(program)
        .iter()
        .any(|connection| &connection.from == from && &connection.to == to)
}

pub fn bind_tiles(
    program: &mut AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
    side: SpatialSide,
) -> Result<(), BoardError> {
    let edge =
        super::authorize_connection(program, from, to).map_err(|_| BoardError::UnknownTile)?;
    if edge.side != side {
        return Err(BoardError::UnknownTile);
    }
    bind_authorized_edge(program, from, to, &edge)
}

pub fn bind_authorized_edge(
    program: &mut AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
    edge: &super::AuthoredEdge,
) -> Result<(), BoardError> {
    if !program.root_surface.nodes.contains_key(from)
        || !program.root_surface.nodes.contains_key(to)
    {
        return Err(BoardError::UnknownTile);
    }
    super::connection_policy::apply_edge(program, from, to, edge);
    Ok(())
}

pub fn unbind_connection(
    program: &mut AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
) -> Result<(), BoardError> {
    let connections = super::connection_policy::endpoint_connections(program);
    let removed: Vec<_> = connections
        .iter()
        .filter(|connection| &connection.from == from && &connection.to == to)
        .cloned()
        .collect();
    if removed.is_empty() {
        return Err(BoardError::UnknownTile);
    }
    let mut target = super::connection_policy::effective_bindings(program, to);
    let mut source = super::connection_policy::effective_bindings(program, from);
    for edge in &removed {
        target.inputs.insert(edge.input.clone(), SpatialSide::Off);
        if !connections.iter().any(|other| {
            other.from == edge.from && other.output == edge.output && other.to != edge.to
        }) {
            source.outputs.insert(edge.output.clone(), SpatialSide::Off);
        }
    }
    program.root_surface.explicit_relations.retain(|relation| {
        let edge = super::connection_policy::explicit_connection(relation);
        &edge.from != from || &edge.to != to
    });
    program.root_surface.bindings.insert(to.clone(), target);
    program.root_surface.bindings.insert(from.clone(), source);
    Ok(())
}

pub fn unbind_output_side(
    program: &mut AuthoredTesseraProgram,
    node: &NodeId,
    side: SpatialSide,
) -> Result<Option<NodeId>, BoardError> {
    if !program.root_surface.nodes.contains_key(node) {
        return Err(BoardError::UnknownTile);
    }
    let neighbor = neighbor_at_side(program, node, side);
    if let Some(output) = default_output_on_side(program, node, side) {
        let mut bindings = super::connection_policy::effective_bindings(program, node);
        bindings.outputs.insert(output, SpatialSide::Off);
        program.root_surface.bindings.insert(node.clone(), bindings);
    }
    if let Some(neighbor_id) = &neighbor
        && let Some(input) = default_input_on_side(program, neighbor_id, side.opposite())
    {
        let mut bindings = super::connection_policy::effective_bindings(program, neighbor_id);
        bindings.inputs.insert(input, SpatialSide::Off);
        program
            .root_surface
            .bindings
            .insert(neighbor_id.clone(), bindings);
    }
    Ok(neighbor)
}

fn default_output_on_side(
    program: &AuthoredTesseraProgram,
    node: &NodeId,
    side: SpatialSide,
) -> Option<OutputEndpoint> {
    super::connection_policy::effective_bindings(program, node)
        .outputs
        .into_iter()
        .find_map(|(endpoint, bound)| (bound == side).then_some(endpoint))
}

fn default_input_on_side(
    program: &AuthoredTesseraProgram,
    node: &NodeId,
    side: SpatialSide,
) -> Option<InputEndpoint> {
    super::connection_policy::effective_bindings(program, node)
        .inputs
        .into_iter()
        .find_map(|(endpoint, bound)| (bound == side).then_some(endpoint))
}

pub use super::export::{
    empty_container_stack, export_container_stack, export_container_stack_excluding,
    export_container_stack_with_insert, map_container_kind_for_document, stack_nodes_on_surface,
};
