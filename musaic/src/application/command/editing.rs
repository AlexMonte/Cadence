//! Atomic structural edits. History stores only affected document subtrees;
//! sample buffers and unrelated project state are not copied.
use super::PlacementTarget;
use crate::{
    application::{
        editor::{EditorAttention, SelectionState},
        pipeline::scene_sync::surface_content::owned_compound_for_node,
        session::MusaicProject,
    },
    domain::{
        board::BoardSlot,
        document::{
            self, ContainerKind, DocumentNodeKind, DocumentPatch, DocumentQueries, NodeLocation,
            PlacementAddress, StackIndex, TileSpawnKind,
        },
    },
};
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use tessera::prelude::NodeId;

#[path = "variation.rs"]
pub mod variation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileEdit {
    Connect {
        request: u64,
        plan: super::connection::ConnectionPlan,
    },
    InsertNotes {
        request: u64,
        target: super::note_entry::NoteEntryTarget,
        text: String,
    },
    RenameTrick {
        id: u64,
        name: String,
    },
    RenameOutput {
        node: NodeId,
        name: String,
    },
    DefineTrick {
        node: NodeId,
        name: String,
        input: Option<NodeId>,
    },
    SetChannels {
        channels: u16,
    },
    Copy,
    Delete,
    RemoveAccidental {
        node: NodeId,
    },
    Paste,
    Duplicate,
    IndependentVariation,
    /// Transpose selected note expressions, including notes in selected containers.
    Transpose {
        semitones: i8,
    },
    MoveHere,
    Move {
        node: NodeId,
        target: PlacementTarget,
    },
    Group(ContainerKind),
    Flow {
        node: NodeId,
        edit: super::flow::FlowEdit,
    },
}

pub const TRANSPOSE_ACTIONS: [(&str, TileEdit); 4] = [
    (
        "Transpose up a semitone",
        TileEdit::Transpose { semitones: 1 },
    ),
    (
        "Transpose down a semitone",
        TileEdit::Transpose { semitones: -1 },
    ),
    (
        "Transpose up an octave",
        TileEdit::Transpose { semitones: 12 },
    ),
    (
        "Transpose down an octave",
        TileEdit::Transpose { semitones: -12 },
    ),
];

#[derive(Resource, Debug, Clone, Default)]
pub struct TileClipboard(Option<Fragment>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Fragment {
    channels: u16,
    tricks: BTreeMap<u64, crate::domain::tricks::TrickDefinition>,
    pub patch: DocumentPatch,
    roots: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TileEditRecord {
    before: Fragment,
    after: Fragment,
}

impl Fragment {
    fn capture(project: &MusaicProject, roots: Vec<NodeId>) -> Self {
        let patch = document::capture_subtree_patch(&project.document, &roots);
        Self {
            channels: project.document.channels,
            tricks: project.document.tricks.clone(),
            patch,
            roots,
        }
    }
}

/// A compact note face denotes the complete expression, including owned values.
pub(super) fn selected_roots(
    project: &MusaicProject,
    selection: &SelectionState,
    attention: &EditorAttention,
) -> Result<Vec<NodeId>, String> {
    use crate::application::editor::FocusTarget;
    let mut ids = selection.nodes.clone();
    if let FocusTarget::Atom { node } | FocusTarget::Tile { node } = &attention.focus {
        if !ids.contains(node) {
            ids = BTreeSet::from([node.clone()]);
        }
    }
    if ids.is_empty() {
        return Err("Select a tile or note stack first".into());
    }
    let queries = DocumentQueries::new(&project.document);
    for id in ids.clone() {
        if let Some(view) = owned_compound_for_node(&queries, &id) {
            ids.extend(view.groups.into_iter().flat_map(|group| group.members));
        }
    }
    let mut roots = Vec::new();
    for id in &ids {
        let mut location = project
            .document
            .graph
            .location_of(id)
            .ok_or("The selected tile no longer exists")?;
        let mut nested = false;
        while let Some(parent) = project
            .document
            .graph
            .container_node_for_surface(location.surface)
        {
            if ids.contains(&parent) {
                nested = true;
                break;
            }
            location = project
                .document
                .graph
                .location_of(&parent)
                .ok_or("Missing parent tile")?;
        }
        if !nested {
            roots.push(id.clone());
        }
    }
    roots.sort_by_key(|id| {
        project
            .document
            .graph
            .location_of(id)
            .map(|l| (l.surface, address_order(l.address)))
    });
    let surface = project
        .document
        .graph
        .location_of(&roots[0])
        .unwrap()
        .surface;
    if roots
        .iter()
        .any(|id| project.document.graph.location_of(id).unwrap().surface != surface)
    {
        return Err("Select tiles from one board or container".into());
    }
    Ok(roots)
}

pub fn target_from_focus(attention: &EditorAttention) -> Option<PlacementTarget> {
    use crate::application::editor::FocusTarget;
    match attention.focus {
        FocusTarget::EmptySlot { surface, slot } => {
            Some(PlacementTarget::BoardSlot { surface, slot })
        }
        FocusTarget::StackInsert { surface, index } => {
            Some(PlacementTarget::StackIndex { surface, index })
        }
        _ => None,
    }
}

fn target_location(target: PlacementTarget) -> NodeLocation {
    match target {
        PlacementTarget::BoardSlot { surface, slot } => NodeLocation {
            surface,
            address: PlacementAddress::BoardSlot(slot),
        },
        PlacementTarget::StackIndex { surface, index } => NodeLocation {
            surface,
            address: PlacementAddress::StackIndex(index),
        },
    }
}

pub(crate) fn spawn_kind(kind: &DocumentNodeKind) -> Result<TileSpawnKind, String> {
    Ok(match kind {
        DocumentNodeKind::Atom(node) => TileSpawnKind::Atom {
            atom: node.atom.clone(),
        },
        DocumentNodeKind::Container(node) => TileSpawnKind::Container { kind: node.kind },
        DocumentNodeKind::Output(node) => TileSpawnKind::Output {
            name: node.name.clone(),
        },
        DocumentNodeKind::Sound(node) => TileSpawnKind::sound(node.definition.clone()),
        DocumentNodeKind::TrickInstance(node) => TileSpawnKind::TrickInstance {
            prototype: node.prototype,
        },
        DocumentNodeKind::Tile(node) => TileSpawnKind::Tile {
            prototype: node.prototype,
        },
        DocumentNodeKind::FlowControl(control) => TileSpawnKind::FlowControl {
            control: control.clone(),
        },
        DocumentNodeKind::Arrangement(_) => {
            return Err("Move or copy the arrangement's source container".into());
        }
    })
}

fn validate_role(
    project: &MusaicProject,
    kind: &DocumentNodeKind,
    target: NodeLocation,
) -> Result<(), String> {
    if !project.document.surfaces.contains(target.surface) {
        return Err("The destination no longer exists".into());
    }
    let root = target.surface == project.document.root_surface;
    let allowed = match (&kind, root) {
        (DocumentNodeKind::Atom(node), true) => node.atom.numeric_rational().is_some(),
        (DocumentNodeKind::Container(_), _) => true,
        (DocumentNodeKind::Atom(_), false) => true,
        (
            DocumentNodeKind::Output(_)
            | DocumentNodeKind::Sound(_)
            | DocumentNodeKind::TrickInstance(_)
            | DocumentNodeKind::FlowControl(_),
            true,
        ) => true,
        _ => false,
    };
    if !allowed || root != matches!(target.address, PlacementAddress::BoardSlot(_)) {
        return Err(
            "Notes belong inside containers; outputs and transforms belong on the board".into(),
        );
    }
    Ok(())
}

fn destinations(
    fragment: &Fragment,
    target: NodeLocation,
) -> Result<BTreeMap<NodeId, NodeLocation>, String> {
    let first = fragment.patch.locations[&fragment.roots[0]];
    fragment
        .roots
        .iter()
        .map(|id| {
            let old = fragment.patch.locations[id];
            let address = match (first.address, old.address, target.address) {
                (
                    PlacementAddress::StackIndex(a),
                    PlacementAddress::StackIndex(b),
                    PlacementAddress::StackIndex(c),
                ) => PlacementAddress::StackIndex(StackIndex(
                    c.0.checked_add(b.0.checked_sub(a.0).ok_or("Invalid pattern order")?)
                        .ok_or("Pattern is too large")?,
                )),
                (
                    PlacementAddress::BoardSlot(a),
                    PlacementAddress::BoardSlot(b),
                    PlacementAddress::BoardSlot(c),
                ) => PlacementAddress::BoardSlot(BoardSlot::new(
                    i32::try_from(i64::from(c.x) + i64::from(b.x) - i64::from(a.x))
                        .map_err(|_| "Board position is too large")?,
                    i32::try_from(i64::from(c.y) + i64::from(b.y) - i64::from(a.y))
                        .map_err(|_| "Board position is too large")?,
                )),
                (_, _, _) if fragment.roots.len() == 1 => target.address,
                _ => {
                    return Err(
                        "Move one container at a time between different board layouts".into(),
                    );
                }
            };
            Ok((
                id.clone(),
                NodeLocation {
                    surface: target.surface,
                    address,
                },
            ))
        })
        .collect()
}

fn clone_node(
    project: &mut MusaicProject,
    source: &Fragment,
    old: &NodeId,
    location: NodeLocation,
    remap: &mut BTreeMap<NodeId, NodeId>,
) -> Result<NodeId, String> {
    let node = &source.patch.nodes[old];
    validate_role(project, &node.kind, location)?;
    let id = project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            location.surface,
            location.address,
            spawn_kind(&node.kind)?,
        )
        .map_err(|_| "The destination overlaps an existing tile".to_string())?;
    remap.insert(old.clone(), id.clone());
    if let Some(old_surface) = source.patch.container_surfaces.get(old) {
        let new_surface = project.document.graph.container_surface(&id).unwrap();
        let mut children: Vec<_> = source
            .patch
            .locations
            .iter()
            .filter(|(_, loc)| loc.surface == *old_surface)
            .collect();
        children.sort_by_key(|(_, loc)| address_order(loc.address));
        for (child, loc) in children {
            clone_node(
                project,
                source,
                child,
                NodeLocation {
                    surface: new_surface,
                    address: loc.address,
                },
                remap,
            )?;
        }
    }
    Ok(id)
}

fn paste(
    project: &mut MusaicProject,
    source: &Fragment,
    target: NodeLocation,
) -> Result<Vec<NodeId>, String> {
    paste_remapped(project, source, target).map(|(roots, _)| roots)
}

fn paste_remapped(
    project: &mut MusaicProject,
    source: &Fragment,
    target: NodeLocation,
) -> Result<(Vec<NodeId>, BTreeMap<NodeId, NodeId>), String> {
    let locations = destinations(source, target)?;
    let mut remap = BTreeMap::new();
    let mut roots = Vec::new();
    for old in &source.roots {
        roots.push(clone_node(
            project,
            source,
            old,
            locations[old],
            &mut remap,
        )?);
    }
    let source_connections = source
        .patch
        .connections
        .as_ref()
        .ok_or("Copied tiles are missing their connections")?;
    for relation in &source_connections.explicit_relations {
        use tessera::prelude::{RootRelation, StreamTarget};
        let mut relation = relation.clone();
        let (from, to) = match &mut relation {
            RootRelation::ChainedTo { from, to } => (from, to),
            RootRelation::FlowsTo { from, to } => {
                let node = match to {
                    StreamTarget::TransformInput { node, .. }
                    | StreamTarget::OutputInput { node, .. }
                    | StreamTarget::FlowControlInput { node, .. } => node,
                };
                (from, node)
            }
        };
        if let (Some(new_from), Some(new_to)) = (remap.get(&from.node), remap.get(to)) {
            from.node = new_from.clone();
            *to = new_to.clone();
            project
                .document
                .connections
                .explicit_relations
                .push(relation);
        }
    }
    for (old, new) in &remap {
        if let Some(value) = source_connections.bindings.get(old) {
            project
                .document
                .connections
                .bindings
                .insert(new.clone(), value.clone());
        }
    }
    Ok((roots, remap))
}

#[derive(Default)]
pub struct EditOutcome {
    pub record: Option<TileEditRecord>,
    pub diagnostics: Vec<crate::application::editor::EditorTransactionDiagnostic>,
}

pub fn execute(
    project: &mut MusaicProject,
    selection: &mut SelectionState,
    attention: &mut EditorAttention,
    clipboard: &mut TileClipboard,
    action: &TileEdit,
) -> Result<EditOutcome, String> {
    let source = if matches!(action, TileEdit::Connect { .. }) {
        Fragment::capture(project, vec![])
    } else if let TileEdit::Move { node, .. } = action {
        let mut selected = SelectionState::default();
        selected.nodes.insert(node.clone());
        let mut focused = attention.clone();
        focused.focus = crate::application::editor::FocusTarget::Tile { node: node.clone() };
        Fragment::capture(project, selected_roots(project, &selected, &focused)?)
    } else if let TileEdit::RemoveAccidental { node } = action {
        if !matches!(project.document.graph.node(node).map(|n| &n.kind),
            Some(DocumentNodeKind::Atom(atom)) if matches!(atom.atom, document::AtomValue::Accidental(_)))
        {
            return Err("Select an accidental to remove".into());
        }
        Fragment::capture(project, vec![node.clone()])
    } else if matches!(
        action,
        TileEdit::InsertNotes { .. } | TileEdit::SetChannels { .. } | TileEdit::RenameTrick { .. }
    ) {
        Fragment::capture(project, vec![])
    } else if let TileEdit::RenameOutput { node, .. }
    | TileEdit::DefineTrick { node, .. }
    | TileEdit::Flow { node, .. } = action
    {
        Fragment::capture(project, vec![node.clone()])
    } else if matches!(action, TileEdit::Paste) {
        clipboard.0.clone().ok_or("Copy some tiles first")?
    } else {
        Fragment::capture(project, selected_roots(project, selection, attention)?)
    };
    let previous_program = document::export_document_program(&project.document)
        .map_err(|error| format!("Cannot export this document: {error:?}"))?;
    if matches!(action, TileEdit::Copy) {
        clipboard.0 = Some(source);
        return Ok(EditOutcome::default());
    }
    let mut candidate = project.clone();
    let roots = match action {
        TileEdit::IndependentVariation => variation::apply(&mut candidate, &source.roots)?,
        TileEdit::Connect { plan, .. } => {
            super::connection::apply(&mut candidate, attention, plan)?
        }
        TileEdit::InsertNotes { target, text, .. } => {
            super::note_entry::insert(&mut candidate.document, target, text)?
        }
        TileEdit::Copy => unreachable!(),
        TileEdit::Transpose { semitones } => {
            let notes = source
                .patch
                .nodes
                .values()
                .filter_map(|node| {
                    matches!(&node.kind, DocumentNodeKind::Atom(atom)
                    if matches!(atom.atom, document::AtomValue::NoteName(_)))
                    .then_some(node.id.clone())
                })
                .collect::<Vec<_>>();
            super::musical_edits::transpose(&mut candidate.document, &notes, *semitones)?;
            source.roots.clone()
        }
        TileEdit::RenameTrick { id, name } => {
            let name = name.trim();
            if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
                return Err("Give the trick a name of 1–128 characters".into());
            }
            if candidate
                .document
                .tricks
                .iter()
                .any(|(other, d)| other != id && d.name.eq_ignore_ascii_case(name))
            {
                return Err("A trick already has that name".into());
            }
            candidate
                .document
                .tricks
                .get_mut(id)
                .ok_or("Missing trick")?
                .name = name.to_owned();
            match &attention.focus {
                crate::application::editor::FocusTarget::Tile { node } => vec![node.clone()],
                _ => selection.nodes.iter().cloned().collect(),
            }
        }
        TileEdit::RenameOutput { node, name } => {
            candidate.document.graph.rename_output(node, name)?;
            vec![node.clone()]
        }
        TileEdit::SetChannels { channels } => {
            candidate.document.channels = *channels;
            vec![]
        }
        TileEdit::DefineTrick { node, name, input } => {
            crate::application::tricks::define(
                &mut candidate.document,
                node.clone(),
                name.clone(),
                input.clone(),
            )?;
            vec![node.clone()]
        }
        TileEdit::Flow { node, edit } => {
            super::flow::apply(&mut candidate, node, edit)?;
            vec![node.clone()]
        }
        TileEdit::Delete | TileEdit::RemoveAccidental { .. } => {
            for id in &source.roots {
                let deleted = candidate
                    .document
                    .graph
                    .delete_subtree(id, &mut candidate.document.surfaces)
                    .map_err(|e| format!("Could not delete tiles: {e:?}"))?;
                for node in &deleted.nodes {
                    candidate.document.connections.bindings.remove(node);
                }
                candidate
                    .document
                    .connections
                    .explicit_relations
                    .retain(|relation| {
                        let edge = document::connection_policy::explicit_connection(relation);
                        !deleted.nodes.contains(&edge.from) && !deleted.nodes.contains(&edge.to)
                    });
                for surface in deleted.surfaces {
                    candidate
                        .document
                        .surfaces
                        .remove_container_surface(surface);
                }
            }
            if let TileEdit::RemoveAccidental { node } = action {
                let location = source.patch.locations[node];
                if let PlacementAddress::StackIndex(index) = location.address {
                    let following = candidate
                        .document
                        .graph
                        .nodes_on_surface(location.surface)
                        .into_iter()
                        .filter_map(|(loc, n)| match loc.address {
                            PlacementAddress::StackIndex(i) if i.0 > index.0 => Some((
                                n.id.clone(),
                                NodeLocation {
                                    surface: location.surface,
                                    address: PlacementAddress::StackIndex(StackIndex(i.0 - 1)),
                                },
                            )),
                            _ => None,
                        })
                        .collect();
                    candidate
                        .document
                        .graph
                        .relocate_nodes(&candidate.document.surfaces, &following)?;
                }
            }
            Vec::new()
        }
        TileEdit::Paste => paste(
            &mut candidate,
            &source,
            target_location(
                target_from_focus(attention).ok_or("Select an empty destination, then Paste")?,
            ),
        )?,
        TileEdit::Duplicate => {
            let first = source.patch.locations[&source.roots[0]];
            let mut attempt = first;
            let mut result = None;
            for offset in 1..=4096 {
                attempt.address = match first.address {
                    PlacementAddress::StackIndex(index) => PlacementAddress::StackIndex(
                        StackIndex(index.0.checked_add(offset).ok_or("Pattern is too large")?),
                    ),
                    PlacementAddress::BoardSlot(slot) => {
                        PlacementAddress::BoardSlot(BoardSlot::new(
                            slot.x
                                .checked_add(offset as i32)
                                .ok_or("Board position is too large")?,
                            slot.y,
                        ))
                    }
                };
                let mut trial = project.clone();
                if let Ok(ids) = paste(&mut trial, &source, attempt) {
                    candidate = trial;
                    result = Some(ids);
                    break;
                }
            }
            result.ok_or("No free space nearby; copy and choose a destination")?
        }
        TileEdit::Move { node, target } => {
            let target = target_location(*target);
            let existing = match target.address {
                PlacementAddress::StackIndex(index) => candidate
                    .document
                    .graph
                    .node_at_stack_index(target.surface, index),
                PlacementAddress::BoardSlot(slot) => candidate
                    .document
                    .graph
                    .node_at_board_slot(target.surface, slot),
            };
            let tile = spawn_kind(&source.patch.nodes[node].kind)?;
            if let Some(existing) = existing.filter(|existing| !source.roots.contains(existing)) {
                if source.roots.len() == 1
                    && crate::application::editor::transaction::drop::is_number(&tile)
                    && crate::application::editor::transaction::drop::note_owner(
                        &candidate.document,
                        &existing,
                    )
                    .is_some()
                {
                    let owner = crate::application::editor::transaction::drop::apply_number(
                        &mut candidate.document,
                        &existing,
                        &tile,
                    )?;
                    candidate
                        .document
                        .graph
                        .delete_subtree(node, &mut candidate.document.surfaces)
                        .map_err(|error| format!("Cannot combine number: {error:?}"))?;
                    candidate.document.connections.bindings.remove(node);
                    candidate
                        .document
                        .connections
                        .explicit_relations
                        .retain(|relation| {
                            let edge = document::connection_policy::explicit_connection(relation);
                            edge.from != *node && edge.to != *node
                        });
                    vec![owner]
                } else if matches!(target.address, PlacementAddress::StackIndex(_))
                    && source.patch.locations[node].surface == target.surface
                {
                    let first = owned_compound_for_node(
                        &DocumentQueries::new(&candidate.document),
                        &existing,
                    )
                    .and_then(|view| view.groups.first().map(|group| group.owner.clone()))
                    .unwrap_or(existing);
                    let mut cells = candidate
                        .document
                        .graph
                        .nodes_on_surface(target.surface)
                        .into_iter()
                        .map(|(loc, node)| (loc, node.id.clone()))
                        .collect::<Vec<_>>();
                    cells.sort_by_key(|(loc, _)| address_order(loc.address));
                    let original = cells.iter().map(|(_, id)| id.clone()).collect::<Vec<_>>();
                    let mut order = cells
                        .iter()
                        .map(|(_, id)| id.clone())
                        .filter(|id| !source.roots.contains(id))
                        .collect::<Vec<_>>();
                    let before = order
                        .iter()
                        .position(|id| id == &first)
                        .ok_or("The drop destination no longer exists.")?;
                    order.splice(before..before, source.roots.clone());
                    if order != original {
                        // Move the complete interval. Reassigning individual
                        // nodes to the old occupied slots could put a gap
                        // between another note and its owned octave/modifier.
                        let index = |id: &NodeId| match source.patch.locations[id].address {
                            PlacementAddress::StackIndex(i) => i.0,
                            _ => unreachable!("source is in this pattern"),
                        };
                        let start = source.roots.iter().map(index).min().unwrap();
                        let end = source.roots.iter().map(index).max().unwrap();
                        let width = end
                            .checked_sub(start)
                            .and_then(|n| n.checked_add(1))
                            .ok_or("Pattern is too large")?;
                        let destination = cells
                            .iter()
                            .find(|(_, id)| id == &first)
                            .and_then(|(loc, _)| match loc.address {
                                PlacementAddress::StackIndex(i) => Some(i.0),
                                _ => None,
                            })
                            .ok_or("The drop destination no longer exists.")?;
                        let insert = if destination > end {
                            destination
                                .checked_sub(width)
                                .ok_or("Invalid note interval")?
                        } else {
                            destination
                        };
                        let locations = cells
                            .into_iter()
                            .map(|(mut loc, id)| {
                                let PlacementAddress::StackIndex(old) = loc.address else {
                                    return Err("Notes belong inside a pattern".to_string());
                                };
                                let next = if source.roots.contains(&id) {
                                    insert.checked_add(old.0 - start)
                                } else {
                                    let compact = if old.0 > end { old.0 - width } else { old.0 };
                                    if compact >= insert {
                                        compact.checked_add(width)
                                    } else {
                                        Some(compact)
                                    }
                                }
                                .ok_or("Pattern is too large")?;
                                loc.address = PlacementAddress::StackIndex(StackIndex(next));
                                Ok((id, loc))
                            })
                            .collect::<Result<BTreeMap<_, _>, String>>()?;
                        candidate
                            .document
                            .graph
                            .relocate_nodes(&candidate.document.surfaces, &locations)?;
                    }
                    source.roots.clone()
                } else {
                    return Err("That tile is occupied. Drop a number on a note to change its octave, or choose an empty cell.".into());
                }
            } else {
                let locations = destinations(&source, target)?;
                for (id, loc) in &locations {
                    validate_role(project, &source.patch.nodes[id].kind, *loc)?;
                }
                candidate
                    .document
                    .graph
                    .relocate_nodes(&candidate.document.surfaces, &locations)?;
                source.roots.clone()
            }
        }
        TileEdit::MoveHere => {
            let target = target_location(
                target_from_focus(attention)
                    .ok_or("Select an empty destination, then Move here")?,
            );
            let locations = destinations(&source, target)?;
            for (id, loc) in &locations {
                validate_role(project, &source.patch.nodes[id].kind, *loc)?;
            }
            candidate
                .document
                .graph
                .relocate_nodes(&candidate.document.surfaces, &locations)?;
            source.roots.clone()
        }
        TileEdit::Group(kind) => {
            let first = source.patch.locations[&source.roots[0]];
            let PlacementAddress::StackIndex(start) = first.address else {
                return Err("Group notes inside a container".into());
            };
            let count = source.roots.len();
            let after_selected = start.0.checked_add(count).ok_or("Pattern is too large")?;
            if source.roots.iter().enumerate().any(|(i, id)| {
                source.patch.locations[id].address
                    != PlacementAddress::StackIndex(StackIndex(start.0.saturating_add(i)))
            }) {
                return Err("Select adjacent note stacks to group".into());
            }
            // Vacate the selected slots while retaining all selected identities.
            let end = project
                .document
                .graph
                .nodes_on_surface(first.surface)
                .into_iter()
                .filter_map(|(loc, _)| match loc.address {
                    PlacementAddress::StackIndex(i) => Some(i.0),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            let parking = end
                .checked_add(1)
                .and_then(|v| v.checked_add(count))
                .ok_or("Pattern is too large")?
                - count;
            let parked: BTreeMap<_, _> = source
                .roots
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    (
                        id.clone(),
                        NodeLocation {
                            surface: first.surface,
                            address: PlacementAddress::StackIndex(StackIndex(parking + i)),
                        },
                    )
                })
                .collect();
            candidate
                .document
                .graph
                .relocate_nodes(&candidate.document.surfaces, &parked)?;
            let group = candidate
                .document
                .graph
                .insert_tile(
                    &mut candidate.document.surfaces,
                    first.surface,
                    first.address,
                    TileSpawnKind::Container { kind: *kind },
                )
                .map_err(|e| format!("Could not group tiles: {e:?}"))?;
            let local = candidate.document.graph.container_surface(&group).unwrap();
            let locations = source
                .roots
                .iter()
                .enumerate()
                .map(|(i, id)| {
                    (
                        id.clone(),
                        NodeLocation {
                            surface: local,
                            address: PlacementAddress::StackIndex(StackIndex(i)),
                        },
                    )
                })
                .collect();
            candidate
                .document
                .graph
                .relocate_nodes(&candidate.document.surfaces, &locations)?;
            // Close only the space occupied by the grouped pieces, preserving other gaps.
            let following: BTreeMap<_, _> = project
                .document
                .graph
                .nodes_on_surface(first.surface)
                .into_iter()
                .filter_map(|(loc, node)| match loc.address {
                    PlacementAddress::StackIndex(i) if i.0 >= after_selected => Some((
                        node.id.clone(),
                        NodeLocation {
                            surface: first.surface,
                            address: PlacementAddress::StackIndex(StackIndex(i.0 - count + 1)),
                        },
                    )),
                    _ => None,
                })
                .collect();
            candidate
                .document
                .graph
                .relocate_nodes(&candidate.document.surfaces, &following)?;
            vec![group]
        }
    };
    crate::application::outputs::validate(&candidate.document)?;
    crate::application::tricks::validate(&candidate.document)?;
    candidate.document.validate()?;
    let diagnostics = if matches!(action, TileEdit::Move { .. } | TileEdit::MoveHere) {
        crate::application::editor::connection::reconnect_after_edit(
            &mut candidate.document,
            &previous_program,
            &roots,
            None,
        )?
    } else {
        Vec::new()
    };
    candidate.document.validate()?;
    candidate.document.bump_revision();
    candidate.mark_dirty();
    let Some(record) = capture_change(project, &candidate) else {
        return Ok(EditOutcome {
            record: None,
            diagnostics,
        });
    };
    *project = candidate;
    if matches!(action, TileEdit::IndependentVariation) {
        *attention = EditorAttention::new(project.document.root_surface);
    }
    // A pointer gesture moved one visible face. Keep its note inspector open;
    // owned members are still expanded by subsequent structural commands.
    selection.nodes = if matches!(action, TileEdit::Move { .. }) {
        roots.first().cloned().into_iter().collect()
    } else {
        roots.iter().cloned().collect()
    };
    selection.anchor = roots.first().cloned();
    attention.focus = roots
        .first()
        .map(|node| crate::application::editor::FocusTarget::Tile { node: node.clone() })
        .unwrap_or(crate::application::editor::FocusTarget::None);
    if !project.document.surfaces.contains(attention.active_board()) {
        *attention = EditorAttention::new(project.document.root_surface);
    }
    Ok(EditOutcome {
        record: Some(record),
        diagnostics,
    })
}

/// Captures only affected subtrees and output metadata after a successful edit.
/// Node identity, owned surfaces, and neighbor port changes survive replay;
/// allocator counters, decoded assets, and unrelated document state are excluded.
pub(super) fn capture_change(
    before: &MusaicProject,
    after: &MusaicProject,
) -> Option<TileEditRecord> {
    let mut changed = BTreeSet::new();
    for node in before
        .document
        .graph
        .nodes()
        .chain(after.document.graph.nodes())
    {
        let id = &node.id;
        if before.document.graph.node(id) != after.document.graph.node(id)
            || before.document.graph.location_of(id) != after.document.graph.location_of(id)
            || before.document.connections.bindings.get(id)
                != after.document.connections.bindings.get(id)
        {
            changed.insert(id.clone());
        }
    }
    if changed.is_empty()
        && before.document.connections.explicit_relations
            == after.document.connections.explicit_relations
        && before.document.channels == after.document.channels
        && before.document.tricks == after.document.tricks
    {
        return None;
    }
    let ids: Vec<_> = changed.into_iter().collect();
    Some(TileEditRecord {
        before: Fragment::capture(before, ids.clone()),
        after: Fragment::capture(after, ids),
    })
}

pub fn restore(
    project: &mut MusaicProject,
    record: &TileEditRecord,
    redo: bool,
) -> Result<(), String> {
    let fragment = if redo { &record.after } else { &record.before };
    let ids: BTreeSet<_> = record
        .before
        .patch
        .nodes
        .keys()
        .chain(record.after.patch.nodes.keys())
        .cloned()
        .collect();
    let mut candidate = project.clone();
    candidate.document.channels = fragment.channels;
    candidate.document.tricks = fragment.tricks.clone();
    for id in &ids {
        if candidate.document.graph.contains_node(id) {
            let deleted = candidate
                .document
                .graph
                .delete_subtree(id, &mut candidate.document.surfaces)
                .map_err(|e| format!("Cannot restore edit: {e:?}"))?;
            for surface in deleted.surfaces {
                candidate
                    .document
                    .surfaces
                    .remove_container_surface(surface);
            }
        }
        candidate.document.connections.bindings.remove(id);
        candidate
            .document
            .connections
            .explicit_relations
            .retain(|relation| {
                let edge = document::connection_policy::explicit_connection(relation);
                edge.from != *id && edge.to != *id
            });
    }
    document::apply_document_patch(&mut candidate.document, fragment.patch.clone())?;
    crate::application::outputs::validate(&candidate.document)?;
    crate::application::tricks::validate(&candidate.document)?;
    candidate.document.validate()?;
    candidate.mark_dirty();
    *project = candidate;
    Ok(())
}

fn address_order(address: PlacementAddress) -> (i64, i64) {
    match address {
        PlacementAddress::StackIndex(i) => (0, i.0 as i64),
        PlacementAddress::BoardSlot(p) => (i64::from(p.y), i64::from(p.x)),
    }
}
