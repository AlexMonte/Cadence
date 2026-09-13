//! Offline export captures the authored project at command time and compiles it
//! through the same compilation and instrument boundaries as realtime playback.

use crate::application::session::MusaicProject;
use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Mutex, mpsc};
use std::{path::PathBuf, time::Duration};

pub fn export_duration(project: &MusaicProject, cycles: u32) -> Result<f64, String> {
    if !(1..=4096).contains(&cycles) {
        return Err("Choose a whole number from 1 to 4096 cycles.".into());
    }
    let playback = &project.document.playback;
    if !playback.bpm.is_finite()
        || !(1.0..=999.0).contains(&playback.bpm)
        || !(1..=64).contains(&playback.beats_per_cycle)
    {
        return Err("Set a valid tempo and cycle length before exporting.".into());
    }
    let seconds = f64::from(cycles) / playback.cycles_per_second().value();
    if !seconds.is_finite() || seconds <= 0.0 || seconds > 1200.0 {
        return Err("Export must fit within 20 minutes at this tempo.".into());
    }
    Ok(seconds)
}
pub fn render_project_wav(project: &MusaicProject, cycles: u32) -> Result<Vec<u8>, String> {
    use cadence::{
        adapter::audio::offline::{OfflineRenderSettings, render_wav},
        infrastructure::playback::SampleBank,
    };
    let seconds = export_duration(project, cycles)?;
    let ir = super::compile::compile_project_ir(project).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    let (scores, diagnostics, _) = super::pipeline::lowering::lower_project_ir(project, &ir);
    if !diagnostics.is_empty() {
        return Err(format!(
            "The current project cannot render: {diagnostics:?}"
        ));
    }
    if scores.is_empty() && !super::compile::has_inactive_outputs(project) {
        return Err("Connect a pattern to an output before exporting".into());
    }
    let bank = SampleBank::new();
    crate::adapter::audio::load_builtin_samples(&bank);
    crate::adapter::audio::project_samples::load_project_samples(project, &bank);
    let cps = project.document.playback.cycles_per_second();
    let prepared = cadence::prelude::PreparedScore::new(cadence::prelude::merge(
        scores.into_values().collect(),
    ))
    .map_err(|error| error.to_string())?;
    render_wav(
        prepared,
        bank,
        OfflineRenderSettings::new(
            48_000,
            Duration::from_secs_f64(seconds),
            cps,
            (256 * 1024 * 1024 - 44) / 4,
        ),
    )
    .map_err(|error| error.to_string())
}

#[derive(Message)]
pub struct AudioExportRequest {
    pub project: MusaicProject,
    pub path: PathBuf,
    pub cycles: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletedExport {
    pub path: PathBuf,
    pub project_name: String,
    pub cycles: u32,
}
#[derive(Message)]
pub struct FileLocationFeedback(pub String);

#[derive(Resource, Default)]
pub struct AudioExportStatus {
    pub busy: bool,
    pub message: String,
    pub completed: Option<CompletedExport>,
}

#[derive(Resource)]
#[cfg(not(target_arch = "wasm32"))]
struct ExportWorker {
    sender: mpsc::Sender<Result<CompletedExport, String>>,
    receiver: Mutex<mpsc::Receiver<Result<CompletedExport, String>>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl Default for ExportWorker {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender,
            receiver: Mutex::new(receiver),
        }
    }
}

pub fn register(app: &mut App) {
    app.add_message::<AudioExportRequest>()
        .add_message::<FileLocationFeedback>()
        .init_resource::<AudioExportStatus>();
    #[cfg(not(target_arch = "wasm32"))]
    app.init_resource::<ExportWorker>().add_systems(
        Update,
        process_exports.after(crate::infrastructure::app::MusaicSet::Commands),
    );
    #[cfg(target_arch = "wasm32")]
    app.add_systems(
        Update,
        reject_browser_exports.after(crate::infrastructure::app::MusaicSet::Commands),
    );
}

#[cfg(not(target_arch = "wasm32"))]
fn process_exports(
    mut requests: MessageReader<AudioExportRequest>,
    worker: Res<ExportWorker>,
    mut status: ResMut<AudioExportStatus>,
) {
    if let Some(result) = worker
        .receiver
        .lock()
        .ok()
        .and_then(|receiver| receiver.try_recv().ok())
    {
        status.busy = false;
        status.message = match result {
            Ok(receipt) => {
                let message = format!(
                    "Saved {} · {} cycles from {}",
                    receipt.path.display(),
                    receipt.cycles,
                    receipt.project_name
                );
                status.completed = Some(receipt);
                message
            }
            Err(error) => {
                status.completed = None;
                format!("Export failed: {error}")
            }
        };
    }
    for request in requests.read() {
        if status.busy {
            status.message = "An export is already running".into();
            continue;
        }
        status.completed = None;
        let project = request.project.clone();
        let path = request.path.clone();
        let cycles = request.cycles;
        let sender = worker.sender.clone();
        match std::thread::Builder::new()
            .name("musaic-export".into())
            .spawn(move || {
                let result = render_project_wav(&project, cycles).and_then(|bytes| {
                    crate::adapter::persistence::write_audio_export(&path, &bytes)
                        .map_err(|error| error.to_string())?;
                    Ok(CompletedExport {
                        path,
                        project_name: project.metadata.display_name.clone(),
                        cycles,
                    })
                });
                let _ = sender.send(result);
            }) {
            Ok(_) => {
                status.busy = true;
                status.message = format!("Rendering {cycles} cycles…");
            }
            Err(error) => {
                status.message = format!("Could not start export: {error}");
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn reject_browser_exports(
    mut requests: MessageReader<AudioExportRequest>,
    mut status: ResMut<AudioExportStatus>,
) {
    for _ in requests.read() {
        status.busy = false;
        status.completed = None;
        status.message = "WAV export is available in the desktop app".into();
    }
}

#[cfg(test)]
mod receipt_tests {
    use super::*;
    #[test]
    fn completion_reports_actual_worker_success_or_failure_once() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        register(&mut app);
        app.world()
            .resource::<ExportWorker>()
            .sender
            .send(Ok(CompletedExport {
                path: PathBuf::from("/tmp/completed.wav"),
                project_name: "Test song".into(),
                cycles: 8,
            }))
            .unwrap();
        app.update();
        assert_eq!(
            app.world()
                .resource::<AudioExportStatus>()
                .completed
                .as_ref()
                .unwrap()
                .cycles,
            8
        );
        let message = app.world().resource::<AudioExportStatus>().message.clone();
        app.update();
        assert_eq!(app.world().resource::<AudioExportStatus>().message, message);
        app.world()
            .resource::<ExportWorker>()
            .sender
            .send(Err("Disk full".into()))
            .unwrap();
        app.update();
        assert!(
            app.world()
                .resource::<AudioExportStatus>()
                .message
                .contains("Disk full")
        );
        assert!(!app.world().resource::<AudioExportStatus>().busy);
        assert!(
            app.world()
                .resource::<AudioExportStatus>()
                .completed
                .is_none()
        );
    }
}
