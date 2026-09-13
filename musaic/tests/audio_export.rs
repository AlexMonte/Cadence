//! Export captures authored state once and only offers a location after a successful write.
#[path = "support/acceptance.rs"]
mod support;
use musaic::{
    MusaicProject,
    application::{
        audio_export::{AudioExportStatus, export_duration, render_project_wav},
        command::{EditorCommand, EditorCommandBus},
        session::first_loop,
    },
};
fn finish(app: &mut bevy::prelude::App) {
    let started = std::time::Instant::now();
    while app.world().resource::<AudioExportStatus>().busy {
        assert!(started.elapsed().as_secs() < 20, "export worker timeout");
        std::thread::sleep(std::time::Duration::from_millis(10));
        app.update();
    }
}
#[test]
fn duration_guards_cycles_tempo_and_memory_before_rendering() {
    let mut p = first_loop().project;
    p.document.playback.bpm = 120.0;
    p.document.playback.beats_per_cycle = 4;
    assert_eq!(export_duration(&p, 8), Ok(16.0));
    assert_eq!(export_duration(&p, 600), Ok(1200.0));
    for cycles in [0, 601, 4097, u32::MAX] {
        assert!(export_duration(&p, cycles).is_err());
    }
    p.document.playback.bpm = 999.0;
    p.document.playback.beats_per_cycle = 1;
    assert!(export_duration(&p, 4096).is_ok());
    for bpm in [0.0, -1.0, f64::NAN, f64::INFINITY, 1000.0] {
        p.document.playback.bpm = bpm;
        assert!(export_duration(&p, 1).is_err());
    }
    p.document.playback.bpm = 120.0;
    for beats in [0, 65] {
        p.document.playback.beats_per_cycle = beats;
        assert!(export_duration(&p, 1).is_err());
    }
}
#[test]
fn worker_exports_the_command_snapshot_and_failure_clears_the_location() {
    let folder = support::Folder::new();
    let path = folder.0.join("exact snapshot ' $(literal).wav");
    let mut captured = first_loop().project;
    captured.metadata.display_name = "Captured song".into();
    captured.document.playback.bpm = 120.0;
    let expected = render_project_wav(&captured, 1).unwrap();
    assert_eq!(&expected[..4], b"RIFF");
    assert_eq!(u16::from_le_bytes(expected[22..24].try_into().unwrap()), 2);
    assert_eq!(
        u32::from_le_bytes(expected[24..28].try_into().unwrap()),
        48_000
    );
    assert_eq!(expected.len(), 44 + 96_000 * 4);
    assert!(expected[44..].iter().any(|v| *v != 0), "audible PCM");
    let mut app = support::editor(captured);
    app.world_mut()
        .write_message(EditorCommandBus(EditorCommand::ExportAudio {
            path: path.clone(),
            cycles: 1,
        }));
    app.world_mut()
        .write_message(EditorCommandBus(EditorCommand::SetBpm { bpm: 240.0 }));
    app.update();
    assert!(app.world().resource::<AudioExportStatus>().busy);
    assert!(
        app.world()
            .resource::<AudioExportStatus>()
            .completed
            .is_none()
    );
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .playback
            .bpm,
        240.0
    );
    finish(&mut app);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        expected,
        "same PCM as captured project, despite subsequent edit"
    );
    let status = app.world().resource::<AudioExportStatus>();
    let receipt = status.completed.as_ref().unwrap();
    assert_eq!(
        (&receipt.project_name, receipt.cycles, &receipt.path),
        (&"Captured song".to_string(), 1, &path)
    );
    std::fs::remove_file(&path).unwrap();
    support::send(&mut app, EditorCommand::RevealLastExport);
    assert!(
        app.world()
            .resource::<AudioExportStatus>()
            .message
            .contains("Cannot locate")
    );
    // A directory cannot be overwritten as a WAV. No stale success survives this job.
    support::send(
        &mut app,
        EditorCommand::ExportAudio {
            path: folder.0.clone(),
            cycles: 1,
        },
    );
    assert!(
        app.world()
            .resource::<AudioExportStatus>()
            .completed
            .is_none()
    );
    finish(&mut app);
    let status = app.world().resource::<AudioExportStatus>();
    assert!(status.completed.is_none());
    assert!(status.message.starts_with("Export failed:"));
}
