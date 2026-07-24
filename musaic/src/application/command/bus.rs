//! Command bus — the single mutation authority for editor application state.
//!
//! Producers emit [`EditorCommandBus`]; [`dispatch_commands`] is the only writer
//! of document / attention / selection / session / view-settings /
//! project replacement / save metadata. Pipeline dirty flags on [`RuntimeState`]
//! are set here via invalidation; compiled IR is written only by pipeline stages.
//! Panel chrome width stays UI-local (declared exception).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::state::condition::in_state;

use super::{EditorCommand, execute_command, execute_inverse};
use crate::adapter::persistence::{editor_save_project, load_project};
use crate::application::board_view_settings::BoardViewSettings;
use crate::application::editor::interaction::{apply_invalidation, push_transaction_diagnostics};
use crate::application::editor::transaction::{EditorTransactionResult, Invalidation};
use crate::application::editor::{
    ActiveSurfaceChanged, DrawerPanelState, EditorAttention, EditorSession, MinimapPanelState,
    SelectionState,
};
use crate::application::history::{CommandHistory, HistoryEntry, HistoryPolicy};
use crate::application::pipeline::runtime::{RuntimeState, TimelineProvenanceStore};
use crate::application::session::{MusaicProject, ProjectSession};
use crate::infrastructure::diagnostics::DiagnosticStore;
use tessera::bevy::TesseraBoard;

#[derive(Message, Debug, Clone)]
pub struct EditorCommandBus(pub EditorCommand);

/// Mutable handles owned by the command dispatcher — one SystemParam instead of
/// a 16-argument free function.
#[derive(SystemParam)]
struct CommandContext<'w> {
    project: ResMut<'w, MusaicProject>,
    runtime: ResMut<'w, RuntimeState>,
    session_meta: ResMut<'w, ProjectSession>,
    board: ResMut<'w, TesseraBoard>,
    attention: ResMut<'w, EditorAttention>,
    selection: ResMut<'w, SelectionState>,
    provenance: Res<'w, TimelineProvenanceStore>,
    history: ResMut<'w, CommandHistory>,
    session: ResMut<'w, EditorSession>,
    drawer_panel: ResMut<'w, DrawerPanelState>,
    minimap_panel: ResMut<'w, MinimapPanelState>,
    view_settings: ResMut<'w, BoardViewSettings>,
    diagnostics: ResMut<'w, DiagnosticStore>,
    surface_events: MessageWriter<'w, ActiveSurfaceChanged>,
    transport_mode: ResMut<'w, NextState<crate::infrastructure::app::TransportMode>>,
    current_transport: Res<'w, State<crate::infrastructure::app::TransportMode>>,
    clock: ResMut<'w, crate::application::editor::transport::TransportClock>,
}

/// Registers the command bus and its dispatch stage. Called by the editor plugin.
pub fn register_command_bus(app: &mut App) {
    app.init_resource::<CommandHistory>()
        .add_message::<EditorCommandBus>()
        .add_systems(
            Update,
            dispatch_commands
                .after(crate::application::editor::MusaicEditorSet::MutateState)
                .in_set(crate::infrastructure::app::MusaicSet::Commands)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
}

fn dispatch_commands(mut bus: MessageReader<EditorCommandBus>, mut ctx: CommandContext) {
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
    match command {
        EditorCommand::ToggleDrawer => {
            ctx.drawer_panel.toggle(&ctx.attention);
            true
        }
        EditorCommand::ToggleMinimap => {
            ctx.minimap_panel.toggle();
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
                    replace_project(ctx, MusaicProject::new_empty(), None);
                }
            }
            true
        }
        EditorCommand::AdoptProject { project, path } => {
            replace_project(ctx, project.clone(), path.clone());
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
            if let Some(entry) = ctx.history.pop_undo() {
                run_history_entry(&entry, HistoryDirection::Undo, ctx);
                ctx.history.push_redo(entry);
            }
            true
        }
        EditorCommand::Redo => {
            if let Some(entry) = ctx.history.pop_redo() {
                run_history_entry(&entry, HistoryDirection::Redo, ctx);
                ctx.history.push_undo(entry);
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
        }
    }
}

fn replace_project(
    ctx: &mut CommandContext,
    project: MusaicProject,
    path: Option<std::path::PathBuf>,
) {
    *ctx.project = project;
    ctx.session_meta.last_saved_path = path;
    ctx.history.clear();
    ctx.selection.clear();
    *ctx.session = EditorSession::default();
    *ctx.attention = EditorAttention::new(ctx.project.document.root_surface);
    *ctx.runtime = RuntimeState::default();
    ctx.runtime.mark_full_rebuild();
    ctx.project.mark_dirty();
}

enum HistoryDirection {
    Undo,
    Redo,
}

fn run_history_entry(entry: &HistoryEntry, direction: HistoryDirection, ctx: &mut CommandContext) {
    let result = match direction {
        HistoryDirection::Undo => execute_inverse(
            &mut ctx.project.document,
            &mut ctx.board,
            &mut ctx.attention,
            &mut ctx.selection,
            &entry.inverse,
        ),
        HistoryDirection::Redo => execute_command(
            &mut ctx.project.document,
            &mut ctx.board,
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

fn run_command(command: &EditorCommand, ctx: &mut CommandContext, record_history: bool) {
    match execute_command(
        &mut ctx.project.document,
        &mut ctx.board,
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
                    if let Some(inverse) = result.undo.clone() {
                        ctx.history.record_mutation(HistoryEntry {
                            forward: command.clone(),
                            inverse,
                            invalidation: result.invalidation,
                        });
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
    diagnostics
        .items
        .push(crate::infrastructure::diagnostics::LayeredDiagnostic {
            phase: crate::infrastructure::diagnostics::DiagnosticPhase::Runtime,
            diagnostic: crate::infrastructure::diagnostics::AppDiagnostic::Runtime(
                crate::infrastructure::diagnostics::RuntimeDiagnostic::ProjectionFailed {
                    detail: error.to_string(),
                },
            ),
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::editor::transaction::Invalidation;

    #[test]
    fn stored_invalidation_is_applied_on_undo_path() {
        // Contract: HistoryEntry.invalidation is the authority for dirty flags
        // after undo/redo, not the inverse's recomputed result.
        let stored = Invalidation::document_changed();
        let mut runtime = RuntimeState::default();
        apply_flags(&mut runtime, &stored);
        assert!(runtime.dirty.tessera);
        assert!(runtime.dirty.lowering);
        assert!(runtime.dirty.scene);
        assert!(runtime.dirty.runtime);
    }

    #[test]
    fn editor_inverse_is_not_an_editor_command() {
        // Compile-time vocabulary split: inverses live on EditorInverse only.
        let _ = super::super::EditorInverse::DeleteNode {
            node: tessera::prelude::NodeId::new("x"),
        };
    }

    #[test]
    fn save_project_as_clears_dirty_and_sets_path() {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<crate::infrastructure::app::AppState>()
            .init_state::<crate::infrastructure::app::TransportMode>()
            .add_plugins(MinimalPlugins)
            .add_plugins((
                tessera::bevy::TesseraPlugin,
                crate::application::editor::EditorPlugin,
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
