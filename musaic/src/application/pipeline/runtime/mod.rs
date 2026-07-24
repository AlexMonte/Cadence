use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::state::condition::in_state;
use cadence::prelude::{CadenceCompiler, Score, Span, Time as CycleTime};
use serde::{Deserialize, Serialize};
use cadence::bevy::CadenceSet;
use tessera::{
    bevy::{CompileRequested, CompiledIr, TesseraBoard, TesseraDiagnostics, TesseraSystems},
    prelude::{BoardError, PatternIr},
};

use crate::{
    domain::document::hydrate_board_from_document,
    infrastructure::app::{MusaicSet, TransportMode},
    infrastructure::diagnostics::{
        AppDiagnostic, DiagnosticPhase, DiagnosticStore, HostDiagnostic, RuntimeDiagnostic,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectedEventId(pub u64);

pub mod preview_snapshot;
pub mod provenance;
pub use preview_snapshot::{RuntimePreviewSnapshot, TimelineEventKind, TimelineEventRecord};
pub use provenance::TimelineProvenanceStore;

#[derive(Resource, Debug, Clone)]
pub struct RuntimeState {
    pub compiled: Option<CompiledProject>,
    pub diagnostics: Vec<AppDiagnostic>,
    pub dirty: ProjectDirty,
}

impl RuntimeState {
    pub fn mark_full_rebuild(&mut self) {
        self.dirty = ProjectDirty {
            tessera: true,
            lowering: true,
            scene: true,
            runtime: true,
            awaiting_compile: false,
            board_export: true,
        };
    }
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            compiled: None,
            diagnostics: Vec::new(),
            dirty: ProjectDirty {
                tessera: true,
                lowering: false,
                scene: true,
                runtime: false,
                awaiting_compile: true,
                board_export: true,
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompiledProject {
    pub tessera_ir: PatternIr,
    pub scores: BTreeMap<String, Score>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ProjectDirty {
    pub tessera: bool,
    pub lowering: bool,
    pub scene: bool,
    pub runtime: bool,
    pub awaiting_compile: bool,
    /// When true, the document graph must be exported into [`TesseraBoard`] before compile.
    pub board_export: bool,
}

/// Registers the document → sound stages: tessera compile, preview projection, playback sync.
///
/// Called by the playback plugin; not a standalone plugin.
///
/// Frame order inside host sets (see `docs/ARCHITECTURE.md`):
/// - `MusaicSet::Compile`: document→board → [`TesseraSystems`] → collect IR
/// - `MusaicSet::Runtime`: [`CadenceSet::ReplaceScores`] → transport →
///   [`CadenceSet::Tick`] → clock readback → preview
pub fn register_runtime(app: &mut App) {
    app.init_resource::<TimelineProvenanceStore>()
        .init_resource::<RuntimePreviewSnapshot>()
        .add_systems(
            Update,
            (
                sync_document_to_tessera_board.before(TesseraSystems),
                collect_tessera_compile_output.after(TesseraSystems),
            )
                .in_set(MusaicSet::Compile)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        )
        .add_systems(
            Update,
            (
                sync_transport_to_playback
                    .after(CadenceSet::ReplaceScores)
                    .before(CadenceSet::Tick),
                sync_playback_to_transport_clock.after(CadenceSet::Tick),
                mark_preview_dirty_while_playing,
                preview_runtime_for_editor,
            )
                .chain()
                .in_set(MusaicSet::Runtime)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
}

/// While playing, Cadence is the clock master — copy its cycle position into the editor clock.
fn sync_playback_to_transport_clock(
    transport_mode: Res<'_, State<TransportMode>>,
    playback: NonSend<'_, cadence::infrastructure::playback::PlaybackRuntime>,
    mut clock: ResMut<'_, crate::application::editor::transport::TransportClock>,
) {
    if *transport_mode.get() != TransportMode::Playing {
        return;
    }
    clock.position = playback.status().cycle_position;
}

fn sync_document_to_tessera_board(
    project: Res<'_, crate::application::session::MusaicProject>,
    mut runtime: ResMut<'_, RuntimeState>,
    mut board: ResMut<'_, TesseraBoard>,
    mut compile_requests: MessageWriter<'_, CompileRequested>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
) {
    if !runtime.dirty.tessera {
        return;
    }

    if runtime.dirty.board_export {
        if let Err(error) = hydrate_board_from_document(&mut board, &project.document) {
            diagnostics.replace_phase(
                DiagnosticPhase::TesseraCompile,
                [AppDiagnostic::Host(HostDiagnostic::BoardExportFailed {
                    detail: format_board_error(&error),
                })],
            );
            runtime.dirty.tessera = false;
            runtime.dirty.awaiting_compile = false;
            return;
        }
        runtime.dirty.board_export = false;
    }

    // Document tessera.authored_program is updated by command transactions only.
    // Hydrate already projected document → board; compile from the live board.
    compile_requests.write(CompileRequested::forced());
    runtime.dirty.tessera = false;
    runtime.dirty.awaiting_compile = true;
}

fn format_board_error(error: &BoardError) -> String {
    match error {
        BoardError::SlotOccupied => "That cell is already occupied.".into(),
        BoardError::UnknownTile => "That tile was removed or moved.".into(),
        BoardError::DuplicateId { existing_slot } => {
            format!(
                "Id already used at ({}, {}).",
                existing_slot.x, existing_slot.y
            )
        }
    }
}

fn collect_tessera_compile_output(
    mut runtime: ResMut<'_, RuntimeState>,
    compiled: Res<'_, CompiledIr>,
    tessera_diagnostics: Res<'_, TesseraDiagnostics>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
) {
    if !runtime.dirty.awaiting_compile {
        return;
    }

    if !tessera_diagnostics.0.is_empty() {
        diagnostics.replace_phase(
            DiagnosticPhase::TesseraCompile,
            tessera_diagnostics
                .0
                .iter()
                .cloned()
                .map(AppDiagnostic::Tessera),
        );
        runtime.compiled = None;
        runtime.dirty.awaiting_compile = false;
        return;
    }

    let Some(ir) = compiled.0.clone() else {
        return;
    };

    runtime.compiled = Some(CompiledProject {
        tessera_ir: ir,
        scores: BTreeMap::new(),
    });
    runtime.dirty.awaiting_compile = false;
    runtime.dirty.lowering = true;
    diagnostics.clear_phase(DiagnosticPhase::TesseraCompile);
}

fn sync_transport_to_playback(
    project: Res<'_, crate::application::session::MusaicProject>,
    transport_mode: Res<'_, State<TransportMode>>,
    mut playback: NonSendMut<'_, cadence::infrastructure::playback::PlaybackRuntime>,
) {
    let bpm = project.document.playback.bpm.max(1.0);
    let cps = CycleTime::new((bpm as i64).max(1), 60);
    let _ = playback.set_cps(cps);

    match transport_mode.get() {
        TransportMode::Playing => {
            let _ = playback.resume();
        }
        TransportMode::Stopped => {
            let _ = playback.stop();
        }
    }
}

fn mark_preview_dirty_while_playing(
    transport_mode: Res<'_, State<TransportMode>>,
    clock: Res<'_, crate::application::editor::transport::TransportClock>,
    mut runtime: ResMut<'_, RuntimeState>,
    mut last_position: Local<Option<CycleTime>>,
) {
    if *transport_mode.get() != TransportMode::Playing {
        *last_position = None;
        return;
    }
    if last_position.as_ref() != Some(&clock.position) {
        *last_position = Some(clock.position);
        runtime.dirty.runtime = true;
    }
}

fn preview_runtime_for_editor(
    mut runtime: ResMut<'_, RuntimeState>,
    clock: Res<'_, crate::application::editor::transport::TransportClock>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
    mut preview_snapshot: ResMut<'_, RuntimePreviewSnapshot>,
) {
    if !runtime.dirty.runtime {
        return;
    }

    let Some(compiled) = runtime.compiled.as_ref() else {
        runtime.dirty.runtime = false;
        diagnostics.clear_phase(DiagnosticPhase::Runtime);
        return;
    };

    if compiled.scores.is_empty() {
        preview_snapshot.clear();
        runtime.dirty.runtime = false;
        diagnostics.clear_phase(DiagnosticPhase::Runtime);
        return;
    }

    preview_snapshot.clear();

    let position = clock.position;
    let window = match Span::new(position, position + CycleTime::new(1, 16)) {
        Some(window) => window,
        None => {
            diagnostics.replace_phase(
                DiagnosticPhase::Runtime,
                [AppDiagnostic::Runtime(
                    RuntimeDiagnostic::ProjectionFailed {
                        detail: "invalid editor preview window".into(),
                    },
                )],
            );
            runtime.dirty.runtime = false;
            return;
        }
    };

    let compiler = CadenceCompiler::new();
    let mut runtime_diagnostics = Vec::new();
    for (output_id, score) in &compiled.scores {
        match compiler.preview(score, &window) {
            Ok(report) => preview_snapshot.ingest_report(output_id, &report),
            Err(error) => runtime_diagnostics.push(AppDiagnostic::Runtime(
                RuntimeDiagnostic::ProjectionFailed {
                    detail: format!("output {output_id}: {error:?}"),
                },
            )),
        }
    }

    diagnostics.replace_phase(DiagnosticPhase::Runtime, runtime_diagnostics);
    runtime.dirty.runtime = false;
}
