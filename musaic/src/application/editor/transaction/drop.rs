//! Contextual drops preserve the note and its independently owned modifiers.
use crate::{
    application::pipeline::scene_sync::surface_content::{
        OwnedTileGroupRole, owned_compound_for_node,
    },
    domain::document::{
        AtomValue, DocumentNodeKind, DocumentQueries, MusaicDocument, NodeLocation,
        PlacementAddress, StackIndex, TileSpawnKind,
    },
};
use std::collections::BTreeMap;
use tessera::prelude::NodeId;

pub fn note_owner(document: &MusaicDocument, target: &NodeId) -> Option<NodeId> {
    let view = owned_compound_for_node(&DocumentQueries::new(document), target)?;
    view.groups
        .into_iter()
        .find(|group| group.role == OwnedTileGroupRole::Pitch)
        .map(|group| group.owner)
}

pub fn is_number(tile: &TileSpawnKind) -> bool {
    matches!(tile, TileSpawnKind::Atom { atom } if atom.numeric_rational().is_some() || matches!(atom, AtomValue::Octave(_)))
}

/// Change an existing octave in place, or insert one without replacing the next
/// note. The caller commits this candidate together with its board and history.
pub fn apply_number(
    document: &mut MusaicDocument,
    target: &NodeId,
    tile: &TileSpawnKind,
) -> Result<NodeId, String> {
    let TileSpawnKind::Atom { atom } = tile else {
        return Err("Drop a number onto a note to set its octave.".into());
    };
    let value = match atom {
        AtomValue::Octave(value) => i64::from(*value),
        _ => {
            let number = atom
                .numeric_rational()
                .ok_or("Drop a number onto a note to set its octave.")?;
            if number.denominator != 1 {
                return Err("A note's octave must be a whole number from -1 to 9.".into());
            }
            number.numerator
        }
    };
    if !(-1..=9).contains(&value) {
        return Err("A note's octave must be a whole number from -1 to 9.".into());
    }
    let owner = note_owner(document, target)
        .ok_or("Drop this number onto a note, or an empty numeric slot.")?;
    let view = owned_compound_for_node(&DocumentQueries::new(document), &owner)
        .ok_or("The note no longer exists.")?;
    if let Some(group) = view
        .groups
        .iter()
        .find(|group| group.role == OwnedTileGroupRole::Octave)
    {
        document
            .graph
            .set_atom_value(&group.owner, AtomValue::Octave(value as i8))?;
    } else {
        let location = document
            .graph
            .location_of(&owner)
            .ok_or("The note no longer exists.")?;
        let PlacementAddress::StackIndex(index) = location.address else {
            return Err("Notes belong inside a pattern.".into());
        };
        let insert = index.0.checked_add(1).ok_or("Pattern is too large.")?;
        // A free number already adjacent to this note can become its octave
        // without replacing the tile or changing its stable identity.
        let mut next = insert;
        while let Some(id) = document
            .graph
            .node_at_stack_index(location.surface, StackIndex(next))
        {
            let Some(DocumentNodeKind::Atom(atom)) =
                document.graph.node(&id).map(|node| &node.kind)
            else {
                break;
            };
            match atom.atom {
                AtomValue::Accidental(_) => {
                    next = next.checked_add(1).ok_or("Pattern is too large.")?
                }
                AtomValue::Number(_) | AtomValue::Ratio(_) => {
                    document.graph.bind_number_as_octave(&id, value as i8)?;
                    return Ok(owner);
                }
                _ => break,
            }
        }

        let following = document
            .graph
            .nodes_on_surface(location.surface)
            .into_iter()
            .filter_map(|(loc, node)| match loc.address {
                PlacementAddress::StackIndex(i) if i.0 >= insert => Some((node.id.clone(), i.0)),
                _ => None,
            })
            .map(|(id, index)| {
                Ok((
                    id,
                    NodeLocation {
                        surface: location.surface,
                        address: PlacementAddress::StackIndex(StackIndex(
                            index.checked_add(1).ok_or("Pattern is too large.")?,
                        )),
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, String>>()?;
        document
            .graph
            .relocate_nodes(&document.surfaces, &following)?;
        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                location.surface,
                PlacementAddress::StackIndex(StackIndex(insert)),
                TileSpawnKind::Atom {
                    atom: AtomValue::Octave(value as i8),
                },
            )
            .map_err(|error| format!("Cannot set octave: {error:?}"))?;
    }
    // Changing octave must retain the note itself, including accidentals and all
    // modifier-owned numbers. No separate numeric argument is left behind.
    debug_assert!(matches!(
        document.graph.node(&owner).map(|node| &node.kind),
        Some(DocumentNodeKind::Atom(_))
    ));
    Ok(owner)
}
