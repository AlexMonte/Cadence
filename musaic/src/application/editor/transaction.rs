pub mod drop;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tessera::prelude::{
    AuthoredTesseraProgram, NodeId, NodeSpatialBindings, RootRelation, SpatialSide,
};

use crate::application::command::EditorInverse;
use crate::application::editor::selection::{SelectionMode, SelectionState};
use crate::application::editor::workspace::{
    ActiveSurfaceChangeReason, ActiveSurfaceChanged, EditorAttention, FocusTarget, WorkspaceMode,
};
use crate::application::pipeline::runtime::ProjectedEventId;
use crate::domain::board::{BoardSlot, BoardSurfaceId, BoardSurfaceKind};
use crate::domain::document::{
    AuthoredEdge, DocumentNodeKind, DocumentQueries, MusaicDocument, PlacementAddress, StackIndex,
    TileSpawnKind, bind_authorized_edge, capture_subtree_patch, connection_exists,
    export_document_program, unbind_connection,
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
        }
    }

    pub fn editor_only() -> Self {
        Self {
            scene: true,
            ..Self::default()
        }
    }

    pub fn document_replaced() -> Self {
        Self::document_changed()
    }

    pub fn connections_changed() -> Self {
        Self::document_changed()
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

fn commit_candidate(
    document: &mut MusaicDocument,
    mut candidate: MusaicDocument,
) -> Result<(), String> {
    candidate.validate()?;
    candidate.bump_revision();
    *document = candidate;
    Ok(())
}

fn export(document: &MusaicDocument) -> Result<AuthoredTesseraProgram, String> {
    export_document_program(document)
        .map_err(|error| format!("The document cannot be compiled: {error:?}"))
}

pub fn set_atom_value(
    document: &mut MusaicDocument,
    node: &NodeId,
    value: crate::domain::document::AtomValue,
) -> EditorTransactionResult {
    let mut candidate = document.clone();
    let previous = match candidate.graph.set_atom_value(node, value.clone()) {
        Ok(previous) => previous,
        Err(message) => return EditorTransactionResult::rejected(message),
    };
    if previous == value {
        return EditorTransactionResult::accepted(Invalidation::none());
    }
    if let Err(message) = commit_candidate(document, candidate) {
        return EditorTransactionResult::rejected(message);
    }
    EditorTransactionResult::accepted(Invalidation::document_changed()).with_undo(
        EditorInverse::RestoreAtomValue {
            node: node.clone(),
            value: previous,
        },
    )
}

pub fn move_modifier_group(
    document: &mut MusaicDocument,
    owner: &NodeId,
    step: i8,
) -> EditorTransactionResult {
    use crate::application::pipeline::scene_sync::surface_content::{
        OwnedTileGroupRole, owned_compound_for_node,
    };
    if step != -1 && step != 1 {
        return EditorTransactionResult::rejected("Move a group one position at a time.");
    }
    let Some(mut compound) = owned_compound_for_node(&DocumentQueries::new(document), owner) else {
        return EditorTransactionResult::rejected("Select a modifier group to move.");
    };
    let current = compound.selected_group;
    let Some(target) = current
        .checked_add_signed(isize::from(step))
        .filter(|index| *index < compound.groups.len())
    else {
        return EditorTransactionResult::rejected(
            "That group is already at the edge of this note.",
        );
    };
    let movable = |role| {
        matches!(
            role,
            OwnedTileGroupRole::Octave
                | OwnedTileGroupRole::Accidental
                | OwnedTileGroupRole::Modifier(_)
                | OwnedTileGroupRole::SoundModifier(_)
                | OwnedTileGroupRole::RhythmModifier
        )
    };
    if !movable(compound.groups[current].role)
        || !movable(compound.groups[target].role)
        || !compound.groups[current].complete
        || !compound.groups[target].complete
    {
        return EditorTransactionResult::rejected(
            "Complete modifier groups move together within their note.",
        );
    }
    compound.groups.swap(current, target);
    let order = compound
        .groups
        .iter()
        .flat_map(|group| group.members.clone())
        .collect::<Vec<_>>();
    restore_stack_order(document, compound.surface, &order)
}

pub fn restore_stack_order(
    document: &mut MusaicDocument,
    surface: BoardSurfaceId,
    order: &[NodeId],
) -> EditorTransactionResult {
    let mut candidate = document.clone();
    let previous = match candidate.graph.reorder_stack_nodes(surface, order) {
        Ok(previous) => previous,
        Err(message) => return EditorTransactionResult::rejected(message),
    };
    if let Err(message) = commit_candidate(document, candidate) {
        return EditorTransactionResult::rejected(message);
    }
    EditorTransactionResult::accepted(Invalidation::document_changed()).with_undo(
        EditorInverse::RestoreStackOrder {
            surface,
            order: previous,
        },
    )
}

pub fn place_tile(
    document: &mut MusaicDocument,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    target: PlacementTarget,
    tile: TileSpawnKind,
) -> EditorTransactionResult {
    if matches!(tile, TileSpawnKind::Output { .. })
        && document
            .graph
            .nodes()
            .filter(|node| matches!(node.kind, DocumentNodeKind::Output(_)))
            .count()
            >= usize::from(document.channels)
    {
        return EditorTransactionResult::rejected(
            "The project channel limit has been reached. Remove an output or increase Channels.",
        );
    }

    let (surface, address) = match target {
        PlacementTarget::BoardSlot { surface, slot } if surface == document.root_surface => {
            (surface, PlacementAddress::BoardSlot(slot))
        }
        PlacementTarget::BoardSlot { surface, slot } => {
            let Ok(index) = usize::try_from(slot.x) else {
                return EditorTransactionResult::rejected("A stack position cannot be negative.");
            };
            (surface, PlacementAddress::StackIndex(StackIndex(index)))
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

    if let PlacementAddress::StackIndex(index) = address {
        if let Some(existing) = document.graph.node_at_stack_index(surface, index) {
            if drop::is_number(&tile) && drop::note_owner(document, &existing).is_some() {
                let mut candidate = document.clone();
                let owner = match drop::apply_number(&mut candidate, &existing, &tile) {
                    Ok(owner) => owner,
                    Err(error) => return EditorTransactionResult::rejected(error),
                };
                if let Err(message) = commit_candidate(document, candidate) {
                    return EditorTransactionResult::rejected(message);
                }
                selection.select(owner.clone(), SelectionMode::Replace);
                set_focus(attention, document, focus_for_source_node(document, &owner));
                return EditorTransactionResult::accepted(Invalidation::document_changed());
            }
        }
    }

    let previous_program = match export(document) {
        Ok(program) => program,
        Err(message) => return EditorTransactionResult::rejected(message),
    };
    let auto_connect_partner = root_auto_connect_partner(document, attention, selection);
    let mut candidate = document.clone();
    let node = match candidate
        .graph
        .insert_tile(&mut candidate.surfaces, surface, address, tile)
    {
        Ok(node) => node,
        Err(error) => {
            return EditorTransactionResult::rejected(format!(
                "Could not place the tile: {error:?}"
            ));
        }
    };

    let diagnostics =
        if matches!(address, PlacementAddress::BoardSlot(_)) && surface == candidate.root_surface {
            let mut current_program = match export(&candidate) {
                Ok(program) => program,
                Err(message) => return EditorTransactionResult::rejected(message),
            };
            let plan = match crate::application::editor::connection::plan_contextual_connections(
                &previous_program,
                &current_program,
                std::slice::from_ref(&node),
                auto_connect_partner.as_ref(),
            ) {
                Ok(plan) => plan,
                Err(message) => return EditorTransactionResult::rejected(message),
            };
            current_program.root_surface.bindings = plan.bindings;
            current_program.root_surface.explicit_relations = plan.explicit_relations;
            candidate.replace_connections_from(&current_program);
            plan.feedback
                .into_iter()
                .map(|message| EditorTransactionDiagnostic {
                    severity: DiagnosticSeverity::Info,
                    message,
                })
                .collect()
        } else {
            Vec::new()
        };

    if let Err(message) = commit_candidate(document, candidate) {
        return EditorTransactionResult::rejected(message);
    }
    selection.clear();
    selection.select(node.clone(), SelectionMode::Replace);
    if let PlacementAddress::StackIndex(index) = address {
        if document
            .surfaces
            .kind(surface)
            .is_some_and(|kind| kind != BoardSurfaceKind::RootBoard)
        {
            set_focus(
                attention,
                document,
                FocusTarget::StackInsert {
                    surface,
                    index: StackIndex(index.0 + 1),
                },
            );
        } else {
            set_focus(attention, document, focus_for_source_node(document, &node));
        }
    } else {
        set_focus(attention, document, focus_for_source_node(document, &node));
    }

    let mut result = EditorTransactionResult::accepted(Invalidation::document_changed())
        .with_undo(EditorInverse::DeleteNode { node });
    result.diagnostics = diagnostics;
    result
}

fn tile_allowed_on_surface(tile: &TileSpawnKind, surface_kind: &BoardSurfaceKind) -> bool {
    match surface_kind {
        BoardSurfaceKind::RootBoard => match tile {
            TileSpawnKind::Atom { atom } => atom.numeric_rational().is_some(),
            TileSpawnKind::Container { .. }
            | TileSpawnKind::Output { .. }
            | TileSpawnKind::Sound { .. }
            | TileSpawnKind::TrickInstance { .. }
            | TileSpawnKind::FlowControl { .. } => true,
            TileSpawnKind::Tile { .. } => false,
        },
        BoardSurfaceKind::ContainerStack { .. } => matches!(
            tile,
            TileSpawnKind::Atom { .. } | TileSpawnKind::Container { .. }
        ),
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

pub fn delete_node(
    document: &mut MusaicDocument,
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
        attention,
        selection,
        std::slice::from_ref(node),
        false,
    )
}

pub fn disconnect_tiles(
    document: &mut MusaicDocument,
    from: &NodeId,
    to: &NodeId,
) -> EditorTransactionResult {
    let mut program = match export(document) {
        Ok(program) => program,
        Err(message) => return EditorTransactionResult::rejected(message),
    };
    if !connection_exists(&program, from, to) {
        return EditorTransactionResult::rejected("Connection does not exist.");
    }
    if unbind_connection(&mut program, from, to).is_err() {
        return EditorTransactionResult::rejected("Connection does not exist.");
    }
    let mut candidate = document.clone();
    candidate.replace_connections_from(&program);
    if let Err(message) = commit_candidate(document, candidate) {
        return EditorTransactionResult::rejected(message);
    }
    EditorTransactionResult::accepted(Invalidation::connections_changed())
}

pub fn delete_selection(
    document: &mut MusaicDocument,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
) -> EditorTransactionResult {
    if selection.nodes.is_empty() {
        return EditorTransactionResult::rejected("There is no selection to delete.");
    }
    let selected = selection.nodes.iter().cloned().collect::<Vec<_>>();
    delete_nodes(document, attention, selection, &selected, true)
}

fn delete_nodes(
    document: &mut MusaicDocument,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    selected: &[NodeId],
    capture_undo: bool,
) -> EditorTransactionResult {
    for node in selected {
        if !document.graph.contains_node(node) {
            return EditorTransactionResult::rejected(
                "Selection contains a node that no longer exists.",
            );
        }
    }

    let deletion_patch = capture_subtree_patch(document, selected);
    let deleted_ids = deletion_patch
        .nodes
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let restore_patch = capture_undo.then_some(deletion_patch);
    let mut candidate = document.clone();
    let mut program = match export(&candidate) {
        Ok(program) => program,
        Err(message) => return EditorTransactionResult::rejected(message),
    };
    let disconnected_pairs =
        crate::domain::document::connection_policy::endpoint_connections(&program)
            .into_iter()
            .filter(|edge| deleted_ids.contains(&edge.from) || deleted_ids.contains(&edge.to))
            .map(|edge| (edge.from, edge.to))
            .collect::<BTreeSet<_>>();
    for (from, to) in disconnected_pairs {
        let _ = unbind_connection(&mut program, &from, &to);
    }
    candidate.replace_connections_from(&program);

    for node in selected {
        if !candidate.graph.contains_node(node) {
            continue;
        }
        let deleted = match candidate
            .graph
            .delete_subtree(node, &mut candidate.surfaces)
        {
            Ok(deleted) => deleted,
            Err(error) => {
                return EditorTransactionResult::rejected(format!(
                    "Could not delete the selected tile: {error:?}"
                ));
            }
        };
        for surface in deleted.surfaces {
            candidate.surfaces.remove_container_surface(surface);
        }
    }
    candidate
        .connections
        .bindings
        .retain(|node, _| candidate.graph.contains_node(node));
    candidate.connections.explicit_relations.retain(|relation| {
        let edge = crate::domain::document::connection_policy::explicit_connection(relation);
        candidate.graph.contains_node(&edge.from) && candidate.graph.contains_node(&edge.to)
    });

    if let Err(message) = commit_candidate(document, candidate) {
        return EditorTransactionResult::rejected(message);
    }
    let active_board_before = attention.active_board();
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
        let mut result = finish_delete_result(restore_patch);
        result.active_surface_change = surface_change;
        set_focus(attention, document, FocusTarget::None);
        return result;
    }
    set_focus(attention, document, FocusTarget::None);
    finish_delete_result(restore_patch)
}

fn finish_delete_result(
    restore_patch: Option<crate::domain::document::DocumentPatch>,
) -> EditorTransactionResult {
    let result = EditorTransactionResult::accepted(Invalidation::document_changed());
    match restore_patch {
        Some(patch) => result.with_undo(EditorInverse::RestoreSubtree { patch }),
        None => result,
    }
}

pub fn bind_output_side(
    document: &mut MusaicDocument,
    _attention: &EditorAttention,
    node: &NodeId,
    side: SpatialSide,
) -> EditorTransactionResult {
    if !document.graph.contains_node(node) {
        return EditorTransactionResult::rejected("Cannot bind port on missing tile.");
    }
    let previous_bindings = document.connections.bindings.clone();
    let previous_relations = document.connections.explicit_relations.clone();
    let current = match export(document) {
        Ok(program) => program,
        Err(message) => return EditorTransactionResult::rejected(message),
    };
    let program = match crate::domain::document::connection_policy::authorize_side_cycle(
        &current, node, side,
    ) {
        Ok(program) => program,
        Err(error) => return EditorTransactionResult::rejected(error.to_string()),
    };
    let mut candidate = document.clone();
    candidate.replace_connections_from(&program);
    if let Err(message) = commit_candidate(document, candidate) {
        return EditorTransactionResult::rejected(message);
    }
    EditorTransactionResult::accepted(Invalidation::connections_changed()).with_undo(
        connection_inverse(previous_bindings, previous_relations, document),
    )
}

fn connection_inverse(
    previous_bindings: BTreeMap<NodeId, NodeSpatialBindings>,
    previous_relations: Vec<RootRelation>,
    document: &MusaicDocument,
) -> EditorInverse {
    fn changed<T: Clone + PartialEq>(
        before: &BTreeMap<NodeId, T>,
        after: &BTreeMap<NodeId, T>,
    ) -> BTreeMap<NodeId, Option<T>> {
        before
            .keys()
            .chain(after.keys())
            .filter(|id| before.get(*id) != after.get(*id))
            .map(|id| (id.clone(), before.get(id).cloned()))
            .collect()
    }
    EditorInverse::RestoreConnections {
        relations: (previous_relations != document.connections.explicit_relations)
            .then_some(previous_relations),
        bindings: changed(&previous_bindings, &document.connections.bindings),
    }
}

pub fn restore_connections(
    document: &mut MusaicDocument,
    bindings: &BTreeMap<NodeId, Option<NodeSpatialBindings>>,
    relations: &Option<Vec<RootRelation>>,
) -> EditorTransactionResult {
    fn restore<T: Clone>(target: &mut BTreeMap<NodeId, T>, patch: &BTreeMap<NodeId, Option<T>>) {
        for (id, value) in patch {
            if let Some(value) = value {
                target.insert(id.clone(), value.clone());
            } else {
                target.remove(id);
            }
        }
    }

    let mut program = match export(document) {
        Ok(program) => program,
        Err(message) => return EditorTransactionResult::rejected(message),
    };
    restore(&mut program.root_surface.bindings, bindings);
    if let Some(relations) = relations {
        program
            .root_surface
            .explicit_relations
            .clone_from(relations);
    }
    let mut candidate = document.clone();
    candidate.replace_connections_from(&program);
    if let Err(message) = commit_candidate(document, candidate) {
        return EditorTransactionResult::rejected(message);
    }
    EditorTransactionResult::accepted(Invalidation::connections_changed())
}

pub fn cycle_connection(
    document: &mut MusaicDocument,
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
    let (PlacementAddress::BoardSlot(_), PlacementAddress::BoardSlot(_)) =
        (from_location.address, to_location.address)
    else {
        return EditorTransactionResult::rejected(
            "Connections are currently supported on board surfaces only.",
        );
    };
    if attention.workspace_mode != WorkspaceMode::Compose
        || from_location.surface != attention.active_board()
        || to_location.surface != attention.active_board()
    {
        return EditorTransactionResult::rejected(
            "Connections can only be edited on the active pattern board.",
        );
    }
    let previous_bindings = document.connections.bindings.clone();
    let previous_relations = document.connections.explicit_relations.clone();
    let mut program = match export(document) {
        Ok(program) => program,
        Err(message) => return EditorTransactionResult::rejected(message),
    };
    if unbind_connection(&mut program, from, to).is_err() {
        return EditorTransactionResult::rejected("That connection no longer exists.");
    }
    let mut candidate = document.clone();
    candidate.replace_connections_from(&program);
    if let Err(message) = commit_candidate(document, candidate) {
        return EditorTransactionResult::rejected(message);
    }
    EditorTransactionResult::accepted(Invalidation::connections_changed()).with_undo(
        connection_inverse(previous_bindings, previous_relations, document),
    )
}

pub fn connect_tiles(
    document: &mut MusaicDocument,
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

    let mut program = match export(document) {
        Ok(program) => program,
        Err(message) => return EditorTransactionResult::rejected(message),
    };
    if connection_exists(&program, &from, &to) {
        return EditorTransactionResult::rejected("That connection already exists.");
    }
    let edge = match crate::domain::document::connection_policy::authorize_manual_connection(
        &program, &from, &to,
    ) {
        Ok(edge) => edge,
        Err(error) => return EditorTransactionResult::rejected(error.to_string()),
    };
    let previous_bindings = document.connections.bindings.clone();
    let previous_relations = document.connections.explicit_relations.clone();
    if let Err(message) = apply_authorized_connection(&mut program, &from, &to, &edge) {
        return EditorTransactionResult::rejected(message);
    }
    let mut candidate = document.clone();
    candidate.replace_connections_from(&program);
    if let Err(message) = commit_candidate(document, candidate) {
        return EditorTransactionResult::rejected(message);
    }
    selection.clear();
    selection.select(to.clone(), SelectionMode::Replace);
    set_focus(attention, document, focus_for_source_node(document, &to));
    EditorTransactionResult::accepted(Invalidation::connections_changed()).with_undo(
        connection_inverse(previous_bindings, previous_relations, document),
    )
}

fn apply_authorized_connection(
    program: &mut AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
    edge: &AuthoredEdge,
) -> Result<(), String> {
    bind_authorized_edge(program, from, to, edge)
        .map_err(|_| "Could not bind tiles in the document.".to_string())
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
    use crate::application::editor::{ActiveSpace, NavigationMode};
    use crate::domain::document::{AtomValue, ContainerKind, NoteName};

    fn place_container(
        document: &mut MusaicDocument,
        attention: &mut EditorAttention,
        selection: &mut SelectionState,
        slot: BoardSlot,
    ) -> NodeId {
        let result = place_tile(
            document,
            attention,
            selection,
            PlacementTarget::BoardSlot {
                surface: document.root_surface,
                slot,
            },
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        );
        assert!(result.is_accepted(), "{:?}", result.diagnostics);
        selection.nodes.iter().next().unwrap().clone()
    }

    #[test]
    fn placement_commits_one_valid_document_revision() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let node = place_container(
            &mut document,
            &mut attention,
            &mut selection,
            BoardSlot::new(0, 0),
        );

        assert_eq!(document.revision.0, 1);
        assert!(document.validate().is_ok());
        assert!(document.graph.container_surface(&node).is_some());
        assert_eq!(attention.focus, FocusTarget::Tile { node });
    }

    #[test]
    fn rejected_placement_leaves_the_document_unchanged() {
        let mut document = MusaicDocument::new_empty();
        let before = document.clone();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let root = document.root_surface;
        let result = place_tile(
            &mut document,
            &mut attention,
            &mut selection,
            PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(0, 0),
            },
            TileSpawnKind::Atom {
                atom: AtomValue::NoteName(NoteName::A),
            },
        );

        assert!(!result.is_accepted());
        assert_eq!(document, before);
        assert!(selection.nodes.is_empty());
    }

    #[test]
    fn nested_placement_is_authored_directly_in_the_document() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let container = place_container(
            &mut document,
            &mut attention,
            &mut selection,
            BoardSlot::new(0, 0),
        );
        let surface = document.graph.container_surface(&container).unwrap();
        assert!(
            enter_container(&mut document, &mut attention, &mut selection, &container)
                .is_accepted()
        );

        let result = place_tile(
            &mut document,
            &mut attention,
            &mut selection,
            PlacementTarget::StackIndex {
                surface,
                index: StackIndex(0),
            },
            TileSpawnKind::Atom {
                atom: AtomValue::NoteName(NoteName::C),
            },
        );
        assert!(result.is_accepted(), "{:?}", result.diagnostics);
        assert_eq!(document.graph.nodes_on_surface(surface).len(), 1);
        assert_eq!(
            attention.focus,
            FocusTarget::StackInsert {
                surface,
                index: StackIndex(1)
            }
        );
    }

    #[test]
    fn connect_disconnect_and_reconnect_use_canonical_connections() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let from = place_container(
            &mut document,
            &mut attention,
            &mut selection,
            BoardSlot::new(0, 0),
        );
        let root = document.root_surface;
        let placed = place_tile(
            &mut document,
            &mut attention,
            &mut selection,
            PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(5, 0),
            },
            TileSpawnKind::Output {
                name: "main".into(),
            },
        );
        assert!(placed.is_accepted());
        let to = selection.nodes.iter().next().unwrap().clone();
        assert!(connection_exists(
            &export_document_program(&document).unwrap(),
            &from,
            &to
        ));

        assert!(disconnect_tiles(&mut document, &from, &to).is_accepted());
        assert!(!connection_exists(
            &export_document_program(&document).unwrap(),
            &from,
            &to
        ));
        assert!(
            connect_tiles(
                &mut document,
                &mut attention,
                &mut selection,
                from.clone(),
                to.clone(),
            )
            .is_accepted()
        );
        assert!(connection_exists(
            &export_document_program(&document).unwrap(),
            &from,
            &to
        ));
    }

    #[test]
    fn deletion_prunes_routes_to_removed_nodes() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let from = place_container(
            &mut document,
            &mut attention,
            &mut selection,
            BoardSlot::new(0, 0),
        );
        let root = document.root_surface;
        place_tile(
            &mut document,
            &mut attention,
            &mut selection,
            PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(5, 0),
            },
            TileSpawnKind::Output {
                name: "main".into(),
            },
        );
        let output = selection.nodes.iter().next().unwrap().clone();
        assert!(connection_exists(
            &export_document_program(&document).unwrap(),
            &from,
            &output
        ));

        assert!(delete_selection(&mut document, &mut attention, &mut selection).is_accepted());
        assert!(!document.graph.contains_node(&output));
        assert!(
            document
                .connections
                .bindings
                .keys()
                .all(|node| document.graph.contains_node(node))
        );
        assert!(document.validate().is_ok());
    }

    struct Resolver(Option<TimelineSource>);

    impl TimelineSourceResolver for Resolver {
        fn source_for_event(&self, _event: ProjectedEventId) -> Option<TimelineSource> {
            self.0.clone()
        }
    }

    #[test]
    fn timeline_jump_changes_attention_without_changing_the_document() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let node = place_container(
            &mut document,
            &mut attention,
            &mut selection,
            BoardSlot::new(0, 0),
        );
        let before = document.clone();
        attention.enter_navigation(NavigationMode::Timeline);
        let root = document.root_surface;
        let result = jump_to_timeline_source(
            &mut document,
            &mut attention,
            &mut selection,
            &Resolver(Some(TimelineSource {
                surface: root,
                node: node.clone(),
            })),
            ProjectedEventId(7),
        );

        assert!(result.is_accepted());
        assert_eq!(document, before);
        assert_eq!(attention.workspace_mode, WorkspaceMode::Compose);
        assert_eq!(attention.active_space, ActiveSpace::Board(root));
        assert!(selection.contains(node));
    }
}
