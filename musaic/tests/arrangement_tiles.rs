//! A song's visible arrangement and ending-speed tiles are its actual sound source.
#[path = "support/acceptance.rs"]
mod support;

use bevy::prelude::*;
use cadence::prelude::{CadenceCompiler, ControlKey, ControlValue, PreparedScore, Span, Time};
use musaic::{
    MusaicProject,
    adapter::persistence::{load_project, save_project},
    application::{
        audio_export::render_project_wav,
        command::EditorCommand,
        compile::compile_project_ir,
        pipeline::{lowering::lower_project_ir, runtime::RuntimeState},
    },
    domain::{
        board::{BoardSlot, BoardSurfaceId},
        document::{
            AtomValue, ContainerKind, NoteName, PlacementAddress, StackIndex, TileSpawnKind,
            bind_tiles, export_document_program,
        },
        instrument::InstrumentDefinition,
    },
};
use tessera::prelude::{AtomModifier, NodeId, PatternIr, Rational, SpatialSide};

fn insert(
    project: &mut MusaicProject,
    surface: BoardSurfaceId,
    address: PlacementAddress,
    kind: TileSpawnKind,
) -> NodeId {
    project
        .document
        .graph
        .insert_tile(&mut project.document.surfaces, surface, address, kind)
        .unwrap()
}
fn child(
    project: &mut MusaicProject,
    surface: BoardSurfaceId,
    index: usize,
    kind: ContainerKind,
) -> BoardSurfaceId {
    let node = insert(
        project,
        surface,
        PlacementAddress::StackIndex(StackIndex(index)),
        TileSpawnKind::Container { kind },
    );
    project.document.graph.container_surface(&node).unwrap()
}
fn atom(
    project: &mut MusaicProject,
    surface: BoardSurfaceId,
    index: usize,
    atom: AtomValue,
) -> NodeId {
    insert(
        project,
        surface,
        PlacementAddress::StackIndex(StackIndex(index)),
        TileSpawnKind::Atom { atom },
    )
}
fn fixture(ending_modifier: AtomModifier) -> (MusaicProject, NodeId) {
    let mut project = MusaicProject::new_empty();
    project.document.playback.bpm = 240.0;
    let root = project.document.root_surface;
    let song = insert(
        &mut project,
        root,
        PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
        TileSpawnKind::Container {
            kind: ContainerKind::Arrangement,
        },
    );
    let arrangement = project.document.graph.container_surface(&song).unwrap();
    let verse = child(&mut project, arrangement, 0, ContainerKind::Alternating);
    atom(&mut project, verse, 0, AtomValue::NoteName(NoteName::C));
    atom(&mut project, verse, 1, AtomValue::NoteName(NoteName::D));
    atom(
        &mut project,
        arrangement,
        1,
        AtomValue::Modifier(AtomModifier::Elongate(Rational::from_integer(2))),
    );
    let ending = child(&mut project, arrangement, 2, ContainerKind::Sequence);
    atom(&mut project, ending, 0, AtomValue::NoteName(NoteName::E));
    atom(&mut project, ending, 1, AtomValue::Rest);
    let speed = atom(
        &mut project,
        arrangement,
        3,
        AtomValue::Modifier(ending_modifier),
    );
    atom(
        &mut project,
        arrangement,
        4,
        AtomValue::Modifier(AtomModifier::Elongate(Rational::from_integer(2))),
    );
    let instrument = insert(
        &mut project,
        root,
        PlacementAddress::BoardSlot(BoardSlot::new(5, 0)),
        TileSpawnKind::sound(InstrumentDefinition::default()),
    );
    let output = insert(
        &mut project,
        root,
        PlacementAddress::BoardSlot(BoardSlot::new(6, 0)),
        TileSpawnKind::Output {
            name: "Song".into(),
        },
    );
    let mut program = export_document_program(&project.document).unwrap();
    for (from, to) in [(&song, &instrument), (&instrument, &output)] {
        bind_tiles(&mut program, from, to, SpatialSide::East).unwrap();
    }
    project.document.replace_connections_from(&program);
    (project, speed)
}
fn notes(project: &MusaicProject, ir: &PatternIr, start: i64, end: i64) -> Vec<(Time, Time, i64)> {
    let (scores, diagnostics, _) = lower_project_ir(project, ir);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(scores.len(), 1);
    let prepared = PreparedScore::new(scores.values().next().unwrap().clone()).unwrap();
    CadenceCompiler::new()
        .preview(
            &prepared,
            &Span::new(Time::new(start, 1), Time::new(end, 1)).unwrap(),
        )
        .unwrap()
        .starts()
        .map(|event| {
            let projected = event.projected();
            let Some(ControlValue::Scalar(pitch)) = projected.controls().get(&ControlKey::Pitch)
            else {
                panic!("pitch");
            };
            (
                projected.whole().start(),
                projected.whole().end(),
                *pitch as i64,
            )
        })
        .collect()
}
fn live_notes(app: &App, start: i64, end: i64) -> Vec<(Time, Time, i64)> {
    let project = app.world().resource::<MusaicProject>();
    let live = &app
        .world()
        .resource::<RuntimeState>()
        .compiled
        .as_ref()
        .expect("live compilation")
        .tessera_ir;
    let result = notes(project, live, start, end);
    assert_eq!(
        result,
        notes(project, &compile_project_ir(project).unwrap(), start, end)
    );
    result
}
#[test]
fn ending_fast_tile_changes_live_notes_audio_and_saved_song() {
    let (project, speed) = fixture(AtomModifier::Fast(Rational::from_integer(2)));
    let mut app = support::editor(project);
    // Give the normal editor compile/lower pipeline its next scheduled frame.
    app.update();
    let verse = live_notes(&app, 0, 2);
    assert_eq!(
        verse,
        vec![
            (Time::ZERO, Time::ONE, 60),
            (Time::ONE, Time::new(2, 1), 62)
        ]
    );
    let before = live_notes(&app, 2, 4);
    assert_eq!(before.len(), 4, "two ending hits per cycle, for two cycles");
    let baseline_audio = render_project_wav(app.world().resource::<MusaicProject>(), 4).unwrap();
    support::send(
        &mut app,
        EditorCommand::SetAtomValue {
            node: speed,
            value: AtomValue::Modifier(AtomModifier::Fast(Rational::from_integer(4))),
        },
    );
    app.update();
    let after = live_notes(&app, 2, 4);
    assert_eq!(
        after.len(),
        8,
        "editing the visible Fast tile doubles ending hits"
    );
    assert_eq!(
        live_notes(&app, 0, 2),
        verse,
        "the verse and section boundary retain their clocks"
    );
    let expected: Vec<_> = (0..8)
        .map(|i| {
            (
                Time::new(2, 1) + Time::new(i, 4),
                Time::new(2, 1) + Time::new(i * 2 + 1, 8),
                64,
            )
        })
        .collect();
    assert_eq!(after, expected);
    let edited_audio = render_project_wav(app.world().resource::<MusaicProject>(), 4).unwrap();
    assert_ne!(
        baseline_audio, edited_audio,
        "real output audio must change with ending speed"
    );
    let late = live_notes(&app, 42, 44);
    assert_eq!(
        late,
        after
            .iter()
            .map(|(start, end, pitch)| (*start + Time::new(40, 1), *end + Time::new(40, 1), *pitch))
            .collect::<Vec<_>>()
    );
    let folder = support::Folder::new();
    let path = folder.0.join("visible-arrangement.musaic.json");
    save_project(&path, app.world().resource::<MusaicProject>()).unwrap();
    let reopened = load_project(&path).unwrap();
    assert_eq!(
        notes(&reopened, &compile_project_ir(&reopened).unwrap(), 2, 4),
        after
    );
    assert_eq!(render_project_wav(&reopened, 4).unwrap(), edited_audio);
    support::send(&mut app, EditorCommand::Undo);
    app.update();
    assert_eq!(live_notes(&app, 2, 4), before);
    support::send(&mut app, EditorCommand::Redo);
    app.update();
    assert_eq!(live_notes(&app, 2, 4), after);
}

#[test]
fn owned_reverse_pattern_survives_document_round_trip_and_reverses_note_timing() {
    let (project, _) = fixture(AtomModifier::Rev);
    let mut app = support::editor(project);
    app.update();
    let expected = vec![
        (Time::new(5, 2), Time::new(3, 1), 64),
        (Time::new(7, 2), Time::new(4, 1), 64),
    ];
    assert_eq!(live_notes(&app, 2, 4), expected);
    let folder = support::Folder::new();
    let path = folder.0.join("nested-reverse.musaic.json");
    save_project(&path, app.world().resource::<MusaicProject>()).unwrap();
    let reopened = load_project(path).unwrap();
    assert_eq!(
        notes(&reopened, &compile_project_ir(&reopened).unwrap(), 2, 4),
        expected
    );
}
