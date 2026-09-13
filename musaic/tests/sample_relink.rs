//! Sample tuning and relinking through the same worker and history commands used
//! by the editor, including real audio and project-owned file roundtrips.
#![cfg(not(target_arch = "wasm32"))]
#[path = "support/acceptance.rs"]
mod support;

use bevy::prelude::*;
use musaic::{
    adapter::persistence::{export_project_bytes, load_project, save_project},
    application::{
        audio_export::render_project_wav,
        command::{EditorCommand, EditorCommandBus},
        history::CommandHistory,
        session::MusaicProject,
    },
    domain::project::samples::{SampleId, SampleImportOptions as Options},
    infrastructure::diagnostics::DiagnosticStore,
};
use support::{Folder, editor, notes, send, wait_for, wav};
use tessera::prelude::NodeId;

fn project(app: &App) -> &MusaicProject {
    app.world().resource()
}
fn fixture(folder: &Folder) -> (App, SampleId, NodeId) {
    let (initial, _source, output) = notes();
    let mut app = editor(initial);
    let path = folder.0.join("original.wav");
    std::fs::write(&path, wav(48_000, 48_000, 4000)).unwrap();
    send(
        &mut app,
        EditorCommand::ImportSample {
            path,
            sound: Some(output.clone()),
        },
    );
    wait_for(&mut app, |app| {
        !project(app).samples.manifest().samples.is_empty()
    });
    let sample = *project(&app)
        .samples
        .manifest()
        .samples
        .keys()
        .next()
        .unwrap();
    (app, sample, output)
}

#[test]
fn tuning_and_gain_gestures_are_one_undo_step_and_change_exported_audio() {
    let folder = Folder::new();
    let (mut app, sample, _) = fixture(&folder);
    let original = project(&app)
        .samples
        .decoded(sample)
        .unwrap()
        .frames()
        .to_vec();
    let baseline = render_project_wav(project(&app), 1).unwrap();
    let history = app.world().resource::<CommandHistory>().undo_len();
    let asset_revision = project(&app).samples.asset_revision(sample);
    send(&mut app, EditorCommand::BeginSampleOptionsEdit { sample });
    for pitch in [60.0, 65.5, 72.0] {
        send(
            &mut app,
            EditorCommand::SetSampleOptions {
                sample,
                options: Options {
                    sustain_loop: None,
                    root_pitch: Some(pitch),
                    default_gain: Some(0.5),
                },
            },
        );
    }
    send(&mut app, EditorCommand::EndSampleOptionsEdit);
    assert_eq!(
        app.world().resource::<CommandHistory>().undo_len(),
        history + 1
    );
    assert_eq!(
        project(&app).samples.asset_revision(sample),
        asset_revision,
        "metadata must not invalidate an in-flight relink"
    );
    assert_eq!(
        project(&app).samples.decoded(sample).unwrap().frames(),
        original
    );
    let tuned = render_project_wav(project(&app), 1).unwrap();
    assert_ne!(tuned, baseline);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(
        project(&app).samples.manifest().samples[&sample].options,
        Options::default()
    );
    assert_eq!(render_project_wav(project(&app), 1).unwrap(), baseline);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(render_project_wav(project(&app), 1).unwrap(), tuned);
    send(
        &mut app,
        EditorCommand::SetSampleOptions {
            sample,
            options: Options {
                sustain_loop: None,
                root_pitch: Some(72.0),
                default_gain: Some(0.0),
            },
        },
    );
    assert!(
        render_project_wav(project(&app), 1).unwrap()[44..]
            .iter()
            .all(|byte| *byte == 0)
    );
    send(&mut app, EditorCommand::Undo);
    let path = folder.0.join("tuned/song.musaic.json");
    save_project(&path, project(&app)).unwrap();
    let reopened = load_project(path).unwrap();
    assert_eq!(
        reopened.samples.manifest(),
        project(&app).samples.manifest()
    );
    assert_eq!(render_project_wav(&reopened, 1).unwrap(), tuned);
}

#[test]
fn relink_keeps_identity_and_inflight_metadata_and_history_never_rereads_the_source() {
    let folder = Folder::new();
    let (mut app, sample, output) = fixture(&folder);
    let old = project(&app).samples.manifest().samples[&sample].clone();
    let old_frames = project(&app)
        .samples
        .decoded(sample)
        .unwrap()
        .frames()
        .to_vec();
    let asset_revision = project(&app).samples.asset_revision(sample);
    let history = app.world().resource::<CommandHistory>().undo_len();
    let replacement = folder.0.join("replacement.wav");
    std::fs::write(&replacement, wav(24_000, 24_000, 6000)).unwrap();
    // Both commands enter one dispatcher pass; adoption is a later pass, even
    // if the decoder finishes instantly. Metadata edited during decode survives.
    for command in [
        EditorCommand::RelinkSample {
            sample,
            path: replacement.clone(),
        },
        EditorCommand::SetSampleOptions {
            sample,
            options: Options {
                sustain_loop: None,
                root_pitch: Some(60.0),
                default_gain: Some(0.25),
            },
        },
    ] {
        app.world_mut().write_message(EditorCommandBus(command));
    }
    app.update();
    wait_for(&mut app, |app| {
        project(app).samples.asset_revision(sample) > asset_revision
    });
    let relinked = project(&app).samples.manifest().samples[&sample].clone();
    assert_eq!(
        relinked.options,
        Options {
            sustain_loop: None,
            root_pitch: Some(60.0),
            default_gain: Some(0.25)
        }
    );
    assert_eq!(relinked.sample_rate, 24_000);
    assert_eq!(project(&app).samples.manifest().samples.len(), 1);
    assert_ne!(relinked.checksum, old.checksum);
    assert_eq!(
        project(&app).sound_definition(&output).unwrap().source,
        musaic::domain::instrument::InstrumentSource::Sample(sample)
    );
    assert_eq!(
        app.world().resource::<CommandHistory>().undo_len(),
        history + 2
    );
    let relinked_audio = render_project_wav(project(&app), 1).unwrap();
    std::fs::remove_file(replacement).unwrap();
    send(&mut app, EditorCommand::Undo);
    assert_eq!(
        project(&app).samples.decoded(sample).unwrap().frames(),
        old_frames
    );
    assert_eq!(
        project(&app).samples.manifest().samples[&sample].options,
        relinked.options
    );
    assert_ne!(
        render_project_wav(project(&app), 1).unwrap(),
        relinked_audio
    );
    send(&mut app, EditorCommand::Redo);
    assert_eq!(project(&app).samples.manifest().samples[&sample], relinked);
    assert_eq!(
        render_project_wav(project(&app), 1).unwrap(),
        relinked_audio
    );
    let path = folder.0.join("owned/song.musaic.json");
    save_project(&path, project(&app)).unwrap();
    let reopened = load_project(path).unwrap();
    assert_eq!(reopened.samples.manifest().samples[&sample], relinked);
    assert_eq!(render_project_wav(&reopened, 1).unwrap(), relinked_audio);
}

#[test]
fn missing_asset_repair_undo_and_redo_restore_both_audio_and_diagnostics() {
    let folder = Folder::new();
    let (mut app, sample, _) = fixture(&folder);
    let path = folder.0.join("damaged/song.musaic.json");
    save_project(&path, project(&app)).unwrap();
    let relative = project(&app).samples.manifest().samples[&sample]
        .relative_path
        .clone();
    std::fs::remove_file(path.parent().unwrap().join(relative)).unwrap();
    let missing = load_project(&path).unwrap();
    let diagnostics = missing.samples.diagnostics().to_vec();
    assert_eq!(diagnostics.len(), 1);
    send(
        &mut app,
        EditorCommand::AdoptProject {
            project: missing,
            path: Some(path),
        },
    );
    assert!(render_project_wav(project(&app), 1).is_err());
    let revision = project(&app).samples.asset_revision(sample);
    send(
        &mut app,
        EditorCommand::RelinkSample {
            sample,
            path: folder.0.join("original.wav"),
        },
    );
    wait_for(&mut app, |app| {
        project(app).samples.asset_revision(sample) > revision
    });
    assert!(project(&app).samples.diagnostics().is_empty());
    let repaired = render_project_wav(project(&app), 1).unwrap();
    send(&mut app, EditorCommand::Undo);
    assert!(project(&app).samples.decoded(sample).is_none());
    assert_eq!(project(&app).samples.diagnostics(), diagnostics);
    assert!(render_project_wav(project(&app), 1).is_err());
    send(&mut app, EditorCommand::Redo);
    assert!(project(&app).samples.diagnostics().is_empty());
    assert_eq!(render_project_wav(project(&app), 1).unwrap(), repaired);
}

#[test]
fn invalid_metadata_and_failed_decode_are_atomic_and_keep_the_previous_sound() {
    let folder = Folder::new();
    let (mut app, sample, _) = fixture(&folder);
    let before = export_project_bytes(project(&app)).unwrap();
    let audio = render_project_wav(project(&app), 1).unwrap();
    let history = app.world().resource::<CommandHistory>().undo_len();
    let revision = project(&app).samples.revision();
    for options in [
        Options {
            sustain_loop: None,
            root_pitch: Some(-1.0),
            ..Default::default()
        },
        Options {
            sustain_loop: None,
            root_pitch: Some(128.0),
            ..Default::default()
        },
        Options {
            sustain_loop: None,
            root_pitch: Some(f64::NAN),
            ..Default::default()
        },
        Options {
            sustain_loop: None,
            default_gain: Some(-0.1),
            ..Default::default()
        },
        Options {
            sustain_loop: None,
            default_gain: Some(17.0),
            ..Default::default()
        },
        Options {
            sustain_loop: None,
            default_gain: Some(f32::INFINITY),
            ..Default::default()
        },
    ] {
        send(
            &mut app,
            EditorCommand::SetSampleOptions { sample, options },
        );
        assert_eq!(export_project_bytes(project(&app)).unwrap(), before);
        assert_eq!(project(&app).samples.revision(), revision);
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), history);
    }
    let malformed = folder.0.join("broken.wav");
    std::fs::write(&malformed, b"not a wave file").unwrap();
    send(
        &mut app,
        EditorCommand::RelinkSample {
            sample,
            path: malformed,
        },
    );
    wait_for(&mut app, |app| {
        format!("{:?}", app.world().resource::<DiagnosticStore>()).contains("Could not load sound")
    });
    assert_eq!(export_project_bytes(project(&app)).unwrap(), before);
    assert_eq!(project(&app).samples.revision(), revision);
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), history);
    assert_eq!(render_project_wav(project(&app), 1).unwrap(), audio);
}
