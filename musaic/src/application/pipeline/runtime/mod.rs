use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::state::condition::in_state;
use cadence::bevy::CadenceSet;
use cadence::prelude::{CadenceCompiler, PreparedScore, Span, Time as CycleTime};
use serde::{Deserialize, Serialize};
use tessera::prelude::PatternIr;

use crate::{
    infrastructure::app::{MusaicSet, TransportMode},
    infrastructure::diagnostics::{
        AppDiagnostic, DiagnosticPhase, DiagnosticStore, HostDiagnostic, RuntimeDiagnostic,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectedEventId(pub u64);

pub mod activity;
pub mod feedback;
pub mod preview_snapshot;
pub mod provenance;
pub use preview_snapshot::{RuntimePreviewSnapshot, TimelineEventKind, TimelineEventRecord};
pub use provenance::TimelineProvenanceStore;

#[cfg(test)]
mod preview_navigation_tests;

#[derive(Resource, Debug, Clone, Default)]
pub struct RuntimeState {
    pub compiled: Option<CompiledProject>,
    pub proposed: Option<(u64, u64, CompiledProject)>,
    pub pending_ir: Option<PatternIr>,
    pub pending_activity_routing: Option<activity::ActivityRouting>,
    pub diagnostics: Vec<AppDiagnostic>,
    pub revisions: PipelineRevisions,
    pub transport_request: Option<TransportRequest>,
}

#[derive(Debug, Clone, Copy)]
pub enum TransportRequest {
    Seek(CycleTime),
    Stop,
    Panic,
}

impl RuntimeState {
    pub fn mark_full_rebuild(&mut self) {
        self.mark_composition_changed();
    }

    pub fn mark_composition_changed(&mut self) {
        let revision = self.revisions.advance();
        self.revisions.document = revision;
        self.revisions.presentation = revision;
        self.revisions.transport = revision;
    }

    pub fn mark_sound_changed(&mut self) {
        let revision = self.revisions.advance();
        self.revisions.sound = revision;
        self.revisions.transport = revision;
        if self.pending_ir.is_none() {
            self.pending_ir = self
                .proposed
                .as_ref()
                .map(|(_, _, compiled)| compiled.tessera_ir.clone())
                .or_else(|| {
                    self.compiled
                        .as_ref()
                        .map(|compiled| compiled.tessera_ir.clone())
                });
        }
    }

    pub fn mark_presentation_changed(&mut self) {
        let revision = self.revisions.advance();
        self.revisions.presentation = revision;
    }

    pub fn mark_transport_changed(&mut self) {
        let revision = self.revisions.advance();
        self.revisions.transport = revision;
    }

    pub fn needs_compile(&self) -> bool {
        self.revisions.attempted_compilation < self.revisions.document
    }

    pub fn needs_projection(&self) -> bool {
        self.revisions.rendered_projection
            < self.revisions.accepted_audio.max(self.revisions.transport)
    }

    pub fn needs_scene(&self) -> bool {
        self.revisions.rendered_presentation < self.revisions.presentation
    }
}

#[derive(Debug, Clone)]
pub struct CompiledProject {
    pub activity_routing: activity::ActivityRouting,
    pub tessera_ir: PatternIr,
    pub scores: BTreeMap<String, PreparedScore>,
    pub playback_score: PreparedScore,
    pub source_nodes: BTreeMap<u64, tessera::prelude::NodeId>,
}

#[derive(Debug, Clone, Copy)]
pub struct PipelineRevisions {
    next: u64,
    pub document: u64,
    pub sound: u64,
    pub presentation: u64,
    pub transport: u64,
    pub attempted_compilation: u64,
    pub attempted_preparation: u64,
    pub proposed_audio: u64,
    pub accepted_audio: u64,
    pub rendered_projection: u64,
    pub rendered_presentation: u64,
}

impl Default for PipelineRevisions {
    fn default() -> Self {
        Self {
            next: 1,
            document: 1,
            sound: 1,
            presentation: 1,
            transport: 1,
            attempted_compilation: 0,
            attempted_preparation: 0,
            proposed_audio: 0,
            accepted_audio: 0,
            rendered_projection: 0,
            rendered_presentation: 0,
        }
    }
}

impl PipelineRevisions {
    fn advance(&mut self) -> u64 {
        self.next = self.next.saturating_add(1);
        self.next
    }
}

/// Registers the document → sound stages: tessera compile, preview projection, playback sync.
///
/// Called by the playback plugin; not a standalone plugin.
///
/// Frame order inside host sets (see `docs/ARCHITECTURE.md`):
/// - `MusaicSet::Compile`: document → Tessera compiler → current-language IR
/// - `MusaicSet::Runtime`: [`CadenceSet::ReplaceScores`] → transport →
///   [`CadenceSet::Tick`] → clock readback → preview
pub fn register_runtime(app: &mut App) {
    app.init_resource::<TimelineProvenanceStore>()
        .init_resource::<activity::PlaybackActivity>()
        .init_resource::<activity::ActivityCache>()
        .init_resource::<RuntimePreviewSnapshot>()
        .add_systems(
            Update,
            compile_document
                .in_set(MusaicSet::Compile)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        )
        .add_systems(
            Update,
            (
                accept_prepared_revision
                    .after(CadenceSet::ReplaceScores)
                    .before(CadenceSet::Tick),
                activity::reset_activity_for_transport_changes,
                sync_transport_to_playback
                    .after(CadenceSet::ReplaceScores)
                    .before(CadenceSet::Tick),
                sync_playback_to_transport_clock.after(CadenceSet::Tick),
                mark_preview_dirty_while_playing,
                preview_runtime_for_editor,
                feedback::sync_feedback,
                activity::update_playback_activity,
            )
                .chain()
                .in_set(MusaicSet::Runtime)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
}

/// Publish preview/source links only after Cadence accepts the corresponding
/// score. An audio preparation rejection keeps the entire previous revision.
fn accept_prepared_revision(
    sync: Res<cadence::bevy::PlaybackSync>,
    cadence_diagnostics: Res<cadence::bevy::CadenceDiagnostics>,
    mut runtime: ResMut<RuntimeState>,
    mut active_scores: ResMut<cadence::bevy::ActiveScores>,
    mut diagnostics: ResMut<DiagnosticStore>,
) {
    let Some((cadence_revision, _, _)) = runtime.proposed.as_ref() else {
        return;
    };
    if sync.last_applied_revision == *cadence_revision {
        let (_, source_revision, compiled) = runtime.proposed.take().expect("checked proposal");
        runtime.compiled = Some(compiled);
        runtime.revisions.accepted_audio = source_revision;
        runtime.mark_transport_changed();
    } else if let Some(error) = &cadence_diagnostics.last_tick_error {
        runtime.proposed = None;
        let (output_count, prepared) = runtime
            .compiled
            .as_ref()
            .map(|compiled| (compiled.scores.len(), compiled.playback_score.clone()))
            .unwrap_or_else(|| (0, PreparedScore::default()));
        active_scores.replace(output_count, prepared);
        diagnostics.replace_phase(
            DiagnosticPhase::Lowering,
            [AppDiagnostic::Lowering(
                crate::infrastructure::diagnostics::LoweringDiagnostic::UnsupportedPatternNode {
                    node: format!("Audio rejected this revision: {error}"),
                },
            )],
        );
    }
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

fn compile_document(
    project: Res<'_, crate::application::session::MusaicProject>,
    mut runtime: ResMut<'_, RuntimeState>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
) {
    if !runtime.needs_compile() {
        return;
    }
    let revision = runtime.revisions.document;
    runtime.revisions.attempted_compilation = revision;
    match crate::application::compile::compile_project_ir(&project) {
        Ok(ir) => {
            runtime.pending_activity_routing =
                Some(activity::ActivityRouting::from_document(&project.document));
            runtime.pending_ir = Some(ir);
            diagnostics.clear_phase(DiagnosticPhase::TesseraCompile);
        }
        Err(errors) => {
            diagnostics.replace_phase(
                DiagnosticPhase::TesseraCompile,
                errors.into_iter().map(|error| {
                    AppDiagnostic::Host(HostDiagnostic::BoardExportFailed {
                        detail: error.message,
                    })
                }),
            );
        }
    }
}

fn sync_transport_to_playback(
    project: Res<'_, crate::application::session::MusaicProject>,
    transport_mode: Res<'_, State<TransportMode>>,
    mut playback: NonSendMut<'_, cadence::infrastructure::playback::PlaybackRuntime>,
    mut runtime: ResMut<'_, RuntimeState>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
) {
    let result = playback
        .set_cps(project.document.playback.cycles_per_second())
        .and_then(|()| {
            if let Some(request) = runtime.transport_request.take() {
                return match request {
                    TransportRequest::Seek(position) => playback.seek(position),
                    TransportRequest::Stop => playback.stop(),
                    TransportRequest::Panic => playback.panic(),
                };
            }
            match transport_mode.get() {
                TransportMode::Playing => playback.resume(),
                TransportMode::Paused => playback.pause(),
                TransportMode::Stopped => {
                    // A completed stopped seek is a paused audio position, ready for Play.
                    if playback.status().state
                        == cadence::infrastructure::playback::PlaybackState::Playing
                    {
                        playback.stop()
                    } else {
                        Ok(())
                    }
                }
            }
        });
    if let Err(error) = result {
        diagnostics.replace_phase(
            DiagnosticPhase::Runtime,
            [AppDiagnostic::Runtime(
                RuntimeDiagnostic::ProjectionFailed {
                    detail: format!("Playback: {error:?}"),
                },
            )],
        );
    }
}

fn mark_preview_dirty_while_playing(
    transport_mode: Res<'_, State<TransportMode>>,
    clock: Res<'_, crate::application::editor::transport::TransportClock>,
    timeline: Res<'_, crate::application::editor::TimelinePanelState>,
    mut runtime: ResMut<'_, RuntimeState>,
    mut last_cycle: Local<Option<i64>>,
) {
    if *transport_mode.get() != TransportMode::Playing || timeline.preview_cycle.is_some() {
        *last_cycle = None;
        return;
    }
    // A fitted cycle has stable events until its boundary. The playhead moves
    // separately; rebuilding buttons every audio tick also loses keyboard focus.
    let cycle = clock.position.value().floor() as i64;
    if *last_cycle != Some(cycle) {
        *last_cycle = Some(cycle);
        runtime.mark_transport_changed();
    }
}

fn preview_runtime_for_editor(
    playback: NonSend<cadence::infrastructure::playback::PlaybackRuntime>,
    project: Res<'_, crate::application::session::MusaicProject>,
    mut provenance: ResMut<'_, TimelineProvenanceStore>,
    mut runtime: ResMut<'_, RuntimeState>,
    clock: Res<'_, crate::application::editor::transport::TransportClock>,
    timeline: Res<'_, crate::application::editor::TimelinePanelState>,
    mut diagnostics: ResMut<'_, DiagnosticStore>,
    mut preview_snapshot: ResMut<'_, RuntimePreviewSnapshot>,
) {
    if !runtime.needs_projection() {
        return;
    }

    preview_snapshot.clear();
    provenance.clear();

    let position = timeline
        .preview_cycle
        .map(|cycle| CycleTime::new(i64::from(cycle), 1))
        .unwrap_or_else(|| playback.pending_revision_cycle().unwrap_or(clock.position));
    let start = CycleTime::new(position.value().floor() as i64, 1);
    let window = match Span::new(start, start + CycleTime::ONE) {
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
            runtime.revisions.rendered_projection = runtime
                .revisions
                .accepted_audio
                .max(runtime.revisions.transport);
            return;
        }
    };

    preview_snapshot.window = Some(window);
    preview_snapshot.browsing = timeline.preview_cycle.is_some();
    preview_snapshot.pending_cycle = playback.pending_revision_cycle();
    let Some(compiled) = runtime.compiled.as_ref() else {
        runtime.revisions.rendered_projection = runtime
            .revisions
            .accepted_audio
            .max(runtime.revisions.transport);
        diagnostics.clear_phase(DiagnosticPhase::Runtime);
        return;
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

    provenance.rebuild(&preview_snapshot, &compiled.source_nodes, &project.document);
    diagnostics.replace_phase(DiagnosticPhase::Runtime, runtime_diagnostics);
    runtime.revisions.rendered_projection = runtime
        .revisions
        .accepted_audio
        .max(runtime.revisions.transport);
}

#[cfg(test)]
mod revision_tests {
    use super::*;
    use cadence::bevy::{ActiveScores, CadenceDiagnostics, PlaybackSync};
    use cadence::prelude::Score;
    use tessera::prelude::NodeId;

    fn compiled(name: &str) -> CompiledProject {
        CompiledProject {
            activity_routing: Default::default(),
            tessera_ir: PatternIr::default(),
            scores: BTreeMap::from([(name.into(), PreparedScore::new(Score::empty()).unwrap())]),
            playback_score: PreparedScore::default(),
            source_nodes: BTreeMap::from([(0, NodeId::new(name))]),
        }
    }
    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<RuntimeState>()
            .init_resource::<ActiveScores>()
            .init_resource::<PlaybackSync>()
            .init_resource::<CadenceDiagnostics>()
            .init_resource::<DiagnosticStore>()
            .add_systems(Update, accept_prepared_revision);
        app
    }

    #[test]
    fn preview_and_sources_wait_for_the_corresponding_audio_revision() {
        let mut app = app();
        {
            let mut runtime = app.world_mut().resource_mut::<RuntimeState>();
            runtime.compiled = Some(compiled("old-note"));
            runtime.proposed = Some((7, 2, compiled("new-note")));
        }
        app.world_mut()
            .resource_mut::<PlaybackSync>()
            .last_applied_revision = 6;
        app.update();
        assert_eq!(
            app.world()
                .resource::<RuntimeState>()
                .compiled
                .as_ref()
                .unwrap()
                .source_nodes[&0],
            NodeId::new("old-note")
        );
        app.world_mut()
            .resource_mut::<PlaybackSync>()
            .last_applied_revision = 7;
        app.update();
        let runtime = app.world().resource::<RuntimeState>();
        assert!(runtime.proposed.is_none());
        assert_eq!(
            runtime.compiled.as_ref().unwrap().source_nodes[&0],
            NodeId::new("new-note")
        );
        assert!(runtime.needs_projection());
    }

    #[test]
    fn audio_rejection_keeps_the_previous_music_and_tile_links() {
        let mut app = app();
        {
            let mut runtime = app.world_mut().resource_mut::<RuntimeState>();
            runtime.compiled = Some(compiled("old-note"));
            runtime.proposed = Some((7, 2, compiled("rejected-note")));
        }
        app.world_mut().resource_mut::<ActiveScores>().revision = 7;
        app.world_mut()
            .resource_mut::<CadenceDiagnostics>()
            .last_tick_error = Some("unsupported control".into());
        app.update();
        let runtime = app.world().resource::<RuntimeState>();
        assert!(runtime.proposed.is_none());
        assert_eq!(
            runtime.compiled.as_ref().unwrap().source_nodes[&0],
            NodeId::new("old-note")
        );
        assert_eq!(app.world().resource::<ActiveScores>().output_count, 1);
        assert!(app.world().resource::<DiagnosticStore>().has_errors());
    }
}
