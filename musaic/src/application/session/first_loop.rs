//! A small, ordinary document with a complete route to the speakers.
use super::MusaicProject;
use crate::domain::{
    board::BoardSlot,
    document::{
        AtomValue, ContainerKind, NoteName, PlacementAddress, StackIndex, TileSpawnKind,
        bind_tiles, export_document_program,
    },
};
use tessera::prelude::{NodeId, SpatialSide};

/// Launch targets belong to this document only; they are not persisted as tutorial state.
pub struct FirstLoop {
    pub project: MusaicProject,
    pub pattern: NodeId,
    pub first_note: NodeId,
    pub first_octave: NodeId,
}

pub fn first_loop() -> FirstLoop {
    let mut project = MusaicProject::new_empty();
    project.metadata.display_name = "My first loop".into();
    project.metadata.dirty = true;
    let document = &mut project.document;
    document.playback.bpm = 120.0;
    document.playback.beats_per_cycle = 4;
    let root = document.root_surface;
    let pattern = document
        .graph
        .insert_tile(
            &mut document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(13, 15)),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        )
        .expect("starter pattern has a free position");
    let surface = document
        .graph
        .container_surface(&pattern)
        .expect("pattern surface");
    let mut notes = Vec::new();
    for (index, atom) in [
        AtomValue::NoteName(NoteName::C),
        AtomValue::Octave(4),
        AtomValue::NoteName(NoteName::E),
        AtomValue::Octave(4),
        AtomValue::NoteName(NoteName::G),
        AtomValue::Octave(4),
        AtomValue::Rest,
    ]
    .into_iter()
    .enumerate()
    {
        notes.push(
            document
                .graph
                .insert_tile(
                    &mut document.surfaces,
                    surface,
                    PlacementAddress::StackIndex(StackIndex(index)),
                    TileSpawnKind::Atom { atom },
                )
                .expect("starter notes have distinct positions"),
        );
    }
    let instrument = document
        .graph
        .insert_tile(
            &mut document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(18, 15)),
            TileSpawnKind::sound(crate::domain::instrument::InstrumentDefinition::default()),
        )
        .expect("starter instrument follows the five-cell pattern");
    let output = document
        .graph
        .insert_tile(
            &mut document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(19, 15)),
            TileSpawnKind::Output {
                name: "Melody".into(),
            },
        )
        .expect("starter output follows the instrument");
    let mut program = export_document_program(document).expect("starter exports for compilation");
    for (from, to) in [(&pattern, &instrument), (&instrument, &output)] {
        bind_tiles(&mut program, from, to, SpatialSide::East).expect("starter neighbors bind");
    }
    document.replace_connections_from(&program);
    FirstLoop {
        project,
        pattern,
        first_note: notes[0].clone(),
        first_octave: notes[1].clone(),
    }
}
