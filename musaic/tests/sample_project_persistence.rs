//! Native project portability, asset integrity, and WAV import contracts.

#![cfg(not(target_arch = "wasm32"))]

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use musaic::{
    MusaicProject,
    adapter::persistence::{
        adopt_prepared_sample, export_project_bytes, import_project_bytes, import_wav_bytes,
        import_wav_sample, load_project, prepare_wav_sample, save_project,
    },
    domain::{
        document::DocumentNodeKind,
        instrument::{InstrumentDefinition, InstrumentSource},
        project::samples::{SampleDiagnosticKind, SampleImportOptions},
    },
};
use proptest::prelude::*;
use tessera::prelude::NodeId;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        let number = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "musaic-samples-{}-{number}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn pcm16_wav(rate: u32, channels: u16, frames: &[[i16; 2]]) -> Vec<u8> {
    let length = (frames.len() * channels as usize * 2) as u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + length).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&length.to_le_bytes());
    for frame in frames {
        for channel in &frame[..channels as usize] {
            bytes.extend_from_slice(&channel.to_le_bytes());
        }
    }
    bytes
}

fn fixture() -> Vec<u8> {
    pcm16_wav(
        44_100,
        2,
        &[
            [i16::MIN, i16::MAX],
            [0, 8192],
            [-16384, 16384],
            [1024, -2048],
        ],
    )
}

fn instrument_nodes(project: &MusaicProject) -> Vec<NodeId> {
    project
        .document
        .graph
        .nodes_on_surface(project.document.root_surface)
        .into_iter()
        .filter(|(_, node)| matches!(&node.kind, DocumentNodeKind::Sound(_)))
        .map(|(_, node)| node.id.clone())
        .collect()
}

fn instrument_node(project: &MusaicProject) -> NodeId {
    instrument_nodes(project)
        .into_iter()
        .next()
        .expect("fixture has a connected Sound tile")
}

#[test]
fn wav_project_moves_and_save_as_keeps_identical_decoded_audio_and_identity() {
    let directory = TestDirectory::new();
    let source = directory.0.join("Original piano.wav");
    let bytes = fixture();
    std::fs::write(&source, &bytes).unwrap();
    let mut project = MusaicProject::demo();
    let id = import_wav_sample(
        &mut project,
        &source,
        SampleImportOptions {
            sustain_loop: None,
            root_pitch: Some(60.0),
            default_gain: Some(0.75),
        },
    )
    .unwrap();
    let instrument = instrument_node(&project);
    project
        .set_sound_definition(
            &instrument,
            InstrumentDefinition::new(InstrumentSource::Sample(id)),
        )
        .unwrap();
    let original_manifest = project.samples.manifest().clone();
    let original_frames = project.samples.decoded(id).unwrap().frames().to_vec();
    let folder = directory.0.join("Original project");
    let project_path = folder.join("song.musaic.json");
    save_project(&project_path, &project).unwrap();
    std::fs::remove_file(&source).unwrap();
    let moved = directory.0.join("Moved project");
    std::fs::rename(&folder, &moved).unwrap();
    let reopened = load_project(moved.join("song.musaic.json")).unwrap();
    assert_eq!(reopened.samples.manifest(), &original_manifest);
    assert_eq!(reopened.samples.decoded(id).unwrap().sample_rate(), 44_100);
    assert_eq!(
        reopened.samples.decoded(id).unwrap().frames(),
        &original_frames
    );
    assert_eq!(reopened.document.graph, project.document.graph);
    assert!(reopened.samples.diagnostics().is_empty());
    let json = std::fs::read_to_string(moved.join("song.musaic.json")).unwrap();
    assert!(
        !json.contains(&directory.0.display().to_string()),
        "absolute paths must not be stored"
    );
    assert_eq!(
        std::fs::read(moved.join(&original_manifest.samples[&id].relative_path)).unwrap(),
        bytes
    );
    let copy = directory.0.join("Separate copy/song.musaic.json");
    save_project(&copy, &reopened).unwrap();
    std::fs::remove_dir_all(&moved).unwrap();
    let copied = load_project(copy).unwrap();
    assert_eq!(copied.samples.manifest(), &original_manifest);
    assert_eq!(
        copied.samples.decoded(id).unwrap().frames(),
        &original_frames
    );
    assert!(copied.samples.diagnostics().is_empty());
}

#[test]
fn background_preparation_does_not_mutate_a_project_and_adoption_is_atomic() {
    let directory = TestDirectory::new();
    let source = directory.0.join("worker.wav");
    std::fs::write(&source, fixture()).unwrap();
    let mut project = MusaicProject::new_empty();
    let before = export_project_bytes(&project).unwrap();
    let prepared =
        std::thread::spawn(move || prepare_wav_sample(source, SampleImportOptions::default()))
            .join()
            .unwrap()
            .unwrap();
    assert_eq!(export_project_bytes(&project).unwrap(), before);
    let revision = project.samples.revision();
    let id = adopt_prepared_sample(&mut project, prepared).unwrap();
    assert!(project.samples.decoded(id).is_some());
    assert!(project.samples.revision() > revision);
    assert!(project.metadata.dirty);
}

#[test]
fn missing_or_changed_wav_keeps_the_binding_and_produces_a_diagnostic() {
    let directory = TestDirectory::new();
    let mut project = MusaicProject::demo();
    let id = import_wav_bytes(
        &mut project,
        "piano.wav",
        fixture(),
        SampleImportOptions::default(),
    )
    .unwrap();
    let instrument = instrument_node(&project);
    project
        .set_sound_definition(
            &instrument,
            InstrumentDefinition::new(InstrumentSource::Sample(id)),
        )
        .unwrap();
    let path = directory.0.join("song.musaic.json");
    save_project(&path, &project).unwrap();
    let saved_json = std::fs::read(&path).unwrap();
    let asset = directory
        .0
        .join(&project.samples.manifest().samples[&id].relative_path);
    std::fs::remove_file(&asset).unwrap();
    let missing = load_project(&path).unwrap();
    assert!(missing.samples.decoded(id).is_none());
    assert_eq!(missing.document.graph, project.document.graph);
    assert_eq!(
        missing.samples.diagnostics()[0].kind,
        SampleDiagnosticKind::Missing
    );
    assert!(save_project(&path, &missing).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), saved_json);
    let mut changed_bytes = fixture();
    *changed_bytes.last_mut().unwrap() ^= 1;
    std::fs::write(&asset, changed_bytes).unwrap();
    let changed = load_project(&path).unwrap();
    assert!(changed.samples.decoded(id).is_none());
    assert_eq!(
        changed.samples.diagnostics()[0].kind,
        SampleDiagnosticKind::Invalid
    );
    assert_eq!(changed.document.graph, project.document.graph);
}

#[test]
fn failed_save_preserves_previous_manifest_and_referenced_audio() {
    let directory = TestDirectory::new();
    let path = directory.0.join("song.musaic.json");
    let mut project = MusaicProject::demo();
    let first = import_wav_bytes(
        &mut project,
        "first.wav",
        fixture(),
        SampleImportOptions::default(),
    )
    .unwrap();
    save_project(&path, &project).unwrap();
    let previous_json = std::fs::read(&path).unwrap();
    let first_asset = directory
        .0
        .join(&project.samples.manifest().samples[&first].relative_path);
    let previous_wav = std::fs::read(&first_asset).unwrap();
    project.metadata.display_name = "Unsaved edit".into();
    let second = import_wav_bytes(
        &mut project,
        "second.wav",
        pcm16_wav(8000, 1, &[[1234, 0]]),
        SampleImportOptions::default(),
    )
    .unwrap();
    let blocked_path = directory
        .0
        .join(&project.samples.manifest().samples[&second].relative_path);
    std::fs::create_dir(&blocked_path).unwrap();
    assert!(save_project(&path, &project).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), previous_json);
    assert_eq!(std::fs::read(first_asset).unwrap(), previous_wav);
    let previous = load_project(&path).unwrap();
    assert_eq!(previous.metadata.display_name, "Small changes");
    assert_eq!(previous.samples.manifest().samples.len(), 1);
    assert!(previous.samples.diagnostics().is_empty());
}

#[test]
fn malformed_wav_and_manifest_paths_are_rejected_without_partial_import() {
    let mut project = MusaicProject::demo();
    let before = export_project_bytes(&project).unwrap();
    for bytes in [b"not a WAV".to_vec(), fixture()[..45].to_vec()] {
        assert!(
            import_wav_bytes(
                &mut project,
                "broken.wav",
                bytes,
                SampleImportOptions::default()
            )
            .is_err()
        );
        assert_eq!(export_project_bytes(&project).unwrap(), before);
    }
    let id = import_wav_bytes(
        &mut project,
        "valid.wav",
        fixture(),
        SampleImportOptions::default(),
    )
    .unwrap();
    let mut json: serde_json::Value =
        serde_json::from_slice(&export_project_bytes(&project).unwrap()).unwrap();
    json["samples"]["samples"][id.0.to_string()]["relative_path"] = "../../outside.wav".into();
    assert!(import_project_bytes(&serde_json::to_vec(&json).unwrap(), None).is_err());
    assert_eq!(project.samples.manifest().samples.len(), 1);

    let valid = MusaicProject::demo();
    let mut json: serde_json::Value =
        serde_json::from_slice(&export_project_bytes(&valid).unwrap()).unwrap();
    json["unexpected"] = serde_json::json!({"value": true});
    assert!(import_project_bytes(&serde_json::to_vec(&json).unwrap(), None).is_err());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn pcm16_import_matches_original_samples(
        frames in prop::collection::vec(any::<[i16; 2]>(), 1..128),
        stereo in any::<bool>(),
        rate in prop::sample::select(vec![8_000_u32, 11_025, 44_100, 48_000, 96_000]),
    ) {
        let channels = if stereo { 2 } else { 1 };
        let mut project = MusaicProject::new_empty();
        let id = import_wav_bytes(&mut project, "generated.wav", pcm16_wav(rate, channels, &frames), SampleImportOptions::default()).unwrap();
        let decoded = project.samples.decoded(id).unwrap();
        prop_assert_eq!(decoded.len(), frames.len());
        prop_assert_eq!(decoded.sample_rate(), rate);
        prop_assert_eq!(project.samples.manifest().samples[&id].channels, channels);
        for (actual, original) in decoded.frames().iter().zip(&frames) {
            prop_assert_eq!(actual.left, original[0] as f32 / 32768.0);
            prop_assert_eq!(actual.right, original[if stereo { 1 } else { 0 }] as f32 / 32768.0);
        }
    }
}

#[test]
fn recovery_is_a_portable_unsaved_copy_and_never_overwrites_the_original() {
    use musaic::adapter::persistence::recovery::{
        list_recoveries_in, load_recovery, save_recovery,
    };
    let directory = TestDirectory::new();
    let mut project = MusaicProject::demo();
    let original = directory.0.join("original/song.musaic.json");
    let recovery = directory
        .0
        .join("recoveries/session-fixture/recovery.musaic.json");
    let bytes = fixture();
    let id = import_wav_bytes(
        &mut project,
        "stereo.wav",
        bytes.as_slice(),
        SampleImportOptions::default(),
    )
    .unwrap();
    save_project(&original, &project).unwrap();
    let original_bytes = std::fs::read(&original).unwrap();
    project.metadata.display_name = "Recovered pattern".into();
    project.metadata.dirty = true;
    let expected_graph = project.document.graph.clone();
    save_recovery(&recovery, &project).unwrap();
    assert!(
        project.metadata.dirty,
        "autosave cannot mark the user's project saved"
    );
    assert_eq!(std::fs::read(&original).unwrap(), original_bytes);
    let entries = list_recoveries_in(&directory.0.join("recoveries"));
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "Recovered pattern");
    let recovered = load_recovery(&recovery).unwrap();
    assert_eq!(recovered.document.graph, expected_graph);
    assert!(recovered.metadata.dirty);
    assert!(
        recovered.metadata.file_path.is_none(),
        "recovery must ask for a real save destination"
    );
    assert_eq!(
        recovered.samples.decoded(id).unwrap().frames(),
        project.samples.decoded(id).unwrap().frames()
    );
    let saved_as = directory.0.join("recovered-copy/song.musaic.json");
    save_project(&saved_as, &recovered).unwrap();
    std::fs::remove_dir_all(directory.0.join("recoveries")).unwrap();
    assert_eq!(
        load_project(&saved_as)
            .unwrap()
            .samples
            .decoded(id)
            .unwrap()
            .frames(),
        project.samples.decoded(id).unwrap().frames()
    );
}

#[test]
fn invalid_sound_edit_does_not_change_the_recovery_snapshot() {
    use musaic::adapter::persistence::recovery::{load_recovery, save_recovery};
    let directory = TestDirectory::new();
    let mut project = MusaicProject::demo();
    let recovery = directory.0.join("session-fixture/recovery.musaic.json");
    save_recovery(&recovery, &project).unwrap();
    let previous = std::fs::read(&recovery).unwrap();
    let instrument = instrument_node(&project);
    let graph = project.document.graph.clone();
    assert!(
        project
            .set_sound_definition(
                &instrument,
                InstrumentDefinition {
                    rate: tessera::prelude::Rational::zero(),
                    ..InstrumentDefinition::default()
                },
            )
            .is_err()
    );
    assert_eq!(project.document.graph, graph);
    assert_eq!(std::fs::read(&recovery).unwrap(), previous);
    assert_eq!(
        load_recovery(&recovery).unwrap().document.graph,
        project.document.graph
    );
}

#[test]
fn project_wav_export_keeps_stereo_duration_and_imported_sound_after_recovery() {
    use musaic::application::audio_export::render_project_wav;
    let directory = TestDirectory::new();
    let mut project = MusaicProject::demo();
    project.document.playback.bpm = 240.0;
    project.document.playback.beats_per_cycle = 1;
    let id = import_wav_bytes(
        &mut project,
        "stereo.wav",
        fixture(),
        SampleImportOptions {
            default_gain: Some(0.05),
            ..Default::default()
        },
    )
    .unwrap();
    let instruments = instrument_nodes(&project);
    assert!(!instruments.is_empty());
    for instrument in instruments {
        project
            .set_sound_definition(
                &instrument,
                InstrumentDefinition::new(InstrumentSource::Sample(id)),
            )
            .unwrap();
    }
    let first = render_project_wav(&project, 2).unwrap();
    assert_eq!(&first[..4], b"RIFF");
    assert_eq!(u16::from_le_bytes(first[22..24].try_into().unwrap()), 2);
    assert_eq!(
        u32::from_le_bytes(first[24..28].try_into().unwrap()),
        48_000
    );
    assert_eq!(
        first.len(),
        44 + 24_000 * 4,
        "two quarter-second cycles at 48kHz stereo"
    );
    let samples: Vec<_> = first[44..]
        .chunks_exact(4)
        .map(|frame| {
            (
                i16::from_le_bytes(frame[..2].try_into().unwrap()),
                i16::from_le_bytes(frame[2..].try_into().unwrap()),
            )
        })
        .collect();
    assert!(
        samples
            .iter()
            .any(|(left, right)| *left < -100 && *right > 100),
        "opposite source channels retain their polarity"
    );
    let path = directory.0.join("session-export/recovery.musaic.json");
    musaic::adapter::persistence::recovery::save_recovery(&path, &project).unwrap();
    let recovered = musaic::adapter::persistence::recovery::load_recovery(&path).unwrap();
    assert_eq!(
        render_project_wav(&recovered, 2).unwrap(),
        first,
        "recovery exports the same authored stereo result"
    );
    assert!(render_project_wav(&recovered, 0).is_err());
    assert!(render_project_wav(&recovered, u32::MAX).is_err());
}

#[test]
fn synth_patches_and_noise_export_identically_after_project_reload() {
    use musaic::{
        application::audio_export::render_project_wav,
        domain::instrument::{SynthPreset, Waveform},
    };
    for source in [
        InstrumentSource::Preset(SynthPreset::Bass),
        InstrumentSource::Preset(SynthPreset::Pad),
        InstrumentSource::Preset(SynthPreset::Percussion),
        InstrumentSource::Synth(Waveform::Noise),
    ] {
        let mut project = MusaicProject::demo();
        project.document.playback.bpm = 240.0;
        project.document.playback.beats_per_cycle = 1;
        for instrument in instrument_nodes(&project) {
            project
                .set_sound_definition(&instrument, InstrumentDefinition::new(source.clone()))
                .unwrap();
        }
        let before = render_project_wav(&project, 1).unwrap();
        let reopened =
            import_project_bytes(&export_project_bytes(&project).unwrap(), None).unwrap();
        assert_eq!(
            render_project_wav(&reopened, 1).unwrap(),
            before,
            "{source:?}"
        );
    }
}
