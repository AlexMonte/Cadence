//! Imported banks use identical ordered decoded variants in the editor and WAV export.
#![cfg(not(target_arch = "wasm32"))]
#[path = "support/acceptance.rs"]
mod support;
use bevy::prelude::*;
use musaic::{
    adapter::{
        audio::project_samples::load_project_samples,
        persistence::{export_project_bytes, import_wav_bytes, load_project, save_project},
    },
    application::{
        audio_export::render_project_wav,
        command::{EditorCommand, EditorCommandBus},
        history::CommandHistory,
        session::MusaicProject,
    },
    domain::{
        board::BoardSlot,
        document::{
            AtomValue, ContainerKind, NoteName, PlacementAddress, StackIndex, TileSpawnKind,
            bind_tiles, export_document_program,
        },
        instrument::{InstrumentDefinition, InstrumentSource},
        project::samples::{
            SampleBankDefinition, SampleId, SampleImportOptions, SamplePitchZone, SampleVariant,
        },
    },
};
use support::{Folder, editor, send, wait_for};
use tessera::prelude::{AtomModifier, Rational, SpatialSide};

fn constant_wav(value: i16) -> Vec<u8> {
    let mut bytes = support::wav(48_000, 144_000, value);
    for frame in bytes[44..].chunks_exact_mut(2) {
        frame.copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn fixture(modifiers: Vec<AtomModifier>) -> (App, [SampleId; 3]) {
    let mut project = MusaicProject::new_empty();
    let root = project.document.root_surface;
    let pattern = project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        )
        .unwrap();
    let surface = project.document.graph.container_surface(&pattern).unwrap();
    let mut atoms = Vec::new();
    for note in [NoteName::C, NoteName::E] {
        atoms.push(AtomValue::NoteName(note));
        atoms.push(AtomValue::Octave(4));
        atoms.extend(modifiers.iter().cloned().map(AtomValue::Modifier));
    }
    for (index, atom) in atoms.into_iter().enumerate() {
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
    let instrument = project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(5, 0)),
            TileSpawnKind::sound(InstrumentDefinition::default()),
        )
        .unwrap();
    let output = project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(6, 0)),
            TileSpawnKind::Output { name: "out".into() },
        )
        .unwrap();
    let mut program = export_document_program(&project.document).unwrap();
    bind_tiles(&mut program, &pattern, &instrument, SpatialSide::East).unwrap();
    bind_tiles(&mut program, &instrument, &output, SpatialSide::East).unwrap();
    project.document.replace_connections_from(&program);
    let ids = [
        ("C.wav", 60.0, 4000),
        ("E.wav", 64.0, -6000),
        ("G.wav", 67.0, 8000),
    ]
    .map(|(name, root, value)| {
        import_wav_bytes(
            &mut project,
            name,
            constant_wav(value),
            SampleImportOptions {
                sustain_loop: None,
                root_pitch: Some(root),
                default_gain: None,
            },
        )
        .unwrap()
    });
    project
        .set_sound_definition(
            &instrument,
            InstrumentDefinition::new(InstrumentSource::Sample(ids[0])),
        )
        .unwrap();
    (editor(project), ids)
}

fn project(app: &App) -> &MusaicProject {
    app.world().resource()
}
fn set_bank(app: &mut App, lead: SampleId, definition: Option<SampleBankDefinition>) {
    send(
        app,
        EditorCommand::SetSampleBank {
            sample: lead,
            definition,
        },
    );
}
fn pcm(wav: &[u8], frame: usize) -> i16 {
    i16::from_le_bytes(wav[44 + frame * 4..46 + frame * 4].try_into().unwrap())
}
fn bank(ids: [SampleId; 3]) -> SampleBankDefinition {
    SampleBankDefinition {
        variants: ids
            .into_iter()
            .map(|sample| SampleVariant {
                bank: Some("soft".into()),
                ..SampleVariant::new(sample)
            })
            .collect(),
    }
}

#[test]
fn pitch_zones_bank_labels_and_variant_order_reach_saved_rendered_audio() {
    let (mut app, ids) = fixture(vec![]);
    let mut definition = bank(ids);
    for (variant, (low, high)) in
        definition
            .variants
            .iter_mut()
            .zip([(0.0, 61.0), (62.0, 65.0), (66.0, 127.0)])
    {
        variant.pitch_zone = Some(SamplePitchZone { low, high });
    }
    set_bank(&mut app, ids[0], Some(definition));
    let wave = render_project_wav(project(&app), 1).unwrap();
    let halfway = (wave.len() - 44) / 8;
    assert!(pcm(&wave, 1000) > 1000);
    assert!(
        pcm(&wave, halfway + 1000) < -1000,
        "E note must select the E zone recording"
    );

    let (mut selected, selected_ids) = fixture(vec![
        AtomModifier::SampleBank("soft".into()),
        AtomModifier::SampleVariant(1),
    ]);
    let definition = bank(selected_ids);
    set_bank(&mut selected, selected_ids[0], Some(definition.clone()));
    let before = render_project_wav(project(&selected), 1).unwrap();
    assert!(pcm(&before, 1000) < -1000);
    let mut reordered = definition.clone();
    reordered.variants.swap(1, 2);
    set_bank(&mut selected, selected_ids[0], Some(reordered.clone()));
    let after = render_project_wav(project(&selected), 1).unwrap();
    assert!(
        pcm(&after, 1000) > 1000,
        "variant 1 must follow the explicit saved ordering"
    );
    send(&mut selected, EditorCommand::Undo);
    assert_eq!(render_project_wav(project(&selected), 1).unwrap(), before);
    send(&mut selected, EditorCommand::Redo);
    assert_eq!(render_project_wav(project(&selected), 1).unwrap(), after);
    let folder = Folder::new();
    let path = folder.0.join("bank/song.musaic.json");
    save_project(&path, project(&selected)).unwrap();
    let reopened = load_project(&path).unwrap();
    assert_eq!(
        reopened.samples.manifest().banks[&selected_ids[0]],
        reordered
    );
    assert_eq!(render_project_wav(&reopened, 1).unwrap(), after);

    // Export command captures a project snapshot before subsequent bank edits.
    let export_path = folder.0.join("bank.wav");
    for command in [
        EditorCommand::ExportAudio {
            path: export_path.clone(),
            cycles: 1,
        },
        EditorCommand::SetSampleBank {
            sample: selected_ids[0],
            definition: Some(definition),
        },
    ] {
        selected
            .world_mut()
            .write_message(EditorCommandBus(command));
    }
    selected.update();
    wait_for(&mut selected, |_| export_path.exists());
    assert_eq!(std::fs::read(export_path).unwrap(), after);
}

#[test]
fn equal_hints_keep_distinct_variants_and_bank_filtering_changes_the_recording() {
    let (mut app, ids) = fixture(vec![
        AtomModifier::SampleBank("bright".into()),
        AtomModifier::SampleVariant(0),
    ]);
    let mut definition = bank(ids);
    // Matching roots and zones must not cause the bank loader to deduplicate
    // separately imported takes.
    for sample in ids {
        send(
            &mut app,
            EditorCommand::SetSampleOptions {
                sample,
                options: SampleImportOptions {
                    sustain_loop: None,
                    root_pitch: Some(60.0),
                    default_gain: None,
                },
            },
        );
    }
    definition.variants[0].bank = Some("soft".into());
    definition.variants[1].bank = Some("bright".into());
    definition.variants[2].bank = Some("bright".into());
    set_bank(&mut app, ids[0], Some(definition.clone()));
    assert!(pcm(&render_project_wav(project(&app), 1).unwrap(), 1000) < -1000);
    definition.variants.swap(1, 2);
    set_bank(&mut app, ids[0], Some(definition));
    assert!(pcm(&render_project_wav(project(&app), 1).unwrap(), 1000) > 1000);
}

#[test]
fn invalid_membership_or_settings_preserve_history_and_missing_members_stop_export() {
    let (mut app, ids) = fixture(vec![]);
    set_bank(&mut app, ids[0], Some(bank(ids)));
    let before = export_project_bytes(project(&app)).unwrap();
    let history = app.world().resource::<CommandHistory>().undo_len();
    let mut duplicate = bank(ids);
    duplicate.variants.push(SampleVariant::new(ids[1]));
    let mut missing = bank(ids);
    missing.variants[1].sample = SampleId(999);
    let mut unordered = bank(ids);
    unordered.variants[0].sample = ids[1];
    let mut zone = bank(ids);
    zone.variants[0].pitch_zone = Some(SamplePitchZone {
        low: 100.0,
        high: 60.0,
    });
    let mut nonfinite = bank(ids);
    nonfinite.variants[0].pitch_zone = Some(SamplePitchZone {
        low: f64::NAN,
        high: 60.0,
    });
    let mut limit = bank(ids);
    limit.variants[0].playback_limit_ms = Some(0);
    for invalid in [
        duplicate,
        missing,
        unordered,
        zone,
        nonfinite,
        limit,
        SampleBankDefinition { variants: vec![] },
    ] {
        set_bank(&mut app, ids[0], Some(invalid));
        assert_eq!(export_project_bytes(project(&app)).unwrap(), before);
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), history);
    }
    set_bank(&mut app, ids[1], Some(SampleBankDefinition::new(ids[1])));
    assert_eq!(
        export_project_bytes(project(&app)).unwrap(),
        before,
        "membership in two banks is ambiguous and rejected"
    );
    let folder = Folder::new();
    let path = folder.0.join("song.musaic.json");
    save_project(&path, project(&app)).unwrap();
    std::fs::remove_file(
        folder
            .0
            .join(&project(&app).samples.manifest().samples[&ids[1]].relative_path),
    )
    .unwrap();
    let missing = load_project(&path).unwrap();
    assert!(render_project_wav(&missing, 1).is_err());
    let runtime_bank = cadence::infrastructure::playback::SampleBank::new();
    load_project_samples(project(&app), &runtime_bank);
    assert!(runtime_bank.contains(&ids[0].runtime_name()));
    load_project_samples(&missing, &runtime_bank);
    assert!(
        !runtime_bank.contains(&ids[0].runtime_name()),
        "missing member must retire stale lead alias"
    );
}

#[test]
fn member_relink_preserves_order_and_bank_dissolution_preserves_recordings() {
    let (mut app, ids) = fixture(vec![AtomModifier::SampleVariant(1)]);
    let definition = bank(ids);
    set_bank(&mut app, ids[0], Some(definition.clone()));
    let before = render_project_wav(project(&app), 1).unwrap();
    let folder = Folder::new();
    let replacement = folder.0.join("replacement.wav");
    std::fs::write(&replacement, constant_wav(9000)).unwrap();
    let revision = project(&app).samples.asset_revision(ids[1]);
    send(
        &mut app,
        EditorCommand::RelinkSample {
            sample: ids[1],
            path: replacement,
        },
    );
    wait_for(&mut app, |app| {
        project(app).samples.asset_revision(ids[1]) > revision
    });
    assert_eq!(project(&app).samples.manifest().banks[&ids[0]], definition);
    let after = render_project_wav(project(&app), 1).unwrap();
    assert_ne!(after, before);
    assert!(pcm(&after, 1000) > 1000);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(render_project_wav(project(&app), 1).unwrap(), before);
    send(&mut app, EditorCommand::Redo);
    set_bank(&mut app, ids[0], None);
    assert_eq!(project(&app).samples.manifest().samples.len(), 3);
    assert!(project(&app).samples.manifest().banks.is_empty());
    send(&mut app, EditorCommand::Undo);
    assert_eq!(render_project_wav(project(&app), 1).unwrap(), after);
}

#[test]
fn playback_limit_truncates_the_variant_in_real_pcm() {
    let (mut app, ids) = fixture(vec![AtomModifier::SampleVariant(0)]);
    let mut definition = bank(ids);
    definition.variants[0].playback_limit_ms = Some(20);
    set_bank(&mut app, ids[0], Some(definition));
    let wave = render_project_wav(project(&app), 1).unwrap();
    assert!(pcm(&wave, 100) > 0);
    assert_eq!(
        pcm(&wave, 4000),
        0,
        "playback limit stops the recording even while note duration continues"
    );
}

#[test]
fn hat_choke_releases_an_overlapping_bank_variant() {
    let (mut app, ids) = fixture(vec![AtomModifier::Legato(Rational::from_integer(2))]);
    let mut definition = bank(ids);
    set_bank(&mut app, ids[0], Some(definition.clone()));
    let overlapping = render_project_wav(project(&app), 1).unwrap();
    for variant in &mut definition.variants {
        variant.hat_choke = true;
    }
    set_bank(&mut app, ids[0], Some(definition));
    let choked = render_project_wav(project(&app), 1).unwrap();
    let halfway = (choked.len() - 44) / 8;
    let previous = pcm(&overlapping, halfway + 4000);
    let current = pcm(&choked, halfway + 4000);
    assert!(
        previous < -100 && current < previous * 2,
        "the positive first recording should stop mixing with the negative second one: {previous} -> {current}"
    );
}
