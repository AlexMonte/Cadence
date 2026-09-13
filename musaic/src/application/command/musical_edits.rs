//! Musical edits mutate ordinary pitch pieces on the transaction's candidate.
//! The shared structural-edit owner handles atomic acceptance and undo.
use crate::{
    application::pipeline::scene_sync::surface_content::{
        OwnedTileGroupRole, owned_compound_for_node,
    },
    domain::document::{
        Accidental, AtomValue, DocumentNodeKind, DocumentQueries, MusaicDocument, NodeLocation,
        NoteName, PlacementAddress, StackIndex, TileSpawnKind,
    },
};
use std::collections::BTreeMap;
use tessera::prelude::NodeId;

struct PitchEdit {
    note: NodeId,
    name: NoteName,
    accidental: Option<(Option<NodeId>, Accidental)>,
    octave: Option<(Option<NodeId>, i8)>,
}

pub(super) fn transpose(
    document: &mut MusaicDocument,
    notes: &[NodeId],
    semitones: i8,
) -> Result<(), String> {
    if notes.is_empty() {
        return Err("Select notes or a container with pitched notes first.".into());
    }
    if semitones == 0 {
        return Ok(());
    }
    // Validate the whole selection before changing any pitch. Identity-based
    // plans remain valid when a newly needed accidental shifts later pieces.
    let edits = notes
        .iter()
        .map(|id| plan(document, id, semitones))
        .collect::<Result<Vec<_>, _>>()?;
    for edit in edits {
        document
            .graph
            .set_atom_value(&edit.note, AtomValue::NoteName(edit.name))?;
        if let Some((owner, value)) = edit.accidental {
            match owner {
                Some(owner) => {
                    document
                        .graph
                        .set_atom_value(&owner, AtomValue::Accidental(value))?;
                }
                None => insert_pitch_piece(document, &edit.note, AtomValue::Accidental(value))?,
            }
        }
        if let Some((owner, value)) = edit.octave {
            match owner {
                Some(owner) => {
                    if matches!(atom(document, &owner)?, AtomValue::Octave(_)) {
                        document
                            .graph
                            .set_atom_value(&owner, AtomValue::Octave(value))?;
                    } else {
                        document.graph.bind_number_as_octave(&owner, value)?;
                    }
                }
                None => insert_pitch_piece(document, &edit.note, AtomValue::Octave(value))?,
            }
        }
    }
    Ok(())
}

fn atom<'a>(document: &'a MusaicDocument, node: &NodeId) -> Result<&'a AtomValue, String> {
    match document.graph.node(node).map(|node| &node.kind) {
        Some(DocumentNodeKind::Atom(atom)) => Ok(&atom.atom),
        _ => Err("A selected pitch no longer exists.".into()),
    }
}

fn plan(document: &MusaicDocument, note: &NodeId, semitones: i8) -> Result<PitchEdit, String> {
    let AtomValue::NoteName(name) = *atom(document, note)? else {
        return Err("Select a pitched note.".into());
    };
    let compound = owned_compound_for_node(&DocumentQueries::new(document), note)
        .ok_or("Could not resolve the selected note's pitch pieces.")?;
    let mut octave_owner = None;
    let mut accidental_owner = None;
    let mut octave = 4;
    let mut accidental = Accidental::Natural;
    for group in &compound.groups {
        match group.role {
            OwnedTileGroupRole::Octave => {
                if octave_owner.replace(group.owner.clone()).is_some() {
                    return Err("Finish the note's octave before transposing.".into());
                }
                octave = match atom(document, &group.owner)? {
                    AtomValue::Octave(value) => i16::from(*value),
                    value => value
                        .numeric_rational()
                        .filter(|v| v.denominator == 1)
                        .and_then(|v| i16::try_from(v.numerator).ok())
                        .ok_or("The note needs a whole-number octave.")?,
                };
            }
            OwnedTileGroupRole::Accidental => {
                if accidental_owner.replace(group.owner.clone()).is_some() {
                    return Err("Finish the note's accidental before transposing.".into());
                }
                if let AtomValue::Accidental(value) = atom(document, &group.owner)? {
                    accidental = *value;
                }
            }
            _ => {}
        }
    }
    if !(-1..=9).contains(&octave) {
        return Err("Set an octave between −1 and 9 before transposing.".into());
    }
    let natural = match name {
        NoteName::C => 0,
        NoteName::D => 2,
        NoteName::E => 4,
        NoteName::F => 5,
        NoteName::G => 7,
        NoteName::A => 9,
        NoteName::B => 11,
    };
    let offset = match accidental {
        Accidental::Flat => -1,
        Accidental::Natural => 0,
        Accidental::Sharp => 1,
    };
    let pitch = (octave + 1) * 12 + natural + offset + i16::from(semitones);
    if !(0..=131).contains(&pitch) {
        return Err(
            "The whole selection must stay between C−1 and B9; no notes were changed.".into(),
        );
    }
    let preserve_octave = octave + i16::from(semitones) / 12;
    let (name, accidental, next_octave) =
        if semitones % 12 == 0 && (-1..=9).contains(&preserve_octave) {
            (name, accidental, preserve_octave as i8)
        } else {
            let flat = accidental == Accidental::Flat;
            let (name, accidental) = match pitch.rem_euclid(12) {
                0 => (NoteName::C, Accidental::Natural),
                1 if flat => (NoteName::D, Accidental::Flat),
                1 => (NoteName::C, Accidental::Sharp),
                2 => (NoteName::D, Accidental::Natural),
                3 if flat => (NoteName::E, Accidental::Flat),
                3 => (NoteName::D, Accidental::Sharp),
                4 => (NoteName::E, Accidental::Natural),
                5 => (NoteName::F, Accidental::Natural),
                6 if flat => (NoteName::G, Accidental::Flat),
                6 => (NoteName::F, Accidental::Sharp),
                7 => (NoteName::G, Accidental::Natural),
                8 if flat => (NoteName::A, Accidental::Flat),
                8 => (NoteName::G, Accidental::Sharp),
                9 => (NoteName::A, Accidental::Natural),
                10 if flat => (NoteName::B, Accidental::Flat),
                10 => (NoteName::A, Accidental::Sharp),
                _ => (NoteName::B, Accidental::Natural),
            };
            (name, accidental, (pitch.div_euclid(12) - 1) as i8)
        };
    Ok(PitchEdit {
        note: note.clone(),
        name,
        accidental: (accidental_owner.is_some() || accidental != Accidental::Natural)
            .then_some((accidental_owner, accidental)),
        octave: (octave_owner.is_some() || next_octave != 4).then_some((octave_owner, next_octave)),
    })
}

fn insert_pitch_piece(
    document: &mut MusaicDocument,
    note: &NodeId,
    value: AtomValue,
) -> Result<(), String> {
    let location = document
        .graph
        .location_of(note)
        .ok_or("The note no longer exists.")?;
    let PlacementAddress::StackIndex(index) = location.address else {
        return Err("Notes belong inside a pattern.".into());
    };
    let insert = index.0.checked_add(1).ok_or("Pattern is too large.")?;
    let following = document
        .graph
        .nodes_on_surface(location.surface)
        .into_iter()
        .filter_map(|(loc, node)| match loc.address {
            PlacementAddress::StackIndex(index) if index.0 >= insert => {
                Some((node.id.clone(), index.0))
            }
            _ => None,
        })
        .map(|(node, index)| {
            Ok((
                node,
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
            TileSpawnKind::Atom { atom: value },
        )
        .map_err(|error| format!("Could not add a pitch piece: {error:?}"))?;
    Ok(())
}
