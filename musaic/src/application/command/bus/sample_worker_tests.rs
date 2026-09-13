//! Deterministically deliver worker results across real command transitions.
//! No wall-clock race or large-file decoding is needed to force stale completion.
use super::*;
use crate::{
    adapter::persistence::{export_project_bytes, import_wav_bytes, prepare_wav_bytes},
    application::{editor::EditorPlugin, pipeline::PlaybackPlugin},
    infrastructure::app::{AppState, MusaicSet, TransportMode},
};

fn wav(value: i16) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&44_u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8000_u32.to_le_bytes());
    bytes.extend_from_slice(&16000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&8_u32.to_le_bytes());
    for _ in 0..4 {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn project(value: i16) -> (MusaicProject, SampleId) {
    let mut project = MusaicProject::new_empty();
    let id = import_wav_bytes(&mut project, "initial.wav", wav(value), Default::default()).unwrap();
    (project, id)
}

fn app(project: MusaicProject) -> App {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins(MinimalPlugins)
        .add_plugins((EditorPlugin, PlaybackPlugin));
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
            cadence::bevy::CadenceSet::ReplaceScores.in_set(MusaicSet::Runtime),
            cadence::bevy::CadenceSet::Tick.in_set(MusaicSet::Runtime),
        ),
    );
    app.insert_state(AppState::Editor);
    app.insert_resource(project);
    app.update();
    app
}

fn deliver(app: &mut App, generation: u64, target: Target) {
    app.world()
        .resource::<SampleImports>()
        .sender
        .send(Completion {
            generation,
            target,
            result: Ok(
                prepare_wav_bytes("replacement.wav", wav(2000), Default::default())
                    .unwrap()
                    .into(),
            ),
        })
        .unwrap();
}

#[test]
fn previous_project_results_cannot_replace_same_id_or_unlock_the_new_worker() {
    let (initial, sample) = project(1000);
    let mut app = app(initial);
    let generation = app.world().resource::<SampleImports>().generation;
    app.world_mut().resource_mut::<SampleImports>().busy = true;
    let (replacement, same_id) = project(3000);
    assert_eq!(same_id, sample);
    app.world_mut()
        .write_message(EditorCommandBus(EditorCommand::AdoptProject {
            project: replacement,
            path: None,
        }));
    app.update();
    assert!(
        !app.world().resource::<SampleImports>().busy,
        "obsolete decoding must not block the new project"
    );
    // Represent another decode now active in the new generation.
    app.world_mut().resource_mut::<SampleImports>().busy = true;
    let before = export_project_bytes(app.world().resource::<MusaicProject>()).unwrap();
    for target in [
        Target::Import { sound: None },
        Target::Relink {
            sample,
            revision: 1,
            path: PathBuf::from("unused.wav"),
        },
    ] {
        deliver(&mut app, generation, target);
    }
    app.update();
    assert_eq!(
        export_project_bytes(app.world().resource::<MusaicProject>()).unwrap(),
        before
    );
    assert!(
        app.world().resource::<SampleImports>().busy,
        "an old completion must not unlock a current decode"
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
}

#[test]
fn undo_invalidates_a_decode_started_against_the_replaced_asset() {
    let (initial, sample) = project(1000);
    let mut app = app(initial);
    let generation = app.world().resource::<SampleImports>().generation;
    let revision = app
        .world()
        .resource::<MusaicProject>()
        .samples
        .asset_revision(sample);
    deliver(
        &mut app,
        generation,
        Target::Relink {
            sample,
            revision,
            path: PathBuf::from("first.wav"),
        },
    );
    app.update();
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
    let stale_revision = app
        .world()
        .resource::<MusaicProject>()
        .samples
        .asset_revision(sample);
    app.world_mut()
        .write_message(EditorCommandBus(EditorCommand::Undo));
    app.update();
    let before = export_project_bytes(app.world().resource::<MusaicProject>()).unwrap();
    deliver(
        &mut app,
        generation,
        Target::Relink {
            sample,
            revision: stale_revision,
            path: PathBuf::from("stale.wav"),
        },
    );
    app.update();
    assert_eq!(
        export_project_bytes(app.world().resource::<MusaicProject>()).unwrap(),
        before
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
    app.world_mut()
        .write_message(EditorCommandBus(EditorCommand::Redo));
    app.update();
    let frame = app
        .world()
        .resource::<MusaicProject>()
        .samples
        .decoded(sample)
        .unwrap()
        .frames()[0];
    assert!((frame.left - 2000.0 / 32768.0).abs() < 0.00001);
}
