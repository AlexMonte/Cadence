//! Command bus — the single mutation authority for editor application state.
//!
//! Producers emit [`EditorCommandBus`]; [`dispatch_commands`] is the only writer
//! of document / attention / selection / session / view-settings /
//! project replacement / save metadata. Pipeline revisions on [`RuntimeState`]
//! are advanced here via invalidation; compiled IR is written only by pipeline stages.
//! Panel chrome width stays UI-local (declared exception).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::state::condition::in_state;

use super::{EditorCommand, execute_command, execute_inverse};
mod samples;
mod sound_library;
use crate::adapter::persistence::{editor_save_project, load_project};
use crate::application::board_view_settings::BoardViewSettings;
use crate::application::editor::interaction::{apply_invalidation, push_transaction_diagnostics};
use crate::application::editor::transaction::{EditorTransactionResult, Invalidation};
use crate::application::editor::{
    ActiveSurfaceChanged, DrawerPanelState, EditorAttention, EditorSession, MinimapPanelState,
    SelectionState, TimelinePanelState,
};
use crate::application::history::{CommandHistory, HistoryEntry, HistoryPolicy};
use crate::application::pipeline::runtime::{RuntimeState, TimelineProvenanceStore};
use crate::application::session::{MusaicProject, ProjectSession};
use crate::infrastructure::diagnostics::DiagnosticStore;
use samples::{ActiveSampleOptionsEdit, SampleImports};

#[derive(Message, Debug, Clone)]
pub struct EditorCommandBus(pub EditorCommand);

#[derive(Resource, Default)]
struct ActiveAtomEdit {
    node: Option<tessera::prelude::NodeId>,
    recorded: bool,
}

#[derive(Resource, Default)]
struct ActiveSoundEdit(ActiveAtomEdit);

/// Mutable handles owned by the command dispatcher — one SystemParam instead of
/// a 16-argument free function.
#[derive(SystemParam)]
struct CommandContext<'w> {
    export_status: ResMut<'w, crate::application::audio_export::AudioExportStatus>,
    file_feedback: MessageWriter<'w, crate::application::audio_export::FileLocationFeedback>,
    connection_receipts: MessageWriter<'w, super::connection::ConnectionReceipt>,
    note_receipts: MessageWriter<'w, super::note_entry::NoteEntryReceipt>,
    clipboard: ResMut<'w, super::editing::TileClipboard>,
    #[cfg(not(target_arch = "wasm32"))]
    recovery_changes:
        MessageWriter<'w, crate::adapter::persistence::recovery::RecoveryProjectChanged>,
    #[cfg(not(target_arch = "wasm32"))]
    recovery_lifecycle: ResMut<'w, crate::adapter::persistence::recovery::RecoveryLifecycle>,
    export_requests: MessageWriter<'w, crate::application::audio_export::AudioExportRequest>,
    preview_epoch: ResMut<'w, crate::adapter::audio::AudioPreviewEpoch>,
    auditions: MessageWriter<'w, crate::adapter::audio::AuditionRequest>,
    sound_edit: ResMut<'w, ActiveSoundEdit>,
    sample_options_edit: ResMut<'w, ActiveSampleOptionsEdit>,
    imports: ResMut<'w, SampleImports>,
    atom_edit: ResMut<'w, ActiveAtomEdit>,
    project: ResMut<'w, MusaicProject>,
    runtime: ResMut<'w, RuntimeState>,
    session_meta: ResMut<'w, ProjectSession>,
    attention: ResMut<'w, EditorAttention>,
    selection: ResMut<'w, SelectionState>,
    provenance: ResMut<'w, TimelineProvenanceStore>,
    active_scores: ResMut<'w, cadence::bevy::ActiveScores>,
    preview: ResMut<'w, crate::application::pipeline::runtime::RuntimePreviewSnapshot>,
    history: ResMut<'w, CommandHistory>,
    session: ResMut<'w, EditorSession>,
    drawer_panel: ResMut<'w, DrawerPanelState>,
    minimap_panel: ResMut<'w, MinimapPanelState>,
    timeline_panel: ResMut<'w, TimelinePanelState>,
    view_settings: ResMut<'w, BoardViewSettings>,
    diagnostics: ResMut<'w, DiagnosticStore>,
    surface_events: MessageWriter<'w, ActiveSurfaceChanged>,
    transport_mode: ResMut<'w, NextState<crate::infrastructure::app::TransportMode>>,
    current_transport: Res<'w, State<crate::infrastructure::app::TransportMode>>,
    clock: ResMut<'w, crate::application::editor::transport::TransportClock>,
}

impl CommandContext<'_> {
    fn record_mutation(&mut self, entry: HistoryEntry) {
        self.history.record_mutation(entry);
    }
}

/// Registers the command bus and its dispatch stage. Called by the editor plugin.
pub fn register_command_bus(app: &mut App) {
    #[cfg(not(target_arch = "wasm32"))]
    app.init_resource::<crate::adapter::persistence::recovery::RecoveryLifecycle>()
        .add_message::<crate::adapter::persistence::recovery::RecoveryProjectChanged>();
    crate::application::audio_export::register(app);
    app.init_resource::<super::editing::TileClipboard>()
        .init_resource::<CommandHistory>()
        .init_resource::<crate::adapter::audio::AudioPreviewEpoch>()
        .init_resource::<ActiveSoundEdit>()
        .init_resource::<SampleImports>()
        .init_resource::<ActiveSampleOptionsEdit>()
        .init_resource::<ActiveAtomEdit>()
        .add_message::<EditorCommandBus>()
        .add_message::<super::note_entry::NoteEntryReceipt>()
        .add_message::<super::connection::ConnectionReceipt>()
        .add_message::<crate::adapter::audio::AuditionRequest>()
        .add_systems(
            Update,
            dispatch_commands
                .after(crate::application::editor::MusaicEditorSet::MutateState)
                .in_set(crate::infrastructure::app::MusaicSet::Commands)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
}

// macOS native Save/Save As dialogs synchronously dispatch to AppKit. Running
// this system on a worker would deadlock against Bevy's main-thread executor.
fn dispatch_commands(
    mut bus: MessageReader<EditorCommandBus>,
    mut ctx: CommandContext,
    #[cfg(target_os = "macos")] _main_thread: bevy::ecs::system::NonSendMarker,
) {
    samples::drain(&mut ctx);
    let commands: Vec<EditorCommand> = bus.read().map(|EditorCommandBus(c)| c.clone()).collect();
    for command in commands {
        if handle_dispatcher_command(&command, &mut ctx) {
            continue;
        }

        let record_history = command.history_policy() == HistoryPolicy::RecordMutation;
        run_command(&command, &mut ctx, record_history);
    }
}

/// Session / tool / panel / transport / project arms that never enter
/// [`execute_command`]. Returns true when the command was fully handled.
fn handle_dispatcher_command(command: &EditorCommand, ctx: &mut CommandContext) -> bool {
    if sound_library::handle(command, ctx) {
        return true;
    }
    if samples::handle(command, ctx) {
        return true;
    }
    let delete_action = super::editing::TileEdit::Delete;
    let editing_action = match command {
        EditorCommand::EditTiles(action) => Some(action.clone()),
        EditorCommand::DeleteSelection => Some(delete_action),
        _ => None,
    };
    if let Some(action) = editing_action {
        let clipboard = &mut *ctx.clipboard;
        let execution = super::editing::execute(
            &mut ctx.project,
            &mut ctx.selection,
            &mut ctx.attention,
            clipboard,
            &action,
        );
        if let super::editing::TileEdit::Connect { request, .. } = &action {
            ctx.connection_receipts
                .write(super::connection::ConnectionReceipt {
                    request: *request,
                    result: execution.as_ref().map(|_| ()).map_err(Clone::clone),
                });
        }
        if let super::editing::TileEdit::InsertNotes { request, .. } = &action {
            ctx.note_receipts
                .write(super::note_entry::NoteEntryReceipt {
                    request: *request,
                    result: execution.as_ref().map(|_| ()).map_err(Clone::clone),
                    continuation: if execution.is_ok() {
                        super::note_entry::continuation(&ctx.project.document, &ctx.selection.nodes)
                    } else {
                        None
                    },
                });
        }
        match execution {
            Ok(outcome) => {
                let mut result = crate::application::editor::EditorTransactionResult::accepted(
                    Invalidation::document_replaced(),
                );
                result.diagnostics = outcome.diagnostics;
                push_transaction_diagnostics(&mut ctx.diagnostics, &result);
                if let Some(record) = outcome.record {
                    ctx.runtime.mark_full_rebuild();
                    ctx.record_mutation(HistoryEntry {
                        forward: command.clone(),
                        inverse: super::EditorInverse::RestoreTileEdit {
                            record: Box::new(record),
                        },
                        invalidation: Invalidation::document_replaced(),
                    });
                }
            }
            Err(error) => push_execution_error(
                &mut ctx.diagnostics,
                crate::domain::DomainError::Message(error),
            ),
        }
        return true;
    }
    match command {
        EditorCommand::ChooseAudioExport { cycles } => {
            if ctx.export_status.busy {
                return true;
            }
            if let Err(error) =
                crate::application::audio_export::export_duration(&ctx.project, *cycles)
            {
                push_execution_error(
                    &mut ctx.diagnostics,
                    crate::domain::DomainError::Message(error),
                );
                return true;
            }
            #[cfg(not(target_arch = "wasm32"))]
            if let Some(path) = rfd::FileDialog::new()
                .set_file_name("musaic.wav")
                .add_filter("WAV audio", &["wav"])
                .save_file()
            {
                ctx.export_requests
                    .write(crate::application::audio_export::AudioExportRequest {
                        project: ctx.project.clone(),
                        path,
                        cycles: *cycles,
                    });
            }
            true
        }
        EditorCommand::RevealProjectFile | EditorCommand::RevealLastExport => {
            let path = if matches!(command, EditorCommand::RevealProjectFile) {
                ctx.session_meta.last_saved_path.clone()
            } else {
                ctx.export_status
                    .completed
                    .as_ref()
                    .map(|receipt| receipt.path.clone())
            };
            let result = path
                .ok_or_else(|| "Save or export a file before showing its location.".to_string())
                .and_then(|path| crate::adapter::persistence::file_location::reveal(&path));
            let message = match result {
                Ok(()) => "File location requested".to_string(),
                Err(error) => {
                    if matches!(command, EditorCommand::RevealLastExport) {
                        ctx.export_status.message = error.clone();
                    }
                    error
                }
            };
            ctx.file_feedback
                .write(crate::application::audio_export::FileLocationFeedback(
                    message,
                ));
            true
        }
        EditorCommand::ExportAudio { path, cycles } => {
            ctx.export_requests
                .write(crate::application::audio_export::AudioExportRequest {
                    project: ctx.project.clone(),
                    path: path.clone(),
                    cycles: *cycles,
                });
            true
        }
        EditorCommand::AuditionSound { sound } => {
            ctx.preview_epoch.0 = ctx.preview_epoch.0.wrapping_add(1);
            let instrument = ctx
                .project
                .document
                .graph
                .sound_definition(sound)
                .cloned()
                .unwrap_or_default();
            let pitch = match instrument.source {
                crate::domain::instrument::InstrumentSource::Synth(_)
                | crate::domain::instrument::InstrumentSource::Preset(_) => Some(60.0),
                crate::domain::instrument::InstrumentSource::Sample(id) => ctx
                    .project
                    .samples
                    .manifest()
                    .samples
                    .get(&id)
                    .and_then(|sample| sample.options.root_pitch),
                crate::domain::instrument::InstrumentSource::Kit
                | crate::domain::instrument::InstrumentSource::Drum(_) => None,
            };
            match instrument.intent() {
                Ok(mut intent) => {
                    if let (
                        crate::domain::instrument::InstrumentSource::Sample(id),
                        cadence::prelude::Intent::Sample(sample),
                    ) = (&instrument.source, &mut intent)
                    {
                        if let Some(gain) = ctx
                            .project
                            .samples
                            .manifest()
                            .samples
                            .get(id)
                            .and_then(|metadata| metadata.options.default_gain)
                        {
                            sample.gain *= f64::from(gain);
                        }
                    }
                    ctx.auditions.write(crate::adapter::audio::AuditionRequest {
                        epoch: ctx.preview_epoch.0,
                        intent,
                        pitch,
                    });
                }
                Err(error) => push_execution_error(
                    &mut ctx.diagnostics,
                    crate::domain::DomainError::Message(error),
                ),
            }
            true
        }
        EditorCommand::BeginSoundEdit { sound } => {
            ctx.sound_edit.0 = ActiveAtomEdit {
                node: Some(sound.clone()),
                recorded: false,
            };
            true
        }
        EditorCommand::EndSoundEdit => {
            *ctx.sound_edit = ActiveSoundEdit::default();
            true
        }
        EditorCommand::SetSound { sound, definition } => {
            set_sound(ctx, sound, definition.clone(), true);
            true
        }
        EditorCommand::BeginAtomEdit { node } => {
            ctx.atom_edit.node = Some(node.clone());
            ctx.atom_edit.recorded = false;
            true
        }
        EditorCommand::EndAtomEdit => {
            *ctx.sample_options_edit = ActiveSampleOptionsEdit::default();
            *ctx.atom_edit = ActiveAtomEdit::default();
            true
        }
        EditorCommand::ToggleDrawer => {
            ctx.drawer_panel.toggle(&ctx.attention);
            true
        }
        EditorCommand::SetDrawerOpen { open } => {
            ctx.drawer_panel.open = Some(*open);
            true
        }
        EditorCommand::ToggleMinimap => {
            ctx.minimap_panel.toggle();
            true
        }
        EditorCommand::PreviewCycle { cycle } => {
            if cycle.is_none_or(|value| value <= TimelinePanelState::MAX_PREVIEW_CYCLE) {
                ctx.timeline_panel.preview_cycle = *cycle;
                ctx.runtime.mark_transport_changed();
            }
            true
        }
        EditorCommand::EnterTimelineMode => {
            // Timing is a companion to authoring, not a mode that disables it.
            ctx.attention.enter_compose();
            ctx.timeline_panel.open = true;
            if ctx.timeline_panel.height < TimelinePanelState::MIN_HEIGHT {
                ctx.timeline_panel.height = TimelinePanelState::DEFAULT_HEIGHT;
            }
            true
        }
        EditorCommand::EnterCompose => {
            ctx.attention.enter_compose();
            ctx.timeline_panel.open = false;
            true
        }
        EditorCommand::ArmPlacementTool { tile } => {
            if let Err(error) = ctx.session.arm(tile.clone()) {
                bevy::log::warn!("{error}");
            }
            true
        }
        EditorCommand::CancelPlacement => {
            let _ = ctx.session.cancel();
            true
        }
        EditorCommand::StartConnection { source } => {
            if let Err(error) = ctx.session.start_connection(source.clone()) {
                bevy::log::warn!("{error}");
            }
            true
        }
        EditorCommand::AbortConnection => {
            let _ = ctx.session.abort_connection();
            true
        }
        EditorCommand::SetViewSettings { scope, mode } => {
            match scope {
                crate::application::board_view_settings::AtomDisplayScope::RootBoard => {
                    ctx.view_settings.root_board = *mode;
                }
                crate::application::board_view_settings::AtomDisplayScope::ContainerPreviewOnRoot => {
                    ctx.view_settings.container_preview_on_root = *mode;
                }
                crate::application::board_view_settings::AtomDisplayScope::ContainerInterior => {
                    ctx.view_settings.container_interior = *mode;
                }
            }
            // Scene rebuild + ui_projection view_settings field dirty inspector paint.
            apply_flags(
                &mut ctx.runtime,
                &Invalidation {
                    scene: true,
                    ..Invalidation::default()
                },
            );
            true
        }
        EditorCommand::NewProject => {
            replace_project(ctx, MusaicProject::new_empty(), None);
            true
        }
        EditorCommand::OpenProject { path } => {
            match load_project(path) {
                Ok(loaded) => {
                    replace_project(ctx, loaded, Some(path.clone()));
                    crate::adapter::persistence::push_recent_project(path);
                }
                Err(error) => {
                    bevy::log::error!("failed to open project: {error}");
                    push_execution_error(
                        &mut ctx.diagnostics,
                        crate::domain::DomainError::Message(format!(
                            "Could not open project: {error}. Your current project is still open."
                        )),
                    );
                }
            }
            true
        }
        EditorCommand::AdoptProject { project, path } => {
            let project = project.clone();
            if let Err(error) = project
                .document
                .validate()
                .and_then(|_| crate::application::outputs::validate(&project.document))
                .and_then(|_| crate::application::tricks::validate(&project.document))
            {
                push_execution_error(
                    &mut ctx.diagnostics,
                    crate::domain::DomainError::Message(error),
                );
            } else {
                replace_project(ctx, project, path.clone());
            }
            true
        }
        EditorCommand::SaveProject => {
            persist_project(ctx, None, false);
            true
        }
        EditorCommand::SaveProjectAs { path } => {
            persist_project(ctx, Some(path.clone()), false);
            true
        }
        EditorCommand::TransportToggle => {
            ctx.preview_epoch.0 = ctx.preview_epoch.0.wrapping_add(1);
            use crate::infrastructure::diagnostics::{
                AppDiagnostic, DiagnosticPhase, RuntimeDiagnostic,
            };
            if *ctx.current_transport.get() == crate::infrastructure::app::TransportMode::Stopped
                && !transport_has_audible_content(&ctx.runtime)
            {
                ctx.diagnostics.replace_phase(
                    DiagnosticPhase::Runtime,
                    [AppDiagnostic::Runtime(
                        RuntimeDiagnostic::ProjectionFailed {
                            detail: "Nothing to play yet — place tiles and connect outputs first."
                                .into(),
                        },
                    )],
                );
                return true;
            }
            ctx.diagnostics.clear_phase(DiagnosticPhase::Runtime);
            crate::application::editor::transport::handle_transport_toggle(
                *ctx.current_transport.get(),
                &mut ctx.transport_mode,
            );
            true
        }
        EditorCommand::Undo => {
            *ctx.sound_edit = ActiveSoundEdit::default();
            *ctx.sample_options_edit = ActiveSampleOptionsEdit::default();
            *ctx.atom_edit = ActiveAtomEdit::default();
            if let Some(entry) = ctx.history.pop_undo() {
                let errors = ctx.diagnostics.rejection_serial;
                run_history_entry(&entry, HistoryDirection::Undo, ctx);
                if ctx.diagnostics.rejection_serial == errors {
                    ctx.history.push_redo(entry);
                } else {
                    ctx.history.push_undo(entry);
                }
            }
            true
        }
        EditorCommand::Redo => {
            *ctx.sound_edit = ActiveSoundEdit::default();
            *ctx.sample_options_edit = ActiveSampleOptionsEdit::default();
            *ctx.atom_edit = ActiveAtomEdit::default();
            if let Some(entry) = ctx.history.pop_redo() {
                let errors = ctx.diagnostics.rejection_serial;
                run_history_entry(&entry, HistoryDirection::Redo, ctx);
                if ctx.diagnostics.rejection_serial == errors {
                    ctx.history.push_undo(entry);
                } else {
                    ctx.history.push_redo(entry);
                }
            }
            true
        }
        cmd if crate::application::editor::transport::handle_transport_command(
            cmd,
            &mut ctx.transport_mode,
            &mut ctx.clock,
            &mut ctx.runtime,
        ) =>
        {
            ctx.preview_epoch.0 = ctx.preview_epoch.0.wrapping_add(1);
            if matches!(
                cmd,
                EditorCommand::TransportStop | EditorCommand::TransportPanic
            ) {
                ctx.timeline_panel.preview_cycle = None;
            }
            // SetBpm is handled in execute_command; transport helper only
            // claims Play/Stop/Seek now.
            true
        }
        _ => false,
    }
}

fn persist_project(
    ctx: &mut CommandContext,
    explicit_path: Option<std::path::PathBuf>,
    force_picker: bool,
) {
    match editor_save_project(
        &ctx.project,
        ctx.session_meta.last_saved_path.as_deref(),
        explicit_path,
        force_picker,
    ) {
        Ok(Some(saved)) => {
            ctx.project.metadata.dirty = false;
            ctx.project.metadata.file_path = Some(saved.path.display().to_string());
            ctx.session_meta.last_saved_path = Some(saved.path);
        }
        Ok(None) => {}
        Err(error) => {
            bevy::log::error!("save failed: {error}");
            push_execution_error(
                &mut ctx.diagnostics,
                crate::domain::DomainError::Message(format!("Could not save project: {error}")),
            );
        }
    }
}

fn replace_project(
    ctx: &mut CommandContext,
    project: MusaicProject,
    path: Option<std::path::PathBuf>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    match ctx.recovery_lifecycle.prepare_transition(&ctx.project) {
        Ok(change) => {
            ctx.recovery_changes.write(change);
        }
        Err(error) => {
            push_execution_error(
                &mut ctx.diagnostics,
                crate::domain::DomainError::Message(error.into()),
            );
            return;
        }
    }
    let previous_surface = ctx.attention.active_board();
    *ctx.clipboard = super::editing::TileClipboard::default();
    *ctx.project = project;
    ctx.preview_epoch.0 = ctx.preview_epoch.0.wrapping_add(1);
    ctx.imports.replace_project();
    ctx.session_meta.last_saved_path = path;
    ctx.history.clear();
    *ctx.sample_options_edit = ActiveSampleOptionsEdit::default();
    *ctx.sound_edit = ActiveSoundEdit::default();
    *ctx.atom_edit = ActiveAtomEdit::default();
    ctx.selection.clear();
    *ctx.session = EditorSession::default();
    *ctx.attention = EditorAttention::new(ctx.project.document.root_surface);
    // Root IDs are document-local and commonly equal across different files.
    // Opening a new document must still reset navigation and frame its contents.
    ctx.surface_events.write(ActiveSurfaceChanged {
        previous: previous_surface,
        current: ctx.project.document.root_surface,
        reason: crate::application::editor::ActiveSurfaceChangeReason::OpenDocument,
    });
    *ctx.runtime = RuntimeState::default();
    ctx.active_scores
        .replace(0, cadence::prelude::PreparedScore::default());
    ctx.preview.clear();
    ctx.provenance.clear();
    ctx.runtime.mark_full_rebuild();
    ctx.runtime.transport_request =
        Some(crate::application::pipeline::runtime::TransportRequest::Stop);
    ctx.transport_mode
        .set(crate::infrastructure::app::TransportMode::Stopped);
    ctx.clock.seek(cadence::prelude::Time::ZERO);
    ctx.timeline_panel.preview_cycle = None;
}

enum HistoryDirection {
    Undo,
    Redo,
}

fn run_history_entry(entry: &HistoryEntry, direction: HistoryDirection, ctx: &mut CommandContext) {
    if let super::EditorInverse::RestoreTileEdit { record } = &entry.inverse {
        match super::editing::restore(
            &mut ctx.project,
            record,
            matches!(direction, HistoryDirection::Redo),
        ) {
            Ok(()) => {
                ctx.selection
                    .retain_existing(|id| ctx.project.document.graph.contains_node(id));
                // Renaming shared code changes no tile identities. Keep a valid
                // inspected tile so Undo does not discard the user's source context.
                let preserve_focus = matches!(
                    &entry.forward,
                    EditorCommand::EditTiles(super::editing::TileEdit::RenameTrick { .. })
                ) && matches!(&ctx.attention.focus, crate::application::editor::FocusTarget::Tile { node } | crate::application::editor::FocusTarget::Atom { node }
                        if ctx.project.document.graph.location_of(node).is_some_and(|site| site.surface == ctx.attention.active_board()));
                if !preserve_focus {
                    ctx.attention.focus = crate::application::editor::FocusTarget::None;
                }
                if !ctx
                    .project
                    .document
                    .surfaces
                    .contains(ctx.attention.active_board())
                {
                    let previous = ctx.attention.active_board();
                    *ctx.attention = EditorAttention::new(ctx.project.document.root_surface);
                    ctx.surface_events.write(ActiveSurfaceChanged {
                        previous,
                        current: ctx.project.document.root_surface,
                        reason: crate::application::editor::ActiveSurfaceChangeReason::DeleteFallbackToRoot,
                    });
                }
                ctx.runtime.mark_full_rebuild();
            }
            Err(error) => push_execution_error(
                &mut ctx.diagnostics,
                crate::domain::DomainError::Message(error),
            ),
        }
        return;
    }
    if sound_library::restore(entry, &direction, ctx) {
        return;
    }
    if samples::restore(entry, &direction, ctx) {
        return;
    }
    if let super::EditorInverse::RestoreSound { sound, definition } = &entry.inverse {
        let value = match direction {
            HistoryDirection::Undo => definition.clone(),
            HistoryDirection::Redo => match &entry.forward {
                EditorCommand::SetSound { definition, .. } => definition.clone(),
                _ => return,
            },
        };
        set_sound(ctx, sound, value, false);
        return;
    }
    let result = match direction {
        HistoryDirection::Undo => execute_inverse(
            &mut ctx.project.document,
            &mut ctx.attention,
            &mut ctx.selection,
            &entry.inverse,
        ),
        HistoryDirection::Redo => execute_command(
            &mut ctx.project.document,
            &mut ctx.attention,
            &mut ctx.selection,
            &*ctx.provenance,
            &entry.forward,
        ),
    };

    match result {
        Ok(mut result) => {
            if result.is_accepted() {
                // Honor the invalidation stored when the forward mutation was
                // recorded — do not trust the inverse's recomputed flags alone.
                if entry.invalidation.save {
                    ctx.project.mark_dirty();
                }
                apply_flags(&mut ctx.runtime, &entry.invalidation);
                if let Some(change) = result.active_surface_change.take() {
                    ctx.surface_events.write(change);
                }
            }
            push_transaction_diagnostics(&mut ctx.diagnostics, &result);
        }
        Err(error) => push_execution_error(&mut ctx.diagnostics, error),
    }
}

fn set_sound(
    ctx: &mut CommandContext,
    sound: &tessera::prelude::NodeId,
    definition: crate::domain::instrument::InstrumentDefinition,
    record: bool,
) {
    use crate::domain::document::{DocumentNodeKind, DocumentQueries};
    if !matches!(
        DocumentQueries::new(&ctx.project.document).node_kind(sound),
        Some(DocumentNodeKind::Sound(_))
    ) {
        push_execution_error(
            &mut ctx.diagnostics,
            crate::domain::DomainError::Message("Choose a Sound tile".into()),
        );
        return;
    }
    if let Err(error) = definition.validate() {
        push_execution_error(
            &mut ctx.diagnostics,
            crate::domain::DomainError::Message(error),
        );
        return;
    }
    let Some(previous) = ctx.project.sound_definition(sound).cloned() else {
        return;
    };
    if previous == definition {
        return;
    }
    if let Err(error) = ctx.project.set_sound_definition(sound, definition.clone()) {
        push_execution_error(
            &mut ctx.diagnostics,
            crate::domain::DomainError::Message(error),
        );
        return;
    }
    ctx.project.mark_dirty();
    ctx.runtime.mark_full_rebuild();
    if record {
        let entry = HistoryEntry {
            forward: EditorCommand::SetSound {
                sound: sound.clone(),
                definition,
            },
            inverse: super::EditorInverse::RestoreSound {
                sound: sound.clone(),
                definition: previous,
            },
            invalidation: Invalidation {
                save: true,
                ..Invalidation::default()
            },
        };
        let in_gesture = ctx.sound_edit.0.node.as_ref() == Some(sound);
        if in_gesture && ctx.sound_edit.0.recorded {
            ctx.history.continue_sound_edit(entry);
        } else {
            ctx.record_mutation(entry);
        }
        if in_gesture {
            ctx.sound_edit.0.recorded = true;
        }
    }
}

fn run_command(command: &EditorCommand, ctx: &mut CommandContext, record_history: bool) {
    // Placement and contextual connections can change neighboring named ports.
    // Preserve their exact sparse delta, including unused side assignments.
    let placement_before = (record_history
        && matches!(
            command,
            EditorCommand::PlaceTile { .. } | EditorCommand::ConnectTiles { .. }
        ))
    .then(|| ctx.project.clone());
    match execute_command(
        &mut ctx.project.document,
        &mut ctx.attention,
        &mut ctx.selection,
        &*ctx.provenance,
        command,
    ) {
        Ok(result) => {
            if result.is_accepted() {
                if result.invalidation.save {
                    ctx.project.mark_dirty();
                }
                if record_history {
                    let inverse = if let Some(before) = &placement_before {
                        super::editing::capture_change(before, &ctx.project).map(|record| {
                            super::EditorInverse::RestoreTileEdit {
                                record: Box::new(record),
                            }
                        })
                    } else {
                        result.undo.clone()
                    };
                    if let Some(inverse) = inverse {
                        let entry = HistoryEntry {
                            forward: command.clone(),
                            inverse,
                            invalidation: result.invalidation,
                        };
                        let in_gesture = matches!(command, EditorCommand::SetAtomValue { node, .. } if ctx.atom_edit.node.as_ref() == Some(node));
                        if in_gesture && ctx.atom_edit.recorded {
                            ctx.history.continue_atom_edit(entry);
                        } else {
                            ctx.record_mutation(entry);
                        }
                        if in_gesture {
                            ctx.atom_edit.recorded = true;
                        }
                    }
                }
                apply_invalidation(&mut ctx.runtime, &result);
                // SetBpm also updates the live clock (document is source of truth).
                if let EditorCommand::SetBpm { bpm } = command {
                    ctx.clock.set_bpm(*bpm);
                }
            }
            push_transaction_diagnostics(&mut ctx.diagnostics, &result);
            if result.is_accepted() {
                if let Some(change) = result.active_surface_change {
                    ctx.surface_events.write(change);
                }
                if matches!(command, EditorCommand::ConnectTiles { .. }) {
                    ctx.session.settle_after_connect();
                }
                if matches!(command, EditorCommand::PlaceTile { .. }) {
                    ctx.session.settle_after_place();
                }
            }
        }
        Err(error) => {
            push_execution_error(&mut ctx.diagnostics, error);
        }
    }
}

fn apply_flags(runtime: &mut RuntimeState, invalidation: &Invalidation) {
    let synthetic = EditorTransactionResult {
        status: crate::application::editor::transaction::TransactionStatus::Accepted,
        invalidation: *invalidation,
        active_surface_change: None,
        diagnostics: Vec::new(),
        undo: None,
    };
    apply_invalidation(runtime, &synthetic);
}

fn transport_has_audible_content(runtime: &RuntimeState) -> bool {
    runtime
        .compiled
        .as_ref()
        .is_some_and(|compiled| !compiled.scores.is_empty())
}

fn push_execution_error(diagnostics: &mut DiagnosticStore, error: crate::domain::DomainError) {
    diagnostics.push(crate::infrastructure::diagnostics::LayeredDiagnostic {
        phase: crate::infrastructure::diagnostics::DiagnosticPhase::Transaction,
        diagnostic: crate::infrastructure::diagnostics::AppDiagnostic::Transaction(
            crate::infrastructure::diagnostics::TransactionDiagnostic::Rejected {
                message: error.to_string(),
            },
        ),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::editor::transaction::Invalidation;

    #[cfg(target_os = "macos")]
    #[test]
    fn native_save_dispatch_stays_on_the_main_thread() {
        use bevy::ecs::system::System;
        let mut system = IntoSystem::into_system(dispatch_commands);
        system.initialize(&mut World::new());
        assert!(!system.is_send());
    }

    #[test]
    fn stored_invalidation_is_applied_on_undo_path() {
        // Contract: HistoryEntry.invalidation is the authority for dirty flags
        // after undo/redo, not the inverse's recomputed result.
        let stored = Invalidation::document_changed();
        let mut runtime = RuntimeState::default();
        apply_flags(&mut runtime, &stored);
        assert!(runtime.needs_compile());
        assert!(runtime.needs_scene());
        assert!(runtime.needs_projection());
    }

    #[test]
    fn editor_inverse_is_not_an_editor_command() {
        // Compile-time vocabulary split: inverses live on EditorInverse only.
        let _ = super::super::EditorInverse::DeleteNode {
            node: tessera::prelude::NodeId::new("x"),
        };
    }

    #[test]
    fn one_numeric_drag_is_one_undo_step_and_redo_restores_final_value() {
        use crate::domain::document::{AtomValue, DocumentNodeKind};
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::infrastructure::app::AppState>()
            .init_state::<crate::infrastructure::app::TransportMode>()
            .add_plugins(MinimalPlugins)
            .add_plugins((
                crate::application::editor::EditorPlugin,
                crate::application::pipeline::PlaybackPlugin,
            ));
        app.insert_state(crate::infrastructure::app::AppState::Editor);
        let project = MusaicProject::demo();
        let number = project.document.graph.nodes_on_surface(project.document.root_surface).into_iter()
            .find(|(_, node)| matches!(&node.kind, DocumentNodeKind::Atom(atom) if atom.atom == AtomValue::Number(2)))
            .unwrap().1.id.clone();
        app.world_mut().insert_resource(project);
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::BeginAtomEdit {
                node: number.clone(),
            }));
        for value in [3, 4, 5, 6] {
            app.world_mut()
                .write_message(EditorCommandBus(EditorCommand::SetAtomValue {
                    node: number.clone(),
                    value: AtomValue::Number(value),
                }));
            app.update();
        }
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::EndAtomEdit));
        app.update();
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
        let current_value = |app: &App| match &app
            .world()
            .resource::<MusaicProject>()
            .document
            .graph
            .node(&number)
            .unwrap()
            .kind
        {
            DocumentNodeKind::Atom(atom) => atom.atom.clone(),
            _ => panic!("number changed type"),
        };
        assert_eq!(current_value(&app), AtomValue::Number(6));
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::Undo));
        app.update();
        assert_eq!(current_value(&app), AtomValue::Number(2));
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::Redo));
        app.update();
        assert_eq!(current_value(&app), AtomValue::Number(6));
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
    }

    #[test]
    fn save_project_as_clears_dirty_and_sets_path() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::infrastructure::app::AppState>()
            .init_state::<crate::infrastructure::app::TransportMode>()
            .add_plugins(MinimalPlugins)
            .add_plugins((
                crate::application::editor::EditorPlugin,
                crate::application::pipeline::PlaybackPlugin,
            ));
        app.insert_state(crate::infrastructure::app::AppState::Editor);

        {
            let mut project = app.world_mut().resource_mut::<MusaicProject>();
            project.mark_dirty();
            project.metadata.display_name = "save-test".into();
        }

        let dir = std::env::temp_dir().join(format!(
            "musaic-save-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("doc.musaic.json");

        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::SaveProjectAs {
                path: path.clone(),
            }));
        app.update();

        let project = app.world().resource::<MusaicProject>();
        let session = app.world().resource::<ProjectSession>();
        assert!(!project.metadata.dirty);
        assert_eq!(
            project.metadata.file_path.as_deref(),
            Some(path.display().to_string().as_str())
        );
        assert_eq!(session.last_saved_path.as_deref(), Some(path.as_path()));
        assert!(path.is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
#[test]
fn explicit_library_visibility_is_idempotent_when_focus_opens_it_automatically() {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<crate::infrastructure::app::AppState>()
        .init_state::<crate::infrastructure::app::TransportMode>()
        .add_plugins(MinimalPlugins)
        .add_plugins((
            crate::application::editor::EditorPlugin,
            crate::application::pipeline::PlaybackPlugin,
        ));
    app.insert_state(crate::infrastructure::app::AppState::Editor);
    app.world_mut()
        .write_message(EditorCommandBus(EditorCommand::AdoptProject {
            project: MusaicProject::new_empty(),
            path: None,
        }));
    app.update();
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    app.world_mut()
        .write_message(EditorCommandBus(EditorCommand::Focus {
            target: crate::application::editor::FocusTarget::EmptySlot {
                surface: root,
                slot: crate::domain::board::BoardSlot::new(-1, 0),
            },
        }));
    app.update();
    assert!(
        app.world()
            .resource::<DrawerPanelState>()
            .effective_open(app.world().resource::<EditorAttention>())
    );
    let before = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .clone();
    for open in [true, true, false, false, true] {
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::SetDrawerOpen { open }));
        app.update();
        assert_eq!(
            app.world()
                .resource::<DrawerPanelState>()
                .effective_open(app.world().resource::<EditorAttention>()),
            open
        );
    }
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
    assert_eq!(
        app.world().resource::<MusaicProject>().document.graph,
        before
    );
}

#[cfg(test)]
#[test]
fn replacing_a_document_announces_navigation_even_when_root_ids_match() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
        .init_state::<crate::infrastructure::app::AppState>()
        .init_state::<crate::infrastructure::app::TransportMode>()
        .add_plugins((
            crate::application::editor::EditorPlugin,
            crate::application::pipeline::PlaybackPlugin,
        ))
        .insert_state(crate::infrastructure::app::AppState::Editor);
    app.update();
    app.world_mut()
        .resource_mut::<Messages<ActiveSurfaceChanged>>()
        .clear();
    let previous = app.world().resource::<EditorAttention>().active_board();
    app.world_mut()
        .write_message(EditorCommandBus(EditorCommand::NewProject));
    app.update();
    let changes: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<ActiveSurfaceChanged>>()
        .drain()
        .collect();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].previous, previous);
    assert_eq!(changes[0].current, previous);
    assert_eq!(
        changes[0].reason,
        crate::application::editor::ActiveSurfaceChangeReason::OpenDocument
    );
}
