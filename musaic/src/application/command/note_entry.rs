//! Complete-note entry expands to ordinary authored atoms in one structural edit.
use super::PlacementTarget;
use crate::{
    application::{
        editor::{EditorAttention, FocusTarget},
        pipeline::scene_sync::surface_content::owned_compound_for_node,
    },
    domain::{
        board::{BoardSlot, BoardSurfaceId},
        document::{
            Accidental, AtomValue, ContainerKind, DocumentNodeKind, DocumentQueries,
            MusaicDocument, NodeLocation, NoteName, PlacementAddress, StackIndex, TileSpawnKind,
        },
    },
};
use bevy::prelude::Message;
use serde::{Deserialize, Serialize};
use tessera::prelude::NodeId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteEntryTarget {
    Cursor(PlacementTarget),
    /// Resolve the complete expression at execution time, preserving modifier ownership.
    Expression {
        node: NodeId,
        after: bool,
    },
    End(BoardSurfaceId),
}
#[derive(Message, Debug, Clone)]
pub struct NoteEntryReceipt {
    pub request: u64,
    pub result: Result<(), String>,
    /// Exact next insertion anchor, resolved by the command owner after commit.
    pub continuation: Option<NoteEntryTarget>,
}

pub(super) fn continuation(
    document: &MusaicDocument,
    roots: &std::collections::BTreeSet<NodeId>,
) -> Option<NoteEntryTarget> {
    if roots.len() == 1 {
        let node = roots.first()?;
        if let Some(surface) = document.graph.container_surface(node) {
            return Some(NoteEntryTarget::End(surface));
        }
    }
    roots
        .iter()
        .filter_map(|node| {
            let location = document.graph.location_of(node)?;
            let PlacementAddress::StackIndex(index) = location.address else {
                return None;
            };
            Some((index.0, node))
        })
        .max_by_key(|(index, _)| *index)
        .map(|(_, node)| NoteEntryTarget::Expression {
            node: node.clone(),
            after: true,
        })
}

/// Deliberately small convenience syntax, not a separately stored musical language.
/// A token is an explicit note+octave or a rest; adjacent tokens remain sequential.
pub fn parse(text: &str) -> Result<Vec<Vec<AtomValue>>, String> {
    if text.len() > 2048 {
        return Err("Enter at most 2048 characters at once.".into());
    }
    let mut notes = Vec::new();
    for (index, token) in text.split_whitespace().enumerate() {
        if index >= 128 {
            return Err("Insert at most 128 notes or rests at once.".into());
        }
        if token == "~" {
            notes.push(vec![AtomValue::Rest]);
            continue;
        }
        let invalid = || {
            format!(
                "Item {}: “{}” is not a complete note. Use C4, F#4, Bb3 or ~.",
                index + 1,
                token
            )
        };
        let mut chars = token.chars();
        let name = match chars.next().map(|c| c.to_ascii_uppercase()) {
            Some('A') => NoteName::A,
            Some('B') => NoteName::B,
            Some('C') => NoteName::C,
            Some('D') => NoteName::D,
            Some('E') => NoteName::E,
            Some('F') => NoteName::F,
            Some('G') => NoteName::G,
            _ => return Err(invalid()),
        };
        let mut rest = chars.as_str();
        let accidental = match rest.chars().next() {
            Some('#' | '♯') => Some(Accidental::Sharp),
            Some('b' | '♭') => Some(Accidental::Flat),
            Some('♮') => Some(Accidental::Natural),
            _ => None,
        };
        if accidental.is_some() {
            rest = &rest[rest.chars().next().unwrap().len_utf8()..];
        }
        if rest.is_empty() || !rest.bytes().all(|c| c.is_ascii_digit()) {
            return Err(invalid());
        }
        let octave: i8 = rest.parse().map_err(|_| invalid())?;
        if !(0..=9).contains(&octave) {
            return Err(format!("Item {}: use an octave from 0 to 9.", index + 1));
        }
        let mut atoms = vec![AtomValue::NoteName(name)];
        if let Some(accidental) = accidental {
            atoms.push(AtomValue::Accidental(accidental));
        }
        atoms.push(AtomValue::Octave(octave));
        notes.push(atoms);
    }
    if notes.is_empty() {
        return Err("Enter notes such as C4 E4 G4 ~.".into());
    }
    Ok(notes)
}

pub fn target(
    document: &MusaicDocument,
    attention: &EditorAttention,
    after: bool,
) -> NoteEntryTarget {
    match &attention.focus {
        FocusTarget::EmptySlot { surface, slot } => {
            NoteEntryTarget::Cursor(PlacementTarget::BoardSlot {
                surface: *surface,
                slot: *slot,
            })
        }
        FocusTarget::StackInsert { surface, index } => {
            NoteEntryTarget::Cursor(PlacementTarget::StackIndex {
                surface: *surface,
                index: *index,
            })
        }
        FocusTarget::Tile { node } | FocusTarget::Atom { node } => {
            if document
                .graph
                .location_of(node)
                .is_some_and(|l| matches!(l.address, PlacementAddress::BoardSlot(_)))
            {
                if let Some(surface) = document.graph.container_surface(node) {
                    return NoteEntryTarget::End(surface);
                }
            }
            NoteEntryTarget::Expression {
                node: node.clone(),
                after,
            }
        }
        _ if attention.active_board() == document.root_surface => {
            NoteEntryTarget::Cursor(PlacementTarget::BoardSlot {
                surface: document.root_surface,
                slot: BoardSlot::new(0, 0),
            })
        }
        _ => NoteEntryTarget::End(attention.active_board()),
    }
}

pub fn describe(target: &NoteEntryTarget) -> &'static str {
    match target {
        NoteEntryTarget::Cursor(PlacementTarget::BoardSlot { .. }) => {
            "Creates a new sequence at the board cursor. Connect it to an instrument to hear it."
        }
        NoteEntryTarget::Expression { after: false, .. } => {
            "Inserts before the focused expression; its octave and modifiers stay together."
        }
        NoteEntryTarget::Expression { after: true, .. } => {
            "Inserts after the focused expression and all its modifiers."
        }
        NoteEntryTarget::End(_) => {
            "Appends to the selected pattern. Existing note identities and modifiers stay intact."
        }
        _ => "Inserts at the pattern cursor and moves following expressions together.",
    }
}

fn expression_boundary(
    document: &MusaicDocument,
    node: &NodeId,
    after: bool,
) -> Result<(BoardSurfaceId, usize), String> {
    let location = document
        .graph
        .location_of(node)
        .ok_or("The insertion target no longer exists.")?;
    let PlacementAddress::StackIndex(index) = location.address else {
        return Err("Select a pattern or an empty board cell to insert notes.".into());
    };
    let mut indices = vec![index.0];
    if let Some(compound) = owned_compound_for_node(&DocumentQueries::new(document), node) {
        for member in compound.groups.into_iter().flat_map(|group| group.members) {
            if let Some(NodeLocation {
                address: PlacementAddress::StackIndex(index),
                ..
            }) = document.graph.location_of(&member)
            {
                indices.push(index.0);
            }
        }
    }
    let index = if after {
        indices
            .into_iter()
            .max()
            .unwrap()
            .checked_add(1)
            .ok_or("Pattern is too large")?
    } else {
        indices.into_iter().min().unwrap()
    };
    Ok((location.surface, index))
}

/// Mutates only a transaction candidate. The caller validates, commits and records
/// the affected fragment through the same path as pointer structural edits.
pub(super) fn insert(
    document: &mut MusaicDocument,
    target: &NoteEntryTarget,
    text: &str,
) -> Result<Vec<NodeId>, String> {
    let notes = parse(text)?;
    let mut roots = Vec::new();
    let (mut surface, index) = match target {
        NoteEntryTarget::Cursor(PlacementTarget::BoardSlot { surface, slot }) => {
            if *surface != document.root_surface {
                return Err("Choose a board cell or a pattern insertion point.".into());
            }
            let container = document
                .graph
                .insert_tile(
                    &mut document.surfaces,
                    *surface,
                    PlacementAddress::BoardSlot(*slot),
                    TileSpawnKind::Container {
                        kind: ContainerKind::Sequence,
                    },
                )
                .map_err(|_| {
                    "A new sequence needs five empty board cells. Move the cursor to free space."
                        .to_string()
                })?;
            let local = document
                .graph
                .container_surface(&container)
                .ok_or("Missing new pattern")?;
            roots.push(container);
            (local, 0)
        }
        NoteEntryTarget::Cursor(PlacementTarget::StackIndex { surface, index }) => {
            // Even an externally supplied raw index cannot split a compound note.
            let at = document
                .graph
                .nodes_on_surface(*surface)
                .into_iter()
                .find(|(loc, _)| loc.address == PlacementAddress::StackIndex(*index))
                .map(|(_, node)| node.id.clone());
            if let Some(node) = at {
                expression_boundary(document, &node, false)?
            } else {
                (*surface, index.0)
            }
        }
        NoteEntryTarget::Expression { node, after } => expression_boundary(document, node, *after)?,
        NoteEntryTarget::End(surface) => {
            let end = document
                .graph
                .nodes_on_surface(*surface)
                .into_iter()
                .filter_map(|(loc, _)| match loc.address {
                    PlacementAddress::StackIndex(i) => Some(i.0),
                    _ => None,
                })
                .max()
                .map(|i| i.checked_add(1).ok_or("Pattern is too large"))
                .transpose()?
                .unwrap_or(0);
            (*surface, end)
        }
    };
    let owner = document
        .graph
        .container_node_for_surface(surface)
        .ok_or("The pattern no longer exists.")?;
    let kind = match document.graph.node(&owner).map(|n| &n.kind) {
        Some(DocumentNodeKind::Container(c)) => c.kind,
        _ => return Err("The pattern no longer exists.".into()),
    };
    if kind == ContainerKind::Arrangement {
        return Err("Insert notes inside a sequence; arrangements contain patterns.".into());
    }
    // Text order always denotes a sequence. Inside a layer/alternation, keep
    // that sequence as one nested expression instead of silently making a chord.
    let nested = kind != ContainerKind::Sequence;
    let width: usize = if nested {
        1
    } else {
        notes.iter().map(Vec::len).sum()
    };
    let following = document
        .graph
        .nodes_on_surface(surface)
        .into_iter()
        .filter_map(|(loc, node)| match loc.address {
            PlacementAddress::StackIndex(i) if i.0 >= index => Some((i.0, node.id.clone())),
            _ => None,
        })
        .map(|(i, node)| {
            Ok((
                node,
                NodeLocation {
                    surface,
                    address: PlacementAddress::StackIndex(StackIndex(
                        i.checked_add(width).ok_or("Pattern is too large")?,
                    )),
                },
            ))
        })
        .collect::<Result<_, String>>()?;
    document
        .graph
        .relocate_nodes(&document.surfaces, &following)?;
    let mut next = index;
    if nested {
        let sequence = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                surface,
                PlacementAddress::StackIndex(StackIndex(index)),
                TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            )
            .map_err(|e| format!("Cannot insert sequence: {e:?}"))?;
        surface = document
            .graph
            .container_surface(&sequence)
            .ok_or("Missing new sequence")?;
        roots.push(sequence);
        next = 0;
    }
    for note in notes {
        for (part, atom) in note.into_iter().enumerate() {
            let node = document
                .graph
                .insert_tile(
                    &mut document.surfaces,
                    surface,
                    PlacementAddress::StackIndex(StackIndex(next)),
                    TileSpawnKind::Atom { atom },
                )
                .map_err(|e| format!("Cannot insert this note: {e:?}"))?;
            if part == 0
                && roots
                    .first()
                    .is_none_or(|id| document.graph.container_surface(id).is_none())
            {
                roots.push(node);
            }
            next = next.checked_add(1).ok_or("Pattern is too large")?;
        }
    }
    Ok(roots)
}
