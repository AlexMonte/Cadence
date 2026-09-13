//! Deliberate disconnection updates accepted playback and exported audio through
//! the current pattern -> processing -> instrument -> output route.
#![cfg(not(target_arch = "wasm32"))]

#[path = "support/acceptance.rs"]
mod acceptance;

use bevy::prelude::*;
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
    bevy::{ActiveScores, CadencePlugin, CadenceSet, PlaybackSync},
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    prelude::Time as CycleTime,
};
use musaic::{
    MusaicProject,
    adapter::{
        audio::project_samples::load_project_samples,
        persistence::{import_wav_bytes, load_project, save_project},
    },
    application::{
        audio_export::render_project_wav,
        command::{EditorCommand, EditorCommandBus},
        compile::compile_project_ir,
        editor::{EditorPlugin, PlacementTarget},
        pipeline::{
            lowering::register_lowering,
            runtime::{RuntimeState, register_runtime},
        },
    },
    domain::{
        board::BoardSlot,
        document::{
            AtomValue, ContainerKind, GraphTilePrototypeId, MusaicDocument, NoteName,
            OperatorValue, PlacementAddress, StackIndex, TileSpawnKind, bind_tiles,
            export_document_program,
        },
        instrument::{InstrumentDefinition, InstrumentSource},
        project::samples::SampleImportOptions,
    },
    infrastructure::app::{AppState, MusaicSet, TransportMode},
};
use tessera::prelude::{
    InputEndpoint, NodeId, OutputEndpoint, OutputPort, PortGroupId, PortMemberId, RootRelation,
    SpatialSide, StreamSource, StreamTarget,
};

#[derive(Clone)]
struct Fixture {
    project: MusaicProject,
    left_pattern: NodeId,
    right_pattern: NodeId,
    left_output: NodeId,
    right_output: NodeId,
}

fn constant_wav(sample: i16) -> Vec<u8> {
    let length = 16_000_u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_000_u32.to_le_bytes());
    bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&length.to_le_bytes());
    for _ in 0..8_000 {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn insert(document: &mut MusaicDocument, slot: BoardSlot, tile: TileSpawnKind) -> NodeId {
    document
        .graph
        .insert_tile(
            &mut document.surfaces,
            document.root_surface,
            PlacementAddress::BoardSlot(slot),
            tile,
        )
        .unwrap()
}

fn fixture() -> Fixture {
    let mut project = MusaicProject::new_empty();
    project.document.playback.bpm = 60.0;
    project.document.playback.beats_per_cycle = 1;
    let mut routes = Vec::new();
    let mut patterns = Vec::new();
    let mut instruments = Vec::new();
    let mut outputs = Vec::new();

    for (row, name, pan) in [(0, "left", -1), (4, "right", 1)] {
        let pattern = insert(
            &mut project.document,
            BoardSlot::new(0, row),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        );
        let surface = project.document.graph.container_surface(&pattern).unwrap();
        for (index, atom) in [AtomValue::NoteName(NoteName::C), AtomValue::Octave(4)]
            .into_iter()
            .enumerate()
        {
            project
                .document
                .graph
                .insert_tile(
                    &mut project.document.surfaces,
                    surface,
                    PlacementAddress::StackIndex(StackIndex(index)),
                    TileSpawnKind::Atom { atom },
                )
                .unwrap();
        }
        let pan_tile = insert(
            &mut project.document,
            BoardSlot::new(5, row),
            TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(15),
            },
        );
        let pan_value = insert(
            &mut project.document,
            BoardSlot::new(5, row - 1),
            TileSpawnKind::Atom {
                atom: AtomValue::Number(pan),
            },
        );
        let instrument = insert(
            &mut project.document,
            BoardSlot::new(6, row),
            TileSpawnKind::sound(InstrumentDefinition::default()),
        );
        let output = insert(
            &mut project.document,
            BoardSlot::new(7, row),
            TileSpawnKind::Output { name: name.into() },
        );
        routes.extend([
            (pattern.clone(), pan_tile.clone(), SpatialSide::East),
            (pan_value, pan_tile.clone(), SpatialSide::South),
            (pan_tile, instrument.clone(), SpatialSide::East),
            (instrument.clone(), output.clone(), SpatialSide::East),
        ]);
        patterns.push(pattern);
        instruments.push(instrument);
        outputs.push(output);
    }

    let mut program = export_document_program(&project.document).unwrap();
    for (from, to, side) in routes {
        bind_tiles(&mut program, &from, &to, side).unwrap();
    }
    project.document.replace_connections_from(&program);

    for (instrument, name, sample) in [
        (instruments[0].clone(), "left", 4096),
        (instruments[1].clone(), "right", -4096),
    ] {
        let sample = import_wav_bytes(
            &mut project,
            &format!("{name}.wav"),
            constant_wav(sample),
            SampleImportOptions {
                root_pitch: Some(60.0),
                ..default()
            },
        )
        .unwrap();
        project
            .set_sound_definition(
                &instrument,
                InstrumentDefinition::new(InstrumentSource::Sample(sample)),
            )
            .unwrap();
    }

    Fixture {
        project,
        left_pattern: patterns[0].clone(),
        right_pattern: patterns[1].clone(),
        left_output: outputs[0].clone(),
        right_output: outputs[1].clone(),
    }
}

fn editor(project: MusaicProject) -> (App, AudioRenderer) {
    let (audio, renderer) = AudioRenderer::split(AudioRendererSettings::new(8_000, 4096)).unwrap();
    let playback = PlaybackRuntime::new(
        PlaybackSettings {
            cps: CycleTime::ONE,
            ..default()
        },
        audio,
    );
    load_project_samples(&project, &playback.sample_bank());
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins(MinimalPlugins)
        .add_plugins((EditorPlugin, CadencePlugin));
    register_runtime(&mut app);
    register_lowering(&mut app);
    app.configure_sets(
        Update,
        (
            MusaicSet::Input,
            MusaicSet::Commands,
            MusaicSet::DocumentMutation,
            MusaicSet::Compile,
            MusaicSet::Lower,
            MusaicSet::Runtime,
            MusaicSet::SceneSync,
            MusaicSet::RenderUi,
        )
            .chain(),
    )
    .configure_sets(
        Update,
        (
            CadenceSet::ReplaceScores.in_set(MusaicSet::Runtime),
            CadenceSet::Tick.in_set(MusaicSet::Runtime),
        ),
    );
    app.insert_non_send_resource(playback)
        .insert_state(AppState::Editor)
        .insert_resource(project);
    for _ in 0..3 {
        app.update();
    }
    (app, renderer)
}

fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    for _ in 0..3 {
        app.update();
    }
}

fn toggle_source(app: &mut App, pattern: &NodeId) {
    send(
        app,
        EditorCommand::BindOutputSide {
            node: pattern.clone(),
            side: SpatialSide::East,
        },
    );
}

fn assert_outputs(app: &App, expected: &[&NodeId]) {
    let accepted = app
        .world()
        .resource::<RuntimeState>()
        .compiled
        .as_ref()
        .expect("accepted music");
    let actual = accepted
        .scores
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let expected = expected
        .iter()
        .map(|id| id.0.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(
        app.world().resource::<ActiveScores>().revision,
        app.world().resource::<PlaybackSync>().last_applied_revision
    );
}

fn render(app: &mut App, renderer: &mut AudioRenderer, count: usize) -> Vec<Frame> {
    let mut frames = vec![Frame::ZERO; count];
    for chunk in frames.chunks_mut(64) {
        app.update();
        renderer.render(chunk);
    }
    frames
}

fn assert_channels(frames: &[Frame], left: bool, right: bool) {
    let frames = &frames[frames.len() - 1000..];
    let energy = frames.iter().fold((0.0_f64, 0.0_f64), |(l, r), frame| {
        (
            l + f64::from(frame.left).powi(2),
            r + f64::from(frame.right).powi(2),
        )
    });
    if left {
        assert!(energy.0 > 1.0, "left should be audible: {energy:?}");
    } else {
        assert!(energy.0 < 1e-8, "left should be silent: {energy:?}");
    }
    if right {
        assert!(energy.1 > 1.0, "right should be audible: {energy:?}");
    } else {
        assert!(energy.1 < 1e-8, "right should be silent: {energy:?}");
    }
}

fn wav_frame(bytes: &[u8], index: usize) -> (i16, i16) {
    let offset = 44 + index * 4;
    (
        i16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap()),
        i16::from_le_bytes(bytes[offset + 2..offset + 4].try_into().unwrap()),
    )
}

#[test]
fn disconnect_export_save_and_undo_preserve_the_other_route() {
    let fixture = fixture();
    let original_graph = fixture.project.document.graph.clone();
    let original_connections = fixture.project.document.connections.clone();
    let baseline = render_project_wav(&fixture.project, 1).unwrap();
    let reference = wav_frame(&baseline, 1200);
    assert!(reference.0 > 1000 && reference.1 < -1000, "{reference:?}");

    let (mut app, _) = editor(fixture.project.clone());
    assert_outputs(&app, &[&fixture.left_output, &fixture.right_output]);
    toggle_source(&mut app, &fixture.left_pattern);
    assert_outputs(&app, &[&fixture.right_output]);

    let disconnected = app.world().resource::<MusaicProject>();
    let disconnected_connections = disconnected.document.connections.clone();
    assert_ne!(disconnected_connections, original_connections);
    assert_eq!(disconnected.document.graph, original_graph);
    let ir = compile_project_ir(disconnected).unwrap();
    assert_eq!(
        ir.outputs
            .iter()
            .map(|output| output.id.clone())
            .collect::<Vec<_>>(),
        vec![fixture.right_output.clone()]
    );
    let isolated = render_project_wav(disconnected, 1).unwrap();
    assert_eq!(wav_frame(&isolated, 1200), (0, reference.1));

    let folder = acceptance::Folder::new();
    let path = folder.0.join("disconnected.musaic.json");
    save_project(&path, disconnected).unwrap();
    let reopened = load_project(&path).unwrap();
    assert_eq!(reopened.document.graph, original_graph);
    assert_eq!(reopened.document.connections, disconnected_connections);
    assert_eq!(render_project_wav(&reopened, 1).unwrap(), isolated);

    send(&mut app, EditorCommand::Undo);
    assert_outputs(&app, &[&fixture.left_output, &fixture.right_output]);
    assert_eq!(
        render_project_wav(app.world().resource::<MusaicProject>(), 1).unwrap(),
        baseline
    );
    send(&mut app, EditorCommand::Redo);
    assert_outputs(&app, &[&fixture.right_output]);
    toggle_source(&mut app, &fixture.left_pattern);
    assert_outputs(&app, &[&fixture.left_output, &fixture.right_output]);
}

#[test]
fn live_disconnect_reconnect_and_undo_change_real_pcm_with_a_silent_all_off_export() {
    let fixture = fixture();
    let (mut app, mut renderer) = editor(fixture.project);
    send(&mut app, EditorCommand::TransportPlay);
    assert_channels(&render(&mut app, &mut renderer, 2_000), true, true);

    toggle_source(&mut app, &fixture.left_pattern);
    assert_outputs(&app, &[&fixture.right_output]);
    assert_channels(&render(&mut app, &mut renderer, 18_000), false, true);

    toggle_source(&mut app, &fixture.left_pattern);
    assert_outputs(&app, &[&fixture.left_output, &fixture.right_output]);
    assert_channels(&render(&mut app, &mut renderer, 18_000), true, true);

    toggle_source(&mut app, &fixture.left_pattern);
    toggle_source(&mut app, &fixture.right_pattern);
    assert_outputs(&app, &[]);
    assert_channels(&render(&mut app, &mut renderer, 18_000), false, false);
    let silent = render_project_wav(app.world().resource::<MusaicProject>(), 2).unwrap();
    assert_eq!(silent.len(), 44 + 2 * 48_000 * 4);
    assert!(silent[44..].iter().all(|byte| *byte == 0));

    send(&mut app, EditorCommand::Undo);
    assert_outputs(&app, &[&fixture.right_output]);
    assert_channels(&render(&mut app, &mut renderer, 18_000), false, true);
}

#[test]
fn failed_current_compile_preserves_the_last_accepted_playback() {
    let fixture = fixture();
    let (mut app, mut renderer) = editor(fixture.project);
    toggle_source(&mut app, &fixture.right_pattern);
    assert_outputs(&app, &[&fixture.left_output]);
    send(&mut app, EditorCommand::TransportPlay);
    assert_channels(&render(&mut app, &mut renderer, 2_000), true, false);
    let previous_revision = app.world().resource::<PlaybackSync>().last_applied_revision;
    let surface = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .container_surface(&fixture.left_pattern)
        .unwrap();
    send(
        &mut app,
        EditorCommand::EnterContainer {
            container: fixture.left_pattern.clone(),
        },
    );
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(2),
            },
            tile: TileSpawnKind::Atom {
                atom: AtomValue::Operator(OperatorValue::Multiply),
            },
        },
    );
    let project = app.world().resource::<MusaicProject>();
    assert!(compile_project_ir(project).is_err());
    assert!(render_project_wav(project, 1).is_err());
    assert_eq!(
        app.world().resource::<PlaybackSync>().last_applied_revision,
        previous_revision
    );
    assert_outputs(&app, &[&fixture.left_output]);
    assert_channels(&render(&mut app, &mut renderer, 18_000), true, false);
}

#[test]
fn an_incompatible_connected_source_is_not_mistaken_for_an_inactive_output() {
    let mut project = MusaicProject::new_empty();
    let number = insert(
        &mut project.document,
        BoardSlot::new(0, 0),
        TileSpawnKind::Atom {
            atom: AtomValue::Number(1),
        },
    );
    let invalid = insert(
        &mut project.document,
        BoardSlot::new(1, 0),
        TileSpawnKind::Output {
            name: "invalid".into(),
        },
    );
    insert(
        &mut project.document,
        BoardSlot::new(8, 0),
        TileSpawnKind::Output {
            name: "inactive".into(),
        },
    );
    let mut program = export_document_program(&project.document).unwrap();
    program
        .root_surface
        .explicit_relations
        .push(RootRelation::FlowsTo {
            from: StreamSource {
                node: number,
                endpoint: OutputEndpoint::Socket(OutputPort::new("out")),
            },
            to: StreamTarget::OutputInput {
                node: invalid,
                endpoint: InputEndpoint::GroupMember {
                    group: PortGroupId::new("inputs"),
                    member: PortMemberId::new("main"),
                },
            },
        });
    project.document.replace_connections_from(&program);

    assert!(musaic::application::compile::has_inactive_outputs(&project));
    assert!(compile_project_ir(&project).is_err());
    assert!(render_project_wav(&project, 1).is_err());
    assert!(render_project_wav(&MusaicProject::new_empty(), 1).is_err());
}
