//! A first project made entirely from the same document authoring operations
//! used by the editor. It contains no precompiled patterns or audio commands.

use tessera::prelude::{NodeId, SpatialSide};

use super::MusaicProject;
use crate::domain::{
    board::{BoardSlot, BoardSurfaceId},
    document::{
        Accidental, AtomValue, ContainerKind, DrumHit, GraphTilePrototypeId, MusaicDocument,
        NoteName, OperatorValue, PlacementAddress, StackIndex, TileSpawnKind, bind_authorized_edge,
        bind_tiles, export_document_program,
    },
    instrument::{InstrumentDefinition, InstrumentSource},
};

impl MusaicProject {
    /// Compact reference beat: `bd ~ sd ~` → Fast ×2 → Kit → Drums.
    pub fn reference_demo() -> Self {
        let mut project = Self::new_empty();
        project.metadata.display_name = "Reference beat".into();
        project.metadata.dirty = true;
        project.document.playback.bpm = 120.0;
        project.document.playback.beats_per_cycle = 4;
        let document = &mut project.document;
        let root = document.root_surface;
        let sequence = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(13, 15)),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        );
        let sequence_surface = document
            .graph
            .container_surface(&sequence)
            .expect("reference beat sequence surface");
        insert_atoms(
            document,
            sequence_surface,
            &[
                AtomValue::DrumHit(DrumHit::Bd),
                AtomValue::Rest,
                AtomValue::DrumHit(DrumHit::Sd),
                AtomValue::Rest,
            ],
        );
        let fast = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(18, 15)),
            TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(1),
            },
        );
        let rate = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(18, 14)),
            TileSpawnKind::Atom {
                atom: AtomValue::Number(2),
            },
        );
        let kit = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(19, 15)),
            TileSpawnKind::sound(InstrumentDefinition::new(InstrumentSource::Kit)),
        );
        let output = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(20, 15)),
            TileSpawnKind::Output {
                name: "Drums".into(),
            },
        );
        let mut program =
            export_document_program(document).expect("reference beat exports for compilation");
        for (from, to, side) in [
            (&sequence, &fast, SpatialSide::East),
            (&rate, &fast, SpatialSide::South),
            (&fast, &kit, SpatialSide::East),
            (&kit, &output, SpatialSide::East),
        ] {
            let edge = crate::domain::document::authorize_connection(&program, from, to)
                .unwrap_or_else(|error| {
                    panic!(
                        "reference beat neighbors bind from {} to {} on {side:?}: {error:?}",
                        from.0, to.0
                    )
                });
            assert_eq!(edge.side, side);
            bind_authorized_edge(&mut program, from, to, &edge)
                .expect("authorized reference beat connection applies");
        }
        document.replace_connections_from(&program);
        project
    }

    /// Editable example: C#4 @2 ×3, E4, then a pair of alternating patterns.
    /// The root Fast tile owns a connected rate of 2. Opening never starts playback.
    pub fn demo() -> Self {
        let mut project = Self::new_empty();
        project.metadata.display_name = "Small changes".into();
        project.metadata.dirty = true;
        let document = &mut project.document;
        document.playback.bpm = 120.0;
        document.playback.beats_per_cycle = 4;
        let root = document.root_surface;
        let sequence = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(13, 15)),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        );
        let sequence_surface = document
            .graph
            .container_surface(&sequence)
            .expect("sequence surface");
        insert_atoms(
            document,
            sequence_surface,
            &[
                AtomValue::NoteName(NoteName::C),
                AtomValue::Accidental(Accidental::Sharp),
                AtomValue::Octave(4),
                AtomValue::Operator(OperatorValue::At),
                AtomValue::Number(2),
                AtomValue::Operator(OperatorValue::Multiply),
                AtomValue::Number(3),
                AtomValue::NoteName(NoteName::E),
                AtomValue::Octave(4),
            ],
        );
        let alternating = insert(
            document,
            sequence_surface,
            PlacementAddress::StackIndex(StackIndex(9)),
            TileSpawnKind::Container {
                kind: ContainerKind::Alternating,
            },
        );
        let alternatives = document
            .graph
            .container_surface(&alternating)
            .expect("alternate surface");
        for (index, atoms) in [
            vec![
                AtomValue::NoteName(NoteName::G),
                AtomValue::Octave(4),
                AtomValue::NoteName(NoteName::B),
                AtomValue::Octave(4),
            ],
            vec![
                AtomValue::NoteName(NoteName::D),
                AtomValue::Octave(4),
                AtomValue::NoteName(NoteName::F),
                AtomValue::Accidental(Accidental::Sharp),
                AtomValue::Octave(4),
            ],
        ]
        .into_iter()
        .enumerate()
        {
            let branch = insert(
                document,
                alternatives,
                PlacementAddress::StackIndex(StackIndex(index)),
                TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            );
            let surface = document
                .graph
                .container_surface(&branch)
                .expect("nested sequence surface");
            insert_atoms(document, surface, &atoms);
        }
        let fast = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(18, 15)),
            TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(1),
            },
        );
        let rate = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(18, 14)),
            TileSpawnKind::Atom {
                atom: AtomValue::Number(2),
            },
        );
        let instrument = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(19, 15)),
            TileSpawnKind::sound(InstrumentDefinition::default()),
        );
        let output = insert(
            document,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(20, 15)),
            TileSpawnKind::Output {
                name: String::new(),
            },
        );
        let mut program =
            export_document_program(document).expect("example exports for compilation");
        for (from, to, side) in [
            (&sequence, &fast, SpatialSide::East),
            (&rate, &fast, SpatialSide::South),
            (&fast, &instrument, SpatialSide::East),
            (&instrument, &output, SpatialSide::East),
        ] {
            bind_tiles(&mut program, from, to, side).expect("example neighbors bind");
        }
        document.replace_connections_from(&program);
        project
    }
}

fn insert(
    document: &mut MusaicDocument,
    surface: BoardSurfaceId,
    address: PlacementAddress,
    tile: TileSpawnKind,
) -> NodeId {
    document
        .graph
        .insert_tile(&mut document.surfaces, surface, address, tile)
        .expect("example has valid, non-overlapping authored positions")
}

fn insert_atoms(document: &mut MusaicDocument, surface: BoardSurfaceId, atoms: &[AtomValue]) {
    for (index, atom) in atoms.iter().enumerate() {
        insert(
            document,
            surface,
            PlacementAddress::StackIndex(StackIndex(index)),
            TileSpawnKind::Atom { atom: atom.clone() },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::persistence::{export_project_bytes, import_project_bytes},
        application::pipeline::lowering::lower_tessera_ir,
    };
    use cadence::prelude::{CadenceCompiler, ControlKey, ControlValue, PreparedScore, Span, Time};
    use tessera::prelude::TesseraCompiler;

    fn preview(project: &MusaicProject, cycle: i64) -> Vec<(Time, Time, i64)> {
        let program = export_document_program(&project.document).expect("export example");
        let compiled = TesseraCompiler::new()
            .compile_authored(&program)
            .expect("compile authored example");
        let (scores, diagnostics) = lower_tessera_ir(&compiled.ir);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(scores.len(), 1);
        let start = Time::new(cycle, 1);
        let window = Span::new(start, start + Time::ONE).unwrap();
        let prepared = PreparedScore::new(scores.values().next().unwrap().clone())
            .expect("prepare example score");
        let report = CadenceCompiler::new()
            .preview(&prepared, &window)
            .expect("preview example");
        report
            .starts()
            .map(|moment| {
                let projected = moment.projected();
                let Some(ControlValue::Scalar(pitch)) =
                    projected.controls().get(&ControlKey::Pitch)
                else {
                    panic!("example note has chromatic pitch");
                };
                (
                    projected.whole().start(),
                    projected.whole().end(),
                    *pitch as i64,
                )
            })
            .collect()
    }

    #[test]
    fn example_compiles_to_twelve_onsets_and_survives_project_bytes() {
        let project = MusaicProject::demo();
        assert_eq!(project.metadata.display_name, "Small changes");
        assert_eq!(project.document.playback.bpm, 120.0);
        assert_eq!(project.document.playback.beats_per_cycle, 4);
        let original = preview(&project, 0);
        assert_eq!(
            original.len(),
            12,
            "nested pattern should stay moderate: {original:?}"
        );
        assert!(
            original.iter().any(|(_, _, pitch)| *pitch == 61),
            "C sharp is audible"
        );
        assert!(
            original.iter().any(|(_, _, pitch)| *pitch == 64),
            "E is audible"
        );
        let bytes = export_project_bytes(&project).expect("save editable example");
        let reopened = import_project_bytes(&bytes, None).expect("reopen editable example");
        assert_eq!(reopened.document.graph, project.document.graph);
        assert_eq!(preview(&reopened, 0), original);
        assert_eq!(preview(&reopened, 7), preview(&project, 7));
    }
}
