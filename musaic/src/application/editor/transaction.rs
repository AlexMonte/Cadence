use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tessera::{
    bevy::TesseraBoard,
    prelude::{BoardError, NodeId, SpatialSide},
};

use crate::application::command::EditorInverse;
use crate::application::pipeline::runtime::ProjectedEventId;
use crate::domain::board::{BoardSlot, BoardSurfaceId, BoardSurfaceKind};
use crate::domain::document::{
    AuthoredEdge, DocumentNode, DocumentNodeKind, DocumentQueries, MusaicDocument,
    PlacementAddress, RemovedBoardBinding, StackIndex, TileSpawnKind, authorize_connection,
    bind_tiles_on_board, capture_subtree_patch, connection_exists, cycle_port_state,
    empty_container_stack, export_container_stack_excluding, export_container_stack_with_insert,
    map_container_kind_for_document, opposite_spatial_side, root_board_tile_footprint,
    spatial_side_between, stack_nodes_on_surface, sync_document_tessera_from_board,
    sync_root_slot_to_document, unbind_output_side_on_board,
};
use crate::domain::transform::transform_kind_from_prototype;

use crate::application::editor::selection::{SelectionMode, SelectionState};
use crate::application::editor::workspace::{
    ActiveSurfaceChangeReason, ActiveSurfaceChanged, EditorAttention, FocusTarget, WorkspaceMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementTarget {
    BoardSlot {
        surface: BoardSurfaceId,
        slot: BoardSlot,
    },
    StackIndex {
        surface: BoardSurfaceId,
        index: StackIndex,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct EditorTransactionResult {
    pub status: TransactionStatus,
    pub invalidation: Invalidation,
    pub active_surface_change: Option<ActiveSurfaceChanged>,
    pub diagnostics: Vec<EditorTransactionDiagnostic>,
    /// Inverse to undo this transaction when accepted.
    pub undo: Option<EditorInverse>,
}

impl EditorTransactionResult {
    pub fn accepted(invalidation: Invalidation) -> Self {
        Self {
            status: TransactionStatus::Accepted,
            invalidation,
            active_surface_change: None,
            diagnostics: Vec::new(),
            undo: None,
        }
    }

    pub fn rejected(message: impl Into<String>) -> Self {
        Self {
            status: TransactionStatus::Rejected,
            invalidation: Invalidation::none(),
            active_surface_change: None,
            diagnostics: vec![EditorTransactionDiagnostic {
                severity: DiagnosticSeverity::Error,
                message: message.into(),
            }],
            undo: None,
        }
    }

    pub fn is_accepted(&self) -> bool {
        matches!(self.status, TransactionStatus::Accepted)
    }

    pub fn with_undo(mut self, undo: EditorInverse) -> Self {
        self.undo = Some(undo);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionStatus {
    Accepted,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Invalidation {
    pub document: bool,
    pub compile: bool,
    pub lower: bool,
    pub scene: bool,
    pub runtime: bool,
    pub save: bool,
    /// Document graph changed in a way that requires re-export into TesseraBoard before compile.
    pub needs_board_reexport: bool,
}

impl Invalidation {
    pub fn none() -> Self {
        Self::default()
    }

    pub fn document_changed() -> Self {
        Self {
            document: true,
            compile: true,
            lower: true,
            scene: true,
            runtime: true,
            save: true,
            needs_board_reexport: false,
        }
    }

    pub fn editor_only() -> Self {
        Self {
            scene: true,
            ..Self::default()
        }
    }

    /// Full document graph replacement (load, restore patch) — re-export board before compile.
    pub fn document_replaced() -> Self {
        Self {
            document: true,
            compile: true,
            lower: true,
            scene: true,
            runtime: true,
            save: true,
            needs_board_reexport: true,
        }
    }

    /// Board wiring changed; re-export connections onto Tessera before compile.
    pub fn connections_changed() -> Self {
        Self {
            compile: true,
            lower: true,
            scene: true,
            runtime: true,
            save: true,
            needs_board_reexport: true,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTransactionDiagnostic {
    pub severity: DiagnosticSeverity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

pub trait TimelineSourceResolver {
    fn source_for_event(&self, event: ProjectedEventId) -> Option<TimelineSource>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineSource {
    pub surface: BoardSurfaceId,
    pub node: NodeId,
}

pub fn map_board_error(error: BoardError) -> String {
    match error {
        BoardError::SlotOccupied => "That cell is already occupied.".into(),
        BoardError::UnknownTile => "That tile was removed or moved; reconnect or reselect.".into(),
        BoardError::DuplicateId { existing_slot } => {
            format!(
                "That id is already used at ({}, {}).",
                existing_slot.x, existing_slot.y
            )
        }
    }
}

pub fn place_tile(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    target: PlacementTarget,
    tile: TileSpawnKind,
) -> EditorTransactionResult {
    let (surface, address) = match target {
        PlacementTarget::BoardSlot { surface, slot } => {
            if surface == document.root_surface {
                (surface, PlacementAddress::BoardSlot(slot))
            } else {
                (
                    surface,
                    PlacementAddress::StackIndex(StackIndex(slot.x as usize)),
                )
            }
        }
        PlacementTarget::StackIndex { surface, index } => {
            (surface, PlacementAddress::StackIndex(index))
        }
    };
    if !document.surfaces.contains(surface) {
        return EditorTransactionResult::rejected("Cannot place tile on missing authored surface.");
    }
    if attention.workspace_mode != WorkspaceMode::Compose {
        return EditorTransactionResult::rejected("Tiles can only be placed in compose mode.");
    }
    if attention.active_board() != surface {
        return EditorTransactionResult::rejected(
            "Cannot place tile on a board surface that is not currently active.",
        );
    }

    let Some(surface_kind) = document.surfaces.kind(surface) else {
        return EditorTransactionResult::rejected("Cannot determine target board surface kind.");
    };
    if !tile_allowed_on_surface(&tile, &surface_kind) {
        return EditorTransactionResult::rejected(
            "This tile type is not allowed on the target board surface.",
        );
    }

    let auto_connect_partner = root_auto_connect_partner(document, attention, selection);
    let node = match address {
        PlacementAddress::BoardSlot(slot) if surface == document.root_surface => {
            match place_root_tile_on_board(board, document, slot, tile) {
                Ok(node) => node,
                Err(error) => {
                    return EditorTransactionResult::rejected(map_board_error(error));
                }
            }
        }
        PlacementAddress::StackIndex(index) => {
            match place_stack_tile_on_board(board, document, surface, index, tile) {
                Ok(node) => node,
                Err(error) => {
                    return EditorTransactionResult::rejected(map_board_error(error));
                }
            }
        }
        _ => {
            return EditorTransactionResult::rejected(
                "Only root board slots are supported for this placement.",
            );
        }
    };

    if matches!(address, PlacementAddress::BoardSlot(_)) && surface == document.root_surface {
        if let Some(partner) = auto_connect_partner.filter(|partner| partner != &node) {
            if let Ok(edge) =
                authorize_connection(&document.tessera.authored_program, &partner, &node)
            {
                apply_authorized_connection(document, board, &partner, &node, edge)
                    .expect("authorized root-board connection must bind on the live board");
            } else if let Ok(edge) =
                authorize_connection(&document.tessera.authored_program, &node, &partner)
            {
                apply_authorized_connection(document, board, &node, &partner, edge)
                    .expect("authorized root-board connection must bind on the live board");
            }
        }
    }

    selection.clear();
    selection.select(node.clone(), SelectionMode::Replace);
    if let PlacementAddress::StackIndex(index) = address {
        if document
            .surfaces
            .kind(surface)
            .is_some_and(|k| k != BoardSurfaceKind::RootBoard)
        {
            let next = StackIndex(index.0 + 1);
            set_focus(
                attention,
                document,
                FocusTarget::StackInsert {
                    surface,
                    index: next,
                },
            );
        } else {
            set_focus(attention, document, focus_for_source_node(document, &node));
        }
    } else {
        set_focus(attention, document, focus_for_source_node(document, &node));
    }
    EditorTransactionResult::accepted(Invalidation::document_changed())
        .with_undo(EditorInverse::DeleteNode { node })
}

fn place_root_tile_on_board(
    board: &mut TesseraBoard,
    document: &mut MusaicDocument,
    slot: BoardSlot,
    tile: TileSpawnKind,
) -> Result<NodeId, BoardError> {
    let node_id = board
        .tile_at(slot)
        .map(|existing| existing.id)
        .unwrap_or_else(|| document.graph.allocate_node_id());
    let name = node_id.0.clone();
    let footprint = root_board_tile_footprint(&tile);
    let slot_builder = board
        .replace_at(slot.x, slot.y)
        .named(name)
        .footprint(footprint);

    match tile {
        TileSpawnKind::Container { kind } => {
            let stack = empty_container_stack();
            match map_container_kind_for_document(kind) {
                tessera::prelude::ContainerKind::Sequence => {
                    let _ = slot_builder.sequence(stack)?;
                }
                tessera::prelude::ContainerKind::Alternate => {
                    let _ = slot_builder.alternate(stack)?;
                }
                tessera::prelude::ContainerKind::Layer => {
                    let _ = slot_builder.layer_container(stack)?;
                }
            }
        }
        TileSpawnKind::Output { .. } => {
            let _ = slot_builder.output()?;
        }
        TileSpawnKind::TrickInstance { prototype } => {
            let Some(kind) = transform_kind_from_prototype(prototype) else {
                return Err(BoardError::UnknownTile);
            };
            let _ = slot_builder.transform(kind)?;
        }
        _ => return Err(BoardError::UnknownTile),
    }

    sync_root_slot_to_document(document, board, slot)?;
    Ok(node_id)
}

fn place_stack_tile_on_board(
    board: &mut TesseraBoard,
    document: &mut MusaicDocument,
    surface: BoardSurfaceId,
    index: StackIndex,
    tile: TileSpawnKind,
) -> Result<NodeId, BoardError> {
    let container_node = document
        .graph
        .container_node_for_surface(surface)
        .ok_or(BoardError::UnknownTile)?;
    let container = document
        .graph
        .node(&container_node)
        .ok_or(BoardError::UnknownTile)?
        .clone();
    let mut nested = BTreeMap::new();
    let (stack, placed_id, stack_nodes) = export_container_stack_with_insert(
        &mut document.graph,
        &container,
        surface,
        index,
        &tile,
        &mut nested,
    )?;
    let mut handle = board
        .handle(&container_node)
        .ok_or(BoardError::UnknownTile)?;
    handle.set_sequence(stack)?;
    reconcile_stack_nodes_after_projection(document, surface, &stack_nodes);
    document.sync_tile_store_from_graph();
    document.bump_revision();
    Ok(placed_id)
}

fn reconcile_stack_nodes_after_projection(
    document: &mut MusaicDocument,
    surface: BoardSurfaceId,
    stack_nodes: &[(StackIndex, DocumentNode)],
) {
    let desired_ids = stack_nodes
        .iter()
        .map(|(_, node)| node.id.clone())
        .collect::<std::collections::BTreeSet<_>>();

    let existing = document
        .graph
        .nodes_on_surface(surface)
        .into_iter()
        .map(|(location, node)| match location.address {
            PlacementAddress::StackIndex(_) | PlacementAddress::BoardSlot(_) => node.id.clone(),
        })
        .collect::<Vec<_>>();

    let mut deleted = crate::domain::document::graph::DeletedSubtree::default();
    for node_id in existing {
        if desired_ids.contains(&node_id) {
            continue;
        }
        if let Ok(subtree) = document
            .graph
            .delete_subtree(&node_id, &mut document.surfaces)
        {
            deleted.nodes.extend(subtree.nodes);
            deleted.surfaces.extend(subtree.surfaces);
            deleted.edges.extend(subtree.edges);
        }
    }

    let mut inserted = Vec::new();
    for (index, node) in stack_nodes {
        if document
            .graph
            .node_at_stack_index(surface, *index)
            .is_some()
        {
            continue;
        }
        let Some(spawn) = stack_node_to_spawn(node) else {
            continue;
        };
        if document
            .graph
            .insert_tile_at_id(
                &mut document.surfaces,
                surface,
                PlacementAddress::StackIndex(*index),
                node.id.clone(),
                spawn,
            )
            .is_ok()
        {
            inserted.push(node.id.clone());
        }
    }

    document
        .tiles
        .apply_patch(&document.graph, &inserted, &deleted);
    document.sync_tile_store_from_graph();
}

fn stack_node_to_spawn(node: &DocumentNode) -> Option<TileSpawnKind> {
    match &node.kind {
        DocumentNodeKind::Atom(atom) => Some(TileSpawnKind::Atom {
            atom: atom.atom.clone(),
        }),
        DocumentNodeKind::Container(container) => Some(TileSpawnKind::Container {
            kind: container.kind,
        }),
        _ => None,
    }
}

fn tile_allowed_on_surface(tile: &TileSpawnKind, surface_kind: &BoardSurfaceKind) -> bool {
    match surface_kind {
        BoardSurfaceKind::RootBoard => match tile {
            TileSpawnKind::Atom { .. } => false,
            TileSpawnKind::Container { .. } => true,
            TileSpawnKind::Tile { .. } => true,
            TileSpawnKind::Output { .. } => true,
            TileSpawnKind::TrickInstance { .. } => true,
        },
        BoardSurfaceKind::ContainerStack { .. } => match tile {
            TileSpawnKind::Atom { .. } => true,
            TileSpawnKind::Container { .. } => true,
            TileSpawnKind::Tile { .. } => false,
            TileSpawnKind::Output { .. } => false,
            TileSpawnKind::TrickInstance { .. } => true,
        },
    }
}

pub fn enter_container(
    document: &mut MusaicDocument,
    attention: &mut EditorAttention,
    _selection: &mut SelectionState,
    container: &NodeId,
) -> EditorTransactionResult {
    let Some(node) = document.graph.node(container) else {
        return EditorTransactionResult::rejected("Cannot enter missing container.");
    };
    let DocumentNodeKind::Container(container_node) = &node.kind else {
        return EditorTransactionResult::rejected("Focused node is not a container.");
    };
    let local_surface = container_node.local_surface;
    if !document.surfaces.contains(local_surface) {
        return EditorTransactionResult::rejected("Container is missing its local board surface.");
    }
    let surface_change = match attention.set_active_board(
        &document.surfaces,
        local_surface,
        ActiveSurfaceChangeReason::EnterContainer {
            container_id: container.clone(),
        },
    ) {
        Ok(change) => change,
        Err(error) => {
            return EditorTransactionResult::rejected(format!(
                "Could not enter container surface: {error:?}"
            ));
        }
    };
    let mut result = EditorTransactionResult::accepted(Invalidation::editor_only());
    result.active_surface_change = surface_change;
    result
}

/// Removes a single node (inverse of place). Does not record undo metadata.
pub fn delete_node(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    node: &NodeId,
) -> EditorTransactionResult {
    if !selection.contains(node.clone()) {
        selection.clear();
        selection.select(node.clone(), SelectionMode::Replace);
    }
    delete_nodes(
        document,
        board,
        attention,
        selection,
        &[node.clone()],
        false,
    )
}

/// Removes an authored connection (inverse of connect).
pub fn disconnect_tiles(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    from: &NodeId,
    to: &NodeId,
) -> EditorTransactionResult {
    let program = &document.tessera.authored_program;
    let Some(connection) =
        crate::domain::document::connections_from_program(program, &document.port_endpoints)
            .into_iter()
            .find(|c| &c.from == from && &c.to == to)
    else {
        return EditorTransactionResult::rejected("Connection does not exist.");
    };
    let side = connection.spatial_side;
    if unbind_output_side_on_board(board, from, side).is_err() {
        return EditorTransactionResult::rejected("Connection does not exist.");
    }
    document
        .port_endpoints
        .set_side(from, side, crate::domain::document::PortSlotState::None);
    document.port_endpoints.set_side(
        to,
        opposite_spatial_side(side),
        crate::domain::document::PortSlotState::None,
    );
    sync_document_tessera_from_board(document, board);
    document.bump_revision();
    EditorTransactionResult::accepted(Invalidation::connections_changed())
}

pub fn delete_selection(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
) -> EditorTransactionResult {
    if selection.nodes.is_empty() {
        return EditorTransactionResult::rejected("There is no selection to delete.");
    }

    let selected = selection.nodes.iter().cloned().collect::<Vec<_>>();
    delete_nodes(document, board, attention, selection, &selected, true)
}

fn delete_nodes(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    selected: &[NodeId],
    capture_undo: bool,
) -> EditorTransactionResult {
    let restore_patch = if capture_undo {
        capture_subtree_patch(document, selected)
    } else {
        Default::default()
    };
    for node in selected {
        if !document.graph.contains_node(node) {
            return EditorTransactionResult::rejected(
                "Selection contains a node that no longer exists.",
            );
        }
    }

    let active_board_before = attention.active_board();
    let mut deleted_any = false;
    let mut root_slots_to_sync = Vec::new();
    let mut stack_surfaces_to_reconcile = Vec::new();
    for node in selected.iter() {
        let Some(location) = document.graph.location_of(&node) else {
            continue;
        };
        match location.address {
            PlacementAddress::BoardSlot(slot) if location.surface == document.root_surface => {
                if board.remove_at(slot).is_none() {
                    return EditorTransactionResult::rejected(map_board_error(
                        BoardError::UnknownTile,
                    ));
                }
                root_slots_to_sync.push(slot);
                deleted_any = true;
            }
            PlacementAddress::StackIndex(_) | PlacementAddress::BoardSlot(_) => {
                let Some(container_node) =
                    document.graph.container_node_for_surface(location.surface)
                else {
                    return EditorTransactionResult::rejected(map_board_error(
                        BoardError::UnknownTile,
                    ));
                };
                let container = match document.graph.node(&container_node) {
                    Some(node) => node,
                    None => {
                        return EditorTransactionResult::rejected(map_board_error(
                            BoardError::UnknownTile,
                        ));
                    }
                };
                let mut nested = BTreeMap::new();
                let stack = export_container_stack_excluding(
                    &document.graph,
                    container,
                    location.surface,
                    &node,
                    &mut nested,
                );
                let mut handle = match board.handle(&container_node) {
                    Some(handle) => handle,
                    None => {
                        return EditorTransactionResult::rejected(map_board_error(
                            BoardError::UnknownTile,
                        ));
                    }
                };
                if let Err(error) = handle.set_sequence(stack) {
                    return EditorTransactionResult::rejected(map_board_error(error));
                }
                let stack_nodes = stack_nodes_on_surface(&document.graph, location.surface)
                    .into_iter()
                    .filter(|(_, stack_node)| stack_node.id != *node)
                    .collect::<Vec<_>>();
                stack_surfaces_to_reconcile.push((location.surface, stack_nodes));
                deleted_any = true;
            }
        }
    }
    if !deleted_any {
        return EditorTransactionResult::rejected("No selected nodes were deleted.");
    }

    for slot in root_slots_to_sync {
        if let Err(error) = sync_root_slot_to_document(document, board, slot) {
            return EditorTransactionResult::rejected(map_board_error(error));
        }
    }
    for (surface, stack_nodes) in stack_surfaces_to_reconcile {
        reconcile_stack_nodes_after_projection(document, surface, &stack_nodes);
    }

    let remaining = |node: &NodeId| document.graph.contains_node(node);
    document.port_endpoints.retain_nodes(remaining);
    selection.clear();
    if !document.surfaces.contains(active_board_before) {
        let surface_change = match attention.set_active_board(
            &document.surfaces,
            document.root_surface,
            ActiveSurfaceChangeReason::DeleteFallbackToRoot,
        ) {
            Ok(change) => change,
            Err(error) => {
                return EditorTransactionResult::rejected(format!(
                    "Could not restore root surface after delete: {error:?}"
                ));
            }
        };
        let mut result = finish_delete_result(
            Invalidation::document_changed(),
            capture_undo,
            restore_patch,
        );
        result.active_surface_change = surface_change;
        set_focus(attention, document, FocusTarget::None);
        return result;
    }
    set_focus(attention, document, FocusTarget::None);
    finish_delete_result(
        Invalidation::document_changed(),
        capture_undo,
        restore_patch,
    )
}

fn finish_delete_result(
    invalidation: Invalidation,
    capture_undo: bool,
    restore_patch: crate::domain::document::DocumentPatch,
) -> EditorTransactionResult {
    let result = EditorTransactionResult::accepted(invalidation);
    if capture_undo {
        result.with_undo(EditorInverse::RestoreSubtree {
            patch: restore_patch,
        })
    } else {
        result
    }
}

pub fn bind_output_side(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    _attention: &EditorAttention,
    node: &NodeId,
    side: SpatialSide,
) -> EditorTransactionResult {
    if !document.graph.contains_node(node) {
        return EditorTransactionResult::rejected("Cannot bind port on missing tile.");
    }

    let previous = document.port_endpoints.side_state(node, side);
    let next = cycle_port_state(previous);
    let removed_binding = if next.disconnects() {
        unbind_output_side_on_board(board, node, side)
            .ok()
            .flatten()
            .map(|to| RemovedBoardBinding {
                from: node.clone(),
                to,
                side,
            })
    } else {
        None
    };

    document.port_endpoints.set_side(node, side, next);
    sync_document_tessera_from_board(document, board);
    document.bump_revision();

    EditorTransactionResult::accepted(Invalidation::connections_changed()).with_undo(
        EditorInverse::RestorePortBinding {
            node: node.clone(),
            side,
            port_state: previous,
            removed_binding,
        },
    )
}

pub fn restore_port_binding(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    node: &NodeId,
    side: SpatialSide,
    port_state: crate::domain::document::PortSlotState,
    removed_binding: Option<RemovedBoardBinding>,
) -> EditorTransactionResult {
    document.port_endpoints.set_side(node, side, port_state);
    if let Some(binding) = removed_binding {
        let _ = bind_tiles_on_board(board, &binding.from, &binding.to, binding.side);
    }
    sync_document_tessera_from_board(document, board);
    document.bump_revision();
    EditorTransactionResult::accepted(Invalidation::connections_changed())
}

pub fn cycle_connection(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    attention: &EditorAttention,
    from: &NodeId,
    to: &NodeId,
) -> EditorTransactionResult {
    let Some(from_location) = document.graph.location_of(from) else {
        return EditorTransactionResult::rejected("Connection source is missing placement.");
    };
    let Some(to_location) = document.graph.location_of(to) else {
        return EditorTransactionResult::rejected("Connection target is missing placement.");
    };
    let (PlacementAddress::BoardSlot(from_slot), PlacementAddress::BoardSlot(to_slot)) =
        (from_location.address, to_location.address)
    else {
        return EditorTransactionResult::rejected(
            "Connections are currently supported on board surfaces only.",
        );
    };
    let side = spatial_side_between(from_slot, to_slot);
    bind_output_side(document, board, attention, from, side)
}

pub fn connect_tiles(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    from: NodeId,
    to: NodeId,
) -> EditorTransactionResult {
    if attention.workspace_mode != WorkspaceMode::Compose {
        return EditorTransactionResult::rejected(
            "Connections can only be edited in compose mode.",
        );
    }
    if !document.graph.contains_node(&from) || !document.graph.contains_node(&to) {
        return EditorTransactionResult::rejected("Cannot connect missing tiles.");
    }

    let Some(from_location) = document.graph.location_of(&from) else {
        return EditorTransactionResult::rejected("Connection source is missing placement.");
    };
    let Some(to_location) = document.graph.location_of(&to) else {
        return EditorTransactionResult::rejected("Connection target is missing placement.");
    };
    if from_location.surface != to_location.surface {
        return EditorTransactionResult::rejected(
            "Connections currently require both tiles to share a surface.",
        );
    }
    if from_location.surface != attention.active_board() {
        return EditorTransactionResult::rejected(
            "Connections can only be authored on the active surface.",
        );
    }
    if connection_exists(
        &document.tessera.authored_program,
        &document.port_endpoints,
        &from,
        &to,
    ) {
        return EditorTransactionResult::rejected("That connection already exists.".to_string());
    }
    let edge = match authorize_connection(&document.tessera.authored_program, &from, &to) {
        Ok(edge) => edge,
        Err(error) => return EditorTransactionResult::rejected(error.to_string()),
    };
    if apply_authorized_connection(document, board, &from, &to, edge).is_err() {
        return EditorTransactionResult::rejected("Could not bind tiles on the board.");
    }
    document.bump_revision();
    selection.clear();
    selection.select(to.clone(), SelectionMode::Replace);
    set_focus(attention, document, focus_for_source_node(document, &to));
    EditorTransactionResult::accepted(Invalidation::connections_changed()).with_undo(
        EditorInverse::DisconnectTiles {
            from: from.clone(),
            to: to.clone(),
        },
    )
}

fn root_auto_connect_partner(
    document: &MusaicDocument,
    attention: &EditorAttention,
    selection: &SelectionState,
) -> Option<NodeId> {
    let focused = match &attention.focus {
        FocusTarget::Tile { node } => Some(node.clone()),
        _ => None,
    };
    focused
        .or_else(|| {
            selection
                .anchor
                .clone()
                .or_else(|| selection.nodes.iter().next().cloned())
        })
        .filter(|node| {
            document
                .graph
                .location_of(node)
                .is_some_and(|location| location.surface == document.root_surface)
        })
}

fn apply_authorized_connection(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    from: &NodeId,
    to: &NodeId,
    edge: AuthoredEdge,
) -> Result<(), BoardError> {
    bind_tiles_on_board(board, from, to, edge.side)?;
    document.port_endpoints.set_side(from, edge.side, edge.kind);
    document
        .port_endpoints
        .set_side(to, edge.side.opposite(), edge.kind);
    sync_document_tessera_from_board(document, board);
    Ok(())
}

pub fn navigate_to_surface(
    document: &mut MusaicDocument,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    surface: BoardSurfaceId,
) -> EditorTransactionResult {
    let surface_change = match attention.set_active_board(
        &document.surfaces,
        surface,
        ActiveSurfaceChangeReason::BreadcrumbJump,
    ) {
        Ok(change) => change,
        Err(error) => {
            return EditorTransactionResult::rejected(format!(
                "Could not navigate to board surface: {error:?}"
            ));
        }
    };

    attention.enter_compose();
    selection.clear();

    let mut result = EditorTransactionResult::accepted(Invalidation::editor_only());
    result.active_surface_change = surface_change;
    result
}

pub fn jump_to_timeline_source<R>(
    document: &mut MusaicDocument,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    resolver: &R,
    event: ProjectedEventId,
) -> EditorTransactionResult
where
    R: TimelineSourceResolver,
{
    let Some(source) = resolver.source_for_event(event) else {
        return EditorTransactionResult::rejected("Timeline event has no source provenance yet.");
    };
    if !document.surfaces.contains(source.surface) {
        return EditorTransactionResult::rejected(
            "Timeline event points to a missing source board surface.",
        );
    }
    if !document.graph.contains_node(&source.node) {
        return EditorTransactionResult::rejected(
            "Timeline event points to a missing source node.",
        );
    }

    attention.enter_compose();
    let surface_change = match attention.set_active_board(
        &document.surfaces,
        source.surface,
        ActiveSurfaceChangeReason::TimelineJump,
    ) {
        Ok(change) => change,
        Err(error) => {
            return EditorTransactionResult::rejected(format!(
                "Could not navigate to source surface: {error:?}"
            ));
        }
    };
    set_focus(
        attention,
        document,
        focus_for_source_node(document, &source.node),
    );
    selection.clear();
    selection.select(source.node.clone(), SelectionMode::Replace);
    let mut result = EditorTransactionResult::accepted(Invalidation::editor_only());
    result.active_surface_change = surface_change;
    result
}

fn set_focus(attention: &mut EditorAttention, document: &MusaicDocument, target: FocusTarget) {
    let queries = DocumentQueries::new(document);
    if let Err(error) = attention.focus(&queries, target) {
        // Transactions only focus targets they just created or cleared;
        // a failure means the document/attention contract broke.
        panic!("illegal focus after transaction: {error}");
    }
}

fn focus_for_source_node(document: &MusaicDocument, node: &NodeId) -> FocusTarget {
    match document.graph.node(node).map(|node| &node.kind) {
        Some(DocumentNodeKind::Atom(_)) => FocusTarget::Atom { node: node.clone() },
        Some(_) => FocusTarget::Tile { node: node.clone() },
        None => FocusTarget::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::command::EditorCommand;
    use crate::application::editor::ActiveSpace;
    use crate::application::editor::NavigationMode;
    use crate::application::pipeline::runtime::TimelineProvenanceStore;
    use crate::domain::board::BoardSlot;
    use crate::domain::document::{
        AtomValue, ContainerKind, DocumentQueries, MusaicDocument, NoteName, TileSpawnKind,
    };
    use tessera::bevy::TesseraBoard;

    fn slot(col: i32, row: i32) -> BoardSlot {
        BoardSlot::new(col, row)
    }

    fn connection_count(document: &MusaicDocument) -> usize {
        DocumentQueries::new(document)
            .connections_on_surface(document.root_surface)
            .len()
    }

    fn fresh_board() -> TesseraBoard {
        TesseraBoard::new()
    }

    fn board_target(surface: BoardSurfaceId, slot: BoardSlot) -> PlacementTarget {
        PlacementTarget::BoardSlot { surface, slot }
    }

    /// Applies a command through the real execution path used by the dispatcher.
    fn apply(
        document: &mut MusaicDocument,
        board: &mut TesseraBoard,
        attention: &mut EditorAttention,
        selection: &mut SelectionState,
        provenance: &TimelineProvenanceStore,
        command: EditorCommand,
    ) -> EditorTransactionResult {
        crate::application::command::execute_command(
            document, board, attention, selection, provenance, &command,
        )
        .unwrap()
    }

    #[test]
    fn place_container_on_root_creates_node_and_focuses_it() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();

        let root_surface = document.root_surface;
        let mut board = fresh_board();
        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        );

        assert!(result.is_accepted());
        assert!(result.invalidation.document);
        assert!(result.invalidation.compile);
        assert_eq!(document.revision.0, 1);
        assert_eq!(selection.nodes.len(), 1);
        let selected = selection.nodes.iter().next().unwrap().clone();
        assert_eq!(
            attention.focus,
            FocusTarget::Tile {
                node: selected.clone()
            }
        );
        assert!(document.graph.container_surface(&selected).is_some());
        assert_eq!(
            document
                .tessera
                .authored_program
                .root_surface
                .placements
                .get(&selected)
                .expect("placed container has an authored Tessera placement")
                .footprint,
            tessera::prelude::TileFootprint::new(2, 2)
        );
    }

    #[test]
    fn atom_cannot_be_placed_on_root_surface() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();

        let root_surface = document.root_surface;
        let mut board = fresh_board();
        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(NoteName::A),
                },
            },
        );

        assert!(!result.is_accepted());
        assert_eq!(document.revision.0, 0);
        assert!(selection.nodes.is_empty());
    }

    #[test]
    fn enter_container_changes_active_board_to_container_surface() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();

        let root_surface = document.root_surface;
        let mut board = fresh_board();
        let place_result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Subdivision,
                },
            },
        );
        assert!(place_result.is_accepted());

        let container = selection.nodes.iter().next().unwrap().clone();
        let container_surface = document.graph.container_surface(&container).unwrap();
        let enter_result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::EnterContainer { container },
        );

        assert!(enter_result.is_accepted());
        assert_eq!(
            attention.active_space,
            ActiveSpace::Board(container_surface)
        );
        assert_eq!(attention.focus, FocusTarget::None);
        assert_eq!(document.revision.0, 1);
    }

    #[test]
    fn place_atom_inside_container_surface_is_allowed() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();

        let root_surface = document.root_surface;
        let mut board = fresh_board();
        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        );
        let container = selection.nodes.iter().next().unwrap().clone();
        let container_surface = document.graph.container_surface(&container).unwrap();
        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::EnterContainer { container },
        );

        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(container_surface, slot(1, 0)),
                tile: TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(NoteName::C),
                },
            },
        );

        assert!(result.is_accepted());
        assert_eq!(document.revision.0, 2);
        assert_eq!(
            attention.focus,
            FocusTarget::StackInsert {
                surface: container_surface,
                index: StackIndex(2),
            }
        );
    }

    #[test]
    fn place_note_then_octave_in_container_keeps_both_stack_nodes() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let mut board = fresh_board();
        let root_surface = document.root_surface;

        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        );
        let container = selection.nodes.iter().next().unwrap().clone();
        let container_surface = document.graph.container_surface(&container).unwrap();
        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::EnterContainer { container },
        );

        let note_place = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: PlacementTarget::StackIndex {
                    surface: container_surface,
                    index: StackIndex(0),
                },
                tile: TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(NoteName::A),
                },
            },
        );
        assert!(
            note_place.is_accepted(),
            "note place failed: {:?}",
            note_place.diagnostics
        );

        let octave_place = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: PlacementTarget::StackIndex {
                    surface: container_surface,
                    index: StackIndex(1),
                },
                tile: TileSpawnKind::Atom {
                    atom: AtomValue::Octave(2),
                },
            },
        );
        assert!(
            octave_place.is_accepted(),
            "octave place failed: {:?}",
            octave_place.diagnostics
        );

        let stack_atoms = document
            .graph
            .nodes_on_surface(container_surface)
            .into_iter()
            .filter(|(location, _)| matches!(location.address, PlacementAddress::StackIndex(_)))
            .count();
        assert_eq!(stack_atoms, 2);
    }

    #[test]
    fn delete_selection_removes_selected_node_and_clears_focus() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();

        let root_surface = document.root_surface;
        let mut board = fresh_board();
        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        );
        let container = selection.nodes.iter().next().unwrap().clone();

        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::DeleteSelection,
        );

        assert!(result.is_accepted());
        assert!(!document.graph.contains_node(&container));
        assert!(selection.nodes.is_empty());
        assert_eq!(attention.focus, FocusTarget::None);
        assert_eq!(document.revision.0, 2);
    }

    #[test]
    fn connect_tiles_on_same_board_creates_authored_connection() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;

        let mut board = fresh_board();
        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        );
        let from = selection.nodes.iter().next().unwrap().clone();
        selection.clear();
        set_focus(&mut attention, &document, FocusTarget::None);

        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(2, 0)),
                tile: TileSpawnKind::Output {
                    name: "main".into(),
                },
            },
        );
        let to = selection.nodes.iter().next().unwrap().clone();

        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::ConnectTiles {
                from: from.clone(),
                to: to.clone(),
            },
        );

        assert!(result.is_accepted());
        assert_eq!(connection_count(&document), 1);
        let connections = DocumentQueries::new(&document).connections_on_surface(root_surface);
        let connection = connections.first().unwrap();
        assert_eq!(connection.from, from);
        assert_eq!(connection.to, to.clone());
        assert!(selection.contains(to));
    }

    #[test]
    fn connect_tiles_rejects_distant_root_tiles() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;
        let mut board = fresh_board();

        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        );
        let from = selection.nodes.iter().next().unwrap().clone();
        selection.clear();
        set_focus(&mut attention, &document, FocusTarget::None);

        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(3, 0)),
                tile: TileSpawnKind::Output {
                    name: "main".into(),
                },
            },
        );
        let to = selection.nodes.iter().next().unwrap().clone();

        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::ConnectTiles { from, to },
        );

        assert!(!result.is_accepted());
        assert_eq!(connection_count(&document), 0);
        assert_eq!(result.diagnostics[0].message, "tiles must be edge-adjacent");
    }

    #[test]
    fn placing_adjacent_to_focused_root_tile_auto_connects() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;
        let mut board = fresh_board();

        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        );
        let container = selection.nodes.iter().next().unwrap().clone();

        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(2, 0)),
                tile: TileSpawnKind::Output {
                    name: "main".into(),
                },
            },
        );
        let output = selection.nodes.iter().next().unwrap().clone();

        assert!(result.is_accepted());
        assert_eq!(connection_count(&document), 1);
        assert!(connection_exists(
            &document.tessera.authored_program,
            &document.port_endpoints,
            &container,
            &output,
        ));
    }

    #[test]
    fn delete_selection_prunes_connections_to_deleted_nodes() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;

        let mut board = fresh_board();
        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        );
        let from = selection.nodes.iter().next().unwrap().clone();

        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(2, 0)),
                tile: TileSpawnKind::Output {
                    name: "main".into(),
                },
            },
        );
        let to = selection.nodes.iter().next().unwrap().clone();

        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::ConnectTiles {
                from,
                to: to.clone(),
            },
        );
        assert_eq!(connection_count(&document), 1);

        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::DeleteSelection,
        );

        assert!(result.is_accepted());
        assert_eq!(connection_count(&document), 0);
    }

    #[test]
    fn jump_to_timeline_source_moves_to_source_surface_and_focuses_node() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let mut provenance = TimelineProvenanceStore::default();

        let root_surface = document.root_surface;
        let mut board = fresh_board();
        let _ = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::PlaceTile {
                target: board_target(root_surface, slot(0, 0)),
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        );

        let source_node = selection.nodes.iter().next().unwrap().clone();
        let event = ProjectedEventId(44);
        provenance.insert_source(event, root_surface, source_node.clone());
        attention.enter_navigation(NavigationMode::Timeline);
        set_focus(
            &mut attention,
            &document,
            FocusTarget::TimelineEvent { event },
        );

        let mut board = fresh_board();
        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::JumpToTimelineSource { event },
        );

        assert!(result.is_accepted());
        assert_eq!(attention.workspace_mode, WorkspaceMode::Compose);
        assert_eq!(attention.active_space, ActiveSpace::Board(root_surface));
        assert_eq!(
            attention.focus,
            FocusTarget::Tile {
                node: source_node.clone()
            }
        );
        assert!(selection.contains(source_node));
    }

    #[test]
    fn jump_to_timeline_source_without_provenance_is_rejected() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();

        let root_surface = document.root_surface;
        let mut board = fresh_board();
        let result = apply(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            EditorCommand::JumpToTimelineSource {
                event: ProjectedEventId(99),
            },
        );

        assert!(!result.is_accepted());
        assert_eq!(attention.active_space, ActiveSpace::Board(root_surface));
    }
}
