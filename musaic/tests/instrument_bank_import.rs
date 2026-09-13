//! Real preset decoding plus atomic import and portable loop metadata contracts.
#![cfg(not(target_arch = "wasm32"))]
#[path = "support/acceptance.rs"]
mod support;

use musaic::{
    MusaicProject,
    adapter::persistence::{
        adopt_prepared_sample_bank, export_project_bytes, load_project, prepare_sample_bank,
        save_project, set_sample_options,
    },
    domain::project::samples::{SampleImportOptions, SampleLoopRegion},
};
use std::path::Path;

#[test]
fn all_birds_instruments_import_original_zones_and_survive_project_save() {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/instrument-banks/birds-of-a-feather");
    let index: std::collections::BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read(directory.join("index.json")).unwrap()).unwrap();
    let mut project = MusaicProject::new_empty();
    let mut roots = Vec::new();
    for (name, path) in index {
        let prepared = prepare_sample_bank(directory.join(path))
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let lead = adopt_prepared_sample_bank(&mut project, prepared).unwrap();
        let bank = &project.samples.manifest().banks[&lead];
        assert!(!bank.variants.is_empty());
        if name == "Kalimba" {
            let recording = &project.samples.manifest().samples[&lead];
            assert_eq!(recording.options.root_pitch, Some(72.03));
            assert_eq!(
                recording.options.sustain_loop,
                Some(SampleLoopRegion {
                    start_frame: 2024,
                    end_frame: 2192
                })
            );
            assert!(
                recording.frame_count < 44100,
                "original kalimba must not be expanded to eight seconds"
            );
        }
        if name == "Organ" {
            assert_eq!(bank.variants.len(), 3);
            assert!(bank.variants.iter().all(|variant| {
                project.samples.manifest().samples[&variant.sample]
                    .options
                    .sustain_loop
                    .is_some()
            }));
        }
        if name == "Linn hi-hat" {
            assert!(bank.variants[0].hat_choke);
            assert_eq!(bank.variants[0].bank.as_deref(), Some("LinnDrum"));
        }
        roots.push(lead);
    }
    assert_eq!(project.samples.manifest().banks.len(), 12);
    assert_eq!(project.samples.manifest().samples.len(), 28);
    let folder = support::Folder::new();
    let path = folder.0.join("song.musaic.json");
    save_project(&path, &project).unwrap();
    let reopened = load_project(&path).unwrap();
    assert!(reopened.samples.diagnostics().is_empty());
    assert_eq!(reopened.samples.manifest(), project.samples.manifest());
    for lead in roots {
        assert_eq!(
            reopened.samples.decoded(lead).unwrap().frames(),
            project.samples.decoded(lead).unwrap().frames()
        );
    }
}

#[test]
fn bad_member_or_loop_cannot_partially_import_or_replace_metadata() {
    let folder = support::Folder::new();
    std::fs::write(folder.0.join("valid.wav"), support::wav(8000, 8, 1000)).unwrap();
    let path = folder.0.join("instrument.musaic-bank.json");
    for variants in [
        serde_json::json!([{"path":"valid.wav"},{"path":"missing.wav"}]),
        serde_json::json!([{"path":"valid.wav","options":{"sustain_loop":{"start_frame":4,"end_frame":9}}}]),
        serde_json::json!([{"path":"../valid.wav"}]),
        serde_json::json!([{"path":"valid.wav","pitch_zone":{"low":80,"high":40}}]),
        serde_json::json!([{"path":"valid.wav","sample_rate":44100}]),
    ] {
        std::fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({"variants":variants})).unwrap(),
        )
        .unwrap();
        assert!(prepare_sample_bank(&path).is_err());
    }
    std::fs::write(&path, r#"{"variants":[{"path":"valid.wav","options":{"sustain_loop":{"start_frame":2,"end_frame":6}}}]}"#).unwrap();
    let prepared = prepare_sample_bank(&path).unwrap();
    let mut project = MusaicProject::new_empty();
    let lead = adopt_prepared_sample_bank(&mut project, prepared.clone()).unwrap();
    let before = export_project_bytes(&project).unwrap();
    assert!(
        set_sample_options(
            &mut project,
            lead,
            SampleImportOptions {
                sustain_loop: Some(SampleLoopRegion {
                    start_frame: 2,
                    end_frame: 9
                }),
                ..Default::default()
            }
        )
        .is_err()
    );
    assert_eq!(export_project_bytes(&project).unwrap(), before);
    // Fill the destination so the second member cannot be adopted; the whole
    // bank operation must preserve its existing identity counter and content.
    for _ in 1..255 {
        adopt_prepared_sample_bank(&mut project, prepared.clone()).unwrap();
    }
    std::fs::write(
        &path,
        r#"{"variants":[{"path":"valid.wav"},{"path":"valid.wav"}]}"#,
    )
    .unwrap();
    let prepared = prepare_sample_bank(&path).unwrap();
    let before = export_project_bytes(&project).unwrap();
    assert!(adopt_prepared_sample_bank(&mut project, prepared).is_err());
    assert_eq!(export_project_bytes(&project).unwrap(), before);
}
