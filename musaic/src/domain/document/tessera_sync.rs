use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tessera::{
    bevy::TesseraBoard,
    prelude::{
        AtomTile, AuthoredTesseraProgram, AuthoredTesseraProgramExt, BoardError, Container,
        ContainerId, ContainerSurfaceTile, InputEndpoint, NodeId, OutputEndpoint, OutputPort,
        RootSurfaceNodeKind, SpatialSide,
    },
};

use crate::domain::board::{BoardSlot, BoardSurfaceId};

use super::export::{
    export_document_to_board, finish_board_export, map_tessera_container_kind_to_document,
};
use super::graph::{
    AtomValue, DocumentNode, DocumentNodeKind, PlacementAddress, StackIndex, TileSpawnKind,
};
use super::{DocumentEdgeId, MusaicDocument, PortSlotState, default_connection_kind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct OutputPortName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
struct InputPortName(pub String);

// ---------------------------------------------------------------------------
// Legacy connection store (deserialize-only migration from pre–Phase-2 projects)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LegacyConnectionStore {
    #[serde(default)]
    pub by_id: BTreeMap<DocumentEdgeId, LegacyAuthoredConnection>,
    #[serde(default)]
    next_edge_id: u64,
}

impl LegacyConnectionStore {
    pub fn iter(&self) -> impl Iterator<Item = &LegacyAuthoredConnection> {
        self.by_id.values()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    pub fn clear(&mut self) {
        self.by_id.clear();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegacyAuthoredConnection {
    pub id: DocumentEdgeId,
    pub from: NodeId,
    pub to: NodeId,
    #[serde(default = "default_output_port")]
    pub from_port: OutputPortName,
    #[serde(default = "default_input_port")]
    pub to_port: InputPortName,
    #[serde(default = "default_spatial_side")]
    pub spatial_side: SpatialSide,
}

fn default_output_port() -> OutputPortName {
    OutputPortName("main".into())
}

fn default_input_port() -> InputPortName {
    InputPortName("main".into())
}

fn default_spatial_side() -> SpatialSide {
    SpatialSide::East
}

/// Converts persisted ConnectionStore edges into tessera output bindings.
pub fn migrate_legacy_connections(document: &mut MusaicDocument) {
    if document.legacy_connections.is_empty() {
        return;
    }

    let connections = document
        .legacy_connections
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let (program, port_endpoints) = (
        &mut document.tessera.authored_program,
        &mut document.port_endpoints,
    );
    for connection in connections {
        program.bind_output_side(
            connection.from.0.clone(),
            OutputEndpoint::Socket(OutputPort::new(connection.from_port.0.clone())),
            connection.spatial_side,
        );
        let kind = program
            .root_surface
            .placements
            .get(&connection.from)
            .zip(program.root_surface.placements.get(&connection.to))
            .map(|(from, to)| default_connection_kind(from.slot.x, to.slot.x))
            .unwrap_or(PortSlotState::Input);
        port_endpoints.set_side(&connection.from, connection.spatial_side, kind);
        port_endpoints.set_side(&connection.to, connection.spatial_side.opposite(), kind);
    }
    document.legacy_connections.clear();
}

// ---------------------------------------------------------------------------
// Board ↔ document sync
// ---------------------------------------------------------------------------

/// Rebuilds a live [`TesseraBoard`] from the document graph and preserved tessera bindings.
pub fn hydrate_board_from_document(
    board: &mut TesseraBoard,
    document: &MusaicDocument,
) -> Result<(), BoardError> {
    let export = export_document_to_board(document)?;
    let mut program = finish_board_export(export);

    for (node_id, bindings) in &document.tessera.authored_program.root_surface.bindings {
        if program.root_surface.placements.contains_key(node_id) {
            program
                .root_surface
                .bindings
                .insert(node_id.clone(), bindings.clone());
        }
    }

    *board = TesseraBoard::from_program(program);
    board.mark_dirty();
    Ok(())
}

/// Copies the live board program into the document tessera state (no revision bump).
pub fn sync_document_tessera_from_board(document: &mut MusaicDocument, board: &TesseraBoard) {
    document.tessera.authored_program = board.authored_program();
}

/// Updates a single root board cell in the document graph from the tessera board.
pub fn sync_root_slot_to_document(
    document: &mut MusaicDocument,
    board: &TesseraBoard,
    slot: BoardSlot,
) -> Result<(), BoardError> {
    let program = board.authored_program();
    let root = document.root_surface;

    if let Some(node_id) = document.graph.node_at_board_slot(root, slot) {
        if let Ok(deleted) = document
            .graph
            .delete_subtree(&node_id, &mut document.surfaces)
        {
            document.tiles.apply_patch(&document.graph, &[], &deleted);
        }
    }

    if let Some(tile) = board.tile_at(slot) {
        let Some(node_kind) = program.root_surface.nodes.get(&tile.id) else {
            sync_document_tessera_from_board(document, board);
            document.bump_revision();
            return Ok(());
        };
        place_document_root_node(document, &tile.id, slot, node_kind, &program.containers)?;
        document.sync_tile_store_from_graph();
    }

    sync_document_tessera_from_board(document, board);
    document.bump_revision();
    Ok(())
}

/// Projects the full tessera authored program back into the musaic document graph.
pub fn sync_authored_program_to_document(
    document: &mut MusaicDocument,
    program: &AuthoredTesseraProgram,
) -> Result<(), BoardError> {
    let root = document.root_surface;

    let existing_root_nodes = document
        .graph
        .nodes_on_surface(root)
        .into_iter()
        .filter_map(|(location, node)| match location.address {
            PlacementAddress::BoardSlot(_) => Some(node.id.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();

    for node_id in existing_root_nodes {
        let _ = document
            .graph
            .delete_subtree(&node_id, &mut document.surfaces);
    }

    for (node_id, placement) in &program.root_surface.placements {
        let slot = placement.slot;
        let Some(node_kind) = program.root_surface.nodes.get(node_id) else {
            continue;
        };
        place_document_root_node(document, node_id, slot, node_kind, &program.containers)?;
    }

    document.tessera.authored_program = program.clone();
    document.sync_tile_store_from_graph();
    document.bump_revision();
    Ok(())
}

fn place_document_root_node(
    document: &mut MusaicDocument,
    node_id: &NodeId,
    slot: BoardSlot,
    node_kind: &RootSurfaceNodeKind,
    containers: &BTreeMap<ContainerId, Container>,
) -> Result<(), BoardError> {
    let spawn = match node_kind {
        RootSurfaceNodeKind::Container { container } => {
            let Some(container) = containers.get(container) else {
                return Ok(());
            };
            TileSpawnKind::Container {
                kind: map_tessera_container_kind_to_document(container.kind),
            }
        }
        RootSurfaceNodeKind::Output(output) => TileSpawnKind::Output {
            name: output.label.clone().unwrap_or_else(|| "main".into()),
        },
        RootSurfaceNodeKind::Transform(_) | RootSurfaceNodeKind::FlowControl(_) => {
            TileSpawnKind::TrickInstance {
                prototype: crate::domain::document::graph::TilePrototypeId(0),
            }
        }
    };

    document.graph.reserve_node_id(node_id);
    document
        .graph
        .insert_tile_at_id(
            &mut document.surfaces,
            document.root_surface,
            PlacementAddress::BoardSlot(slot),
            node_id.clone(),
            spawn,
        )
        .map_err(|_| BoardError::UnknownTile)?;

    if let RootSurfaceNodeKind::Container { container } = node_kind {
        sync_container_stack_to_document(document, node_id, container, containers)?;
    }

    Ok(())
}

fn sync_container_stack_to_document(
    document: &mut MusaicDocument,
    container_node: &NodeId,
    container_id: &ContainerId,
    containers: &BTreeMap<ContainerId, Container>,
) -> Result<(), BoardError> {
    let Some(local_surface) = document.graph.container_surface(container_node) else {
        return Ok(());
    };

    let preserved_atoms = document
        .graph
        .nodes_on_surface(local_surface)
        .into_iter()
        .filter_map(|(location, node)| {
            let PlacementAddress::StackIndex(index) = location.address else {
                return None;
            };
            let DocumentNodeKind::Atom(atom) = &node.kind else {
                return None;
            };
            matches!(atom.atom, AtomValue::Octave(_) | AtomValue::Accidental(_))
                .then(|| (index, node.clone()))
        })
        .collect::<Vec<_>>();

    let existing = document
        .graph
        .nodes_on_surface(local_surface)
        .into_iter()
        .map(|(_, node)| node.id.clone())
        .collect::<Vec<_>>();
    for node_id in existing {
        let _ = document
            .graph
            .delete_subtree(&node_id, &mut document.surfaces);
    }

    let Some(container) = containers.get(container_id) else {
        return Ok(());
    };

    import_stack_tiles(
        document,
        local_surface,
        container_node,
        &container.stack,
        containers,
    )?;

    for (index, node) in preserved_atoms {
        if document
            .graph
            .node_at_stack_index(local_surface, index)
            .is_some()
        {
            continue;
        }
        let Some(spawn) = atom_node_to_spawn(&node) else {
            continue;
        };
        document.graph.reserve_node_id(&node.id);
        let _ = document.graph.insert_tile_at_id(
            &mut document.surfaces,
            local_surface,
            PlacementAddress::StackIndex(index),
            node.id.clone(),
            spawn,
        );
    }

    Ok(())
}

fn atom_node_to_spawn(node: &DocumentNode) -> Option<TileSpawnKind> {
    match &node.kind {
        DocumentNodeKind::Atom(atom) => Some(TileSpawnKind::Atom {
            atom: atom.atom.clone(),
        }),
        _ => None,
    }
}

fn import_stack_tiles(
    document: &mut MusaicDocument,
    surface: BoardSurfaceId,
    container_node: &NodeId,
    tiles: &[ContainerSurfaceTile],
    containers: &BTreeMap<ContainerId, Container>,
) -> Result<(), BoardError> {
    let mut index = 0usize;
    while index < tiles.len() {
        match &tiles[index] {
            ContainerSurfaceTile::Atom(atom_tile) => {
                if matches!(atom_tile, AtomTile::Rest) {
                    index += 1;
                    continue;
                }
                if let Some(spawn) = atom_tile_to_spawn(atom_tile) {
                    let node_id = NodeId::new(format!("{}_{}", container_node.0, index));
                    document.graph.reserve_node_id(&node_id);
                    let _ = document.graph.insert_tile_at_id(
                        &mut document.surfaces,
                        surface,
                        PlacementAddress::StackIndex(StackIndex(index)),
                        node_id,
                        spawn,
                    );
                }
                index += 1;
            }
            ContainerSurfaceTile::Transform | ContainerSurfaceTile::Output => {
                index += 1;
            }
            ContainerSurfaceTile::NestedContainer(nested_id) => {
                if let Some(nested) = containers.get(nested_id) {
                    let node_id = document.graph.allocate_node_id();
                    document.graph.reserve_node_id(&node_id);
                    document
                        .graph
                        .insert_tile_at_id(
                            &mut document.surfaces,
                            surface,
                            PlacementAddress::StackIndex(StackIndex(index)),
                            node_id.clone(),
                            TileSpawnKind::Container {
                                kind: map_tessera_container_kind_to_document(nested.kind),
                            },
                        )
                        .map_err(|_| BoardError::UnknownTile)?;
                    sync_container_stack_to_document(document, &node_id, nested_id, containers)?;
                }
                index += 1;
            }
        }
    }
    Ok(())
}

fn atom_tile_to_spawn(atom: &AtomTile) -> Option<TileSpawnKind> {
    Some(match atom {
        AtomTile::Note(note) => TileSpawnKind::Atom {
            atom: AtomValue::NoteName(note_letter_from_tessera(note)),
        },
        AtomTile::Rest => TileSpawnKind::Atom {
            atom: AtomValue::Rest,
        },
        AtomTile::Scalar(scalar) => TileSpawnKind::Atom {
            atom: AtomValue::Number(scalar.value.numerator as i32),
        },
        AtomTile::Operator(_) => return None,
    })
}

fn note_letter_from_tessera(
    note: &tessera::prelude::NoteAtom,
) -> crate::domain::document::graph::NoteName {
    use crate::domain::document::graph::NoteName;
    match note.label.as_str() {
        "a" => NoteName::A,
        "b" => NoteName::B,
        "c" => NoteName::C,
        "d" => NoteName::D,
        "e" => NoteName::E,
        "f" => NoteName::F,
        _ => NoteName::G,
    }
}

// ---------------------------------------------------------------------------
// Spatial connection helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovedBoardBinding {
    pub from: NodeId,
    pub to: NodeId,
    pub side: SpatialSide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredBoardConnection {
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
            if candidate == node {
                return None;
            }
            adjacent_cells
                .iter()
                .any(|cell| {
                    candidate_place
                        .footprint
                        .occupies(candidate_place.slot, *cell)
                })
                .then(|| candidate.clone())
        })
}

pub fn connections_from_program(
    program: &AuthoredTesseraProgram,
    port_endpoints: &super::PortEndpointStore,
) -> Vec<AuthoredBoardConnection> {
    let mut connections = Vec::new();
    for (from, bindings) in &program.root_surface.bindings {
        for (_endpoint, side) in &bindings.outputs {
            if !side.is_enabled() {
                continue;
            }
            if !port_endpoints.side_state(from, *side).is_bindable() {
                continue;
            }
            let Some(to) = neighbor_at_side(program, from, *side) else {
                continue;
            };
            connections.push(AuthoredBoardConnection {
                from: from.clone(),
                to,
                spatial_side: *side,
            });
        }
    }
    connections
}

pub fn connection_exists(
    program: &AuthoredTesseraProgram,
    port_endpoints: &super::PortEndpointStore,
    from: &NodeId,
    to: &NodeId,
) -> bool {
    connections_from_program(program, port_endpoints)
        .iter()
        .any(|c| &c.from == from && &c.to == to)
}

/// Binds spatial output/input endpoints between two existing board tiles.
pub fn bind_tiles_on_board(
    board: &mut TesseraBoard,
    from: &NodeId,
    to: &NodeId,
    side: SpatialSide,
) -> Result<(), BoardError> {
    let from_ref = board.tile(from).ok_or(BoardError::UnknownTile)?;
    let to_ref = board.tile(to).ok_or(BoardError::UnknownTile)?;
    let program = board.authored_program();
    let opposite = side.opposite();

    if let Some(output) = default_output_on_side(&program, from, side) {
        board.bind_output_side(&from_ref, output, side)?;
    }
    if let Some(input) = default_input_on_side(&program, to, opposite) {
        board.bind_input_side(&to_ref, input, opposite)?;
    }
    Ok(())
}

/// Clears spatial bindings on the given side for a tile and its neighbor.
pub fn unbind_output_side_on_board(
    board: &mut TesseraBoard,
    node: &NodeId,
    side: SpatialSide,
) -> Result<Option<NodeId>, BoardError> {
    let tile_ref = board.tile(node).ok_or(BoardError::UnknownTile)?;
    let program = board.authored_program();
    let neighbor = neighbor_at_side(&program, node, side);

    if let Some(output) = default_output_on_side(&program, node, side) {
        board.bind_output_side(&tile_ref, output, SpatialSide::Off)?;
    }
    if let Some(neighbor_id) = &neighbor {
        if let Some(neighbor_ref) = board.tile(neighbor_id) {
            let opposite = side.opposite();
            if let Some(input) = default_input_on_side(&program, neighbor_id, opposite) {
                board.bind_input_side(&neighbor_ref, input, SpatialSide::Off)?;
            }
        }
    }
    Ok(neighbor)
}

fn default_output_on_side(
    program: &AuthoredTesseraProgram,
    node_id: &NodeId,
    side: SpatialSide,
) -> Option<OutputEndpoint> {
    if let Some(bindings) = program.root_surface.bindings.get(node_id) {
        for (endpoint, bound_side) in &bindings.outputs {
            if *bound_side == side {
                return Some(endpoint.clone());
            }
        }
    }
    let node = program.root_surface.nodes.get(node_id)?;
    tessera::prelude::default_spatial_bindings(node)
        .outputs
        .into_iter()
        .find_map(|(endpoint, bound_side)| (bound_side == side).then_some(endpoint))
}

fn default_input_on_side(
    program: &AuthoredTesseraProgram,
    node_id: &NodeId,
    side: SpatialSide,
) -> Option<InputEndpoint> {
    if let Some(bindings) = program.root_surface.bindings.get(node_id) {
        for (endpoint, bound_side) in &bindings.inputs {
            if *bound_side == side {
                return Some(endpoint.clone());
            }
        }
    }
    let node = program.root_surface.nodes.get(node_id)?;
    tessera::prelude::default_spatial_bindings(node)
        .inputs
        .into_iter()
        .find_map(|(endpoint, bound_side)| (bound_side == side).then_some(endpoint))
}

// Re-export container stack helpers used by editor transactions.
pub use super::export::{
    empty_container_stack, export_container_stack, export_container_stack_excluding,
    export_container_stack_with_insert, map_container_kind_for_document, stack_nodes_on_surface,
};
