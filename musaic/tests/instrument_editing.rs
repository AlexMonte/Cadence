//! Sound nodes use the command, undo, language, and file boundaries.
use bevy::prelude::*;
use cadence::prelude::{
    BuiltInSynthSource, CadenceCompiler, ControlKey, ControlValue, Intent, PreparedScore, Span,
    Time,
};
use musaic::{
    adapter::persistence::{export_project_bytes, import_project_bytes},
    application::{
        command::{EditorCommand, EditorCommandBus},
        editor::EditorPlugin,
        history::CommandHistory,
        pipeline::{PlaybackPlugin, lowering::lower_tessera_ir_with_sounds},
        session::MusaicProject,
    },
    domain::{
        document::{DocumentNodeKind, export_document_program},
        instrument::{InstrumentDefinition, InstrumentSource, Waveform},
    },
    infrastructure::{
        app::{AppState, TransportMode},
        diagnostics::DiagnosticStore,
    },
};
use tessera::prelude::{NodeId, TesseraCompiler};

fn editor() -> (App, NodeId) {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins(MinimalPlugins)
        .add_plugins((EditorPlugin, PlaybackPlugin));
    app.insert_state(AppState::Editor);
    let project = MusaicProject::demo();
    let instrument = project
        .document
        .graph
        .nodes_on_surface(project.document.root_surface)
        .into_iter()
        .find(|(_, node)| matches!(&node.kind, DocumentNodeKind::Sound(_)))
        .unwrap()
        .1
        .id
        .clone();
    app.insert_resource(project);
    (app, instrument)
}

fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    app.update();
}

fn events(project: &MusaicProject) -> Vec<(Time, i64, Intent)> {
    let program = export_document_program(&project.document).unwrap();
    let compiled = TesseraCompiler::new().compile_authored(&program).unwrap();
    let bindings = project
        .document
        .graph
        .nodes()
        .filter_map(|node| match &node.kind {
            DocumentNodeKind::Sound(sound) => {
                Some((node.id.clone(), sound.definition.intent().unwrap()))
            }
            _ => None,
        })
        .collect();
    let (scores, diagnostics) = lower_tessera_ir_with_sounds(&compiled.ir, &bindings);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let prepared = PreparedScore::new(scores.values().next().unwrap().clone()).unwrap();
    let report = CadenceCompiler::new()
        .preview(&prepared, &Span::new(Time::ZERO, Time::ONE).unwrap())
        .unwrap();
    report
        .starts()
        .map(|event| {
            let event = event.projected();
            let Some(ControlValue::Scalar(pitch)) = event.controls().get(&ControlKey::Pitch) else {
                panic!("missing pitch")
            };
            (event.whole().start(), *pitch as i64, event.intent().clone())
        })
        .collect()
}

#[test]
fn sound_assignment_changes_source_keeps_notes_and_survives_undo_and_file_roundtrip() {
    let (mut app, output) = editor();
    let before = events(app.world().resource::<MusaicProject>());
    let sound = InstrumentDefinition::new(InstrumentSource::Synth(Waveform::Triangle));
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: sound.clone(),
        },
    );
    let project = app.world().resource::<MusaicProject>();
    let after = events(project);
    assert_eq!(
        before
            .iter()
            .map(|(start, pitch, _)| (*start, *pitch))
            .collect::<Vec<_>>(),
        after
            .iter()
            .map(|(start, pitch, _)| (*start, *pitch))
            .collect::<Vec<_>>()
    );
    assert!(after.iter().all(|(_, _, intent)| matches!(intent, Intent::Synth(source) if source.source == BuiltInSynthSource::Triangle)));
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
    let reopened = import_project_bytes(&export_project_bytes(project).unwrap(), None).unwrap();
    assert_eq!(after, events(&reopened));
    send(&mut app, EditorCommand::Undo);
    assert_eq!(before, events(app.world().resource::<MusaicProject>()));
    send(&mut app, EditorCommand::Redo);
    assert_eq!(after, events(app.world().resource::<MusaicProject>()));
}

#[test]
fn failed_open_preserves_music_unsaved_changes_and_undo_history() {
    let (mut app, output) = editor();
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output,
            definition: InstrumentDefinition::new(InstrumentSource::Synth(Waveform::Saw)),
        },
    );
    let before = export_project_bytes(app.world().resource::<MusaicProject>()).unwrap();
    send(
        &mut app,
        EditorCommand::OpenProject {
            path: std::env::temp_dir().join(format!(
                "musaic-nonexistent-{}-project.json",
                std::process::id()
            )),
        },
    );
    // A successful preview refresh must not erase an unrelated file-open error.
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(
        before,
        export_project_bytes(app.world().resource::<MusaicProject>()).unwrap()
    );
    assert!(app.world().resource::<MusaicProject>().metadata.dirty);
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
    assert!(
        format!("{:?}", app.world().resource::<DiagnosticStore>())
            .contains("current project is still open")
    );
}

#[test]
fn setting_a_missing_sound_node_is_rejected_without_history() {
    let (mut app, _) = editor();
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: NodeId::new("missing"),
            definition: InstrumentDefinition::default(),
        },
    );
    assert!(
        app.world()
            .resource::<MusaicProject>()
            .sound_definition(&NodeId::new("missing"))
            .is_none()
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
}

fn designed_sound() -> InstrumentDefinition {
    use musaic::domain::instrument::{
        InstrumentDelay, InstrumentEffect, InstrumentEnvelope, InstrumentReverb,
    };
    use tessera::prelude::Rational as R;
    let mut sound = InstrumentDefinition::new(InstrumentSource::Synth(Waveform::Saw));
    sound.sound.envelope = Some(InstrumentEnvelope {
        attack: R::new(1, 50),
        decay: R::new(1, 10),
        sustain: R::new(3, 5),
        release: R::new(1, 8),
    });
    sound.sound.gain = Some(R::new(1, 4));
    sound.sound.delay = Some(InstrumentDelay::default());
    sound.sound.reverb = Some(InstrumentReverb::default());
    sound.sound.compressor = Some(musaic::domain::instrument::InstrumentCompressor::default());
    sound.sound.effects = vec![
        InstrumentEffect::LowPass {
            cutoff_hz: R::new(500, 1),
            resonance: R::new(1, 5),
        },
        InstrumentEffect::Drive {
            amount: R::new(4, 5),
            wet: R::one(),
            output_gain: R::new(1, 2),
        },
        InstrumentEffect::HighPass {
            cutoff_hz: R::new(60, 1),
            resonance: R::zero(),
        },
    ];
    sound
}

#[test]
fn envelope_sends_and_effect_order_survive_edit_undo_save_reopen_and_change_export() {
    use musaic::{
        adapter::persistence::{load_project, save_project},
        application::audio_export::render_project_wav,
    };
    let (mut app, output) = editor();
    app.world_mut()
        .resource_mut::<MusaicProject>()
        .document
        .playback
        .bpm = 240.0;
    app.world_mut()
        .resource_mut::<MusaicProject>()
        .document
        .playback
        .beats_per_cycle = 1;
    let sound = designed_sound();
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: sound.clone(),
        },
    );
    let first = render_project_wav(app.world().resource::<MusaicProject>(), 2).unwrap();
    assert!(first[44..].iter().any(|byte| *byte != 0));
    let mut reordered = sound.clone();
    reordered.sound.effects.swap(0, 1);
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: reordered.clone(),
        },
    );
    let second = render_project_wav(app.world().resource::<MusaicProject>(), 2).unwrap();
    let distance: f64 = first[44..]
        .chunks_exact(2)
        .zip(second[44..].chunks_exact(2))
        .map(|(a, b)| {
            let a = i16::from_le_bytes(a.try_into().unwrap()) as f64;
            let b = i16::from_le_bytes(b.try_into().unwrap()) as f64;
            (a - b).powi(2)
        })
        .sum();
    assert!(
        distance > 100_000.0,
        "processing order must change the rendered sound"
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .sound_definition(&output)
            .unwrap(),
        &sound
    );
    assert_eq!(
        render_project_wav(app.world().resource::<MusaicProject>(), 2).unwrap(),
        first
    );
    send(&mut app, EditorCommand::Redo);
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .sound_definition(&output)
            .unwrap(),
        &reordered
    );
    let directory = std::env::temp_dir().join(format!(
        "musaic-sound-design-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("instrument.musaic.json");
    save_project(&path, app.world().resource::<MusaicProject>()).unwrap();
    let reopened = load_project(&path).unwrap();
    assert_eq!(reopened.sound_definition(&output).unwrap(), &reordered);
    assert_eq!(render_project_wav(&reopened, 2).unwrap(), second);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn invalid_sound_settings_are_rejected_atomically() {
    use musaic::domain::instrument::InstrumentEffect;
    use tessera::prelude::Rational as R;
    let (mut app, output) = editor();
    let original = app
        .world()
        .resource::<MusaicProject>()
        .sound_definition(&output)
        .unwrap()
        .clone();
    let mut invalid = designed_sound();
    invalid.sound.effects.push(InstrumentEffect::LowPass {
        cutoff_hz: R::new(1, 1),
        resonance: R::zero(),
    });
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: invalid,
        },
    );
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .sound_definition(&output)
            .unwrap(),
        &original
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
    let mut invalid = designed_sound();
    invalid.sound.delay.as_mut().unwrap().time = R::zero();
    assert!(invalid.validate().is_err());
    invalid = designed_sound();
    invalid.sound.envelope.as_mut().unwrap().sustain = R::new(2, 1);
    assert!(invalid.intent().is_err());
    let mut malformed = serde_json::to_value(designed_sound()).unwrap();
    malformed["sound"]["envelope"]["attack"]["denominator"] = serde_json::json!(0);
    let malformed = serde_json::from_value::<InstrumentDefinition>(malformed).unwrap();
    assert!(malformed.validate().is_err());
}

#[test]
fn reusable_sound_snapshots_apply_rename_delete_undo_and_reopen_without_changing_outputs() {
    use musaic::application::command::sound_library::SoundLibraryCommand as P;
    let (mut app, output) = editor();
    let designed = designed_sound();
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: designed.clone(),
        },
    );
    send(
        &mut app,
        EditorCommand::SoundLibrary(P::Save {
            sound: output.clone(),
            name: " Warm bass ".into(),
        }),
    );
    assert_eq!(
        app.world().resource::<MusaicProject>().sound_library["Warm bass"],
        designed
    );
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: InstrumentDefinition::default(),
        },
    );
    assert_eq!(
        app.world().resource::<MusaicProject>().sound_library["Warm bass"],
        designed,
        "the library is a reusable snapshot"
    );
    send(
        &mut app,
        EditorCommand::SoundLibrary(P::Rename {
            name: "Warm bass".into(),
            new_name: "Soft bass".into(),
        }),
    );
    let assigned = app
        .world()
        .resource::<MusaicProject>()
        .sound_definition(&output)
        .unwrap()
        .clone();
    send(
        &mut app,
        EditorCommand::SoundLibrary(P::Delete {
            name: "Soft bass".into(),
        }),
    );
    assert!(
        app.world()
            .resource::<MusaicProject>()
            .sound_library
            .is_empty()
    );
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .sound_definition(&output)
            .unwrap(),
        &assigned
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(
        app.world().resource::<MusaicProject>().sound_library["Soft bass"],
        designed
    );
    send(&mut app, EditorCommand::Undo);
    assert!(
        app.world()
            .resource::<MusaicProject>()
            .sound_library
            .contains_key("Warm bass")
    );
    send(&mut app, EditorCommand::Redo);
    let snapshot = app.world().resource::<MusaicProject>().sound_library["Soft bass"].clone();
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: snapshot,
        },
    );
    let reopened = import_project_bytes(
        &export_project_bytes(app.world().resource::<MusaicProject>()).unwrap(),
        None,
    )
    .unwrap();
    assert_eq!(reopened.sound_definition(&output).unwrap(), &designed);
    assert_eq!(reopened.sound_library["Soft bass"], designed);
}

#[test]
fn invalid_or_duplicate_saved_sounds_leave_library_and_history_unchanged() {
    use musaic::application::command::sound_library::SoundLibraryCommand as P;
    let (mut app, output) = editor();
    send(
        &mut app,
        EditorCommand::SoundLibrary(P::Save {
            sound: output.clone(),
            name: "Bass".into(),
        }),
    );
    let saved = app
        .world()
        .resource::<MusaicProject>()
        .sound_library
        .clone();
    let history = app.world().resource::<CommandHistory>().undo_len();
    for command in [
        P::Save {
            sound: output.clone(),
            name: "bass".into(),
        },
        P::Save {
            sound: output.clone(),
            name: "  ".into(),
        },
        P::Save {
            sound: NodeId::new("absent"),
            name: "Other".into(),
        },
        P::Rename {
            name: "Bass".into(),
            new_name: "x".repeat(129),
        },
        P::Delete {
            name: "absent".into(),
        },
    ] {
        send(&mut app, EditorCommand::SoundLibrary(command));
        assert_eq!(app.world().resource::<MusaicProject>().sound_library, saved);
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), history);
    }
    let mut malformed = app.world().resource::<MusaicProject>().clone();
    malformed.sound_library.insert(
        "Missing sample".into(),
        InstrumentDefinition::new(InstrumentSource::Sample(
            musaic::domain::project::samples::SampleId(999),
        )),
    );
    assert!(export_project_bytes(&malformed).is_err());
}

#[test]
fn reusable_sample_sound_keeps_asset_identity_and_audio_after_native_reopen() {
    use musaic::{
        adapter::persistence::{import_wav_bytes, load_project, save_project},
        application::audio_export::render_project_wav,
        application::command::sound_library::SoundLibraryCommand as P,
        domain::project::samples::SampleImportOptions,
    };
    let (mut app, output) = editor();
    app.world_mut()
        .resource_mut::<MusaicProject>()
        .document
        .playback
        .bpm = 240.0;
    app.world_mut()
        .resource_mut::<MusaicProject>()
        .document
        .playback
        .beats_per_cycle = 1;
    let recording = render_project_wav(app.world().resource::<MusaicProject>(), 1).unwrap();
    let sample = import_wav_bytes(
        &mut app.world_mut().resource_mut::<MusaicProject>(),
        "recording.wav",
        recording,
        SampleImportOptions {
            root_pitch: Some(60.0),
            ..Default::default()
        },
    )
    .unwrap();
    let mut instrument = designed_sound();
    instrument.source = InstrumentSource::Sample(sample);
    send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: instrument.clone(),
        },
    );
    send(
        &mut app,
        EditorCommand::SoundLibrary(P::Save {
            sound: output.clone(),
            name: "Recorded keys".into(),
        }),
    );
    let rendered = render_project_wav(app.world().resource::<MusaicProject>(), 2).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "musaic-saved-sample-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("sounds.musaic.json");
    save_project(&path, app.world().resource::<MusaicProject>()).unwrap();
    let reopened = load_project(&path).unwrap();
    assert_eq!(reopened.sound_library["Recorded keys"], instrument);
    assert_eq!(
        reopened.sound_library["Recorded keys"].source,
        InstrumentSource::Sample(sample)
    );
    assert!(reopened.samples.decoded(sample).is_some());
    assert_eq!(render_project_wav(&reopened, 2).unwrap(), rendered);
    std::fs::remove_dir_all(directory).unwrap();
}
