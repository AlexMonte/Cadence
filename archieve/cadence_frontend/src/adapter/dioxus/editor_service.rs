use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
};

use dioxus::prelude::*;

use crate::adapter::backend;
use crate::adapter::{GraphOp, GridPos, InitStageOp, RuntimeCommitArgs, runtime};
use crate::application::editor::{
    ActivityTab, AutoConnectFeedback, AutoConnectOutcomeKind, BACKEND_REQUIRED_MESSAGE, DragHover,
    DragHoverReason, DragHoverStatus, DragPreviewFeedback, DragPreviewKind, DragPreviewOutcomeKind,
    DragSession, EditorCommand, EditorShellState, EditorState, InteractionState, LoadedSnapshot,
    PendingProjectAction, PointerPoint, SubsystemReadiness, WorkspaceMode, apply_graph_ops_locally,
    apply_graph_result, default_diagnostics_snapshot, default_history_status,
    default_runtime_status, empty_catalog, empty_graph, empty_graph_preview, empty_init_stage,
    empty_project_preview, empty_project_view, empty_sample_library, is_backend_unavailable,
    normalize_graph_view, normalize_selected_nodes, selected_node_view, slugify_identifier,
    tile_side_to_string,
};
use crate::domain::{GraphView, first_free_position, is_cell_occupied};

#[derive(Clone, Debug, PartialEq)]
pub enum EditorAction {
    Dispatch(EditorCommand),
    LoadSnapshot {
        reset_selection: bool,
    },
    SwitchWorkspace {
        mode: WorkspaceMode,
    },
    RefreshRuntimeTelemetry,
    PlacePiece {
        piece_id: String,
        target: Option<GridPos>,
    },
    DeleteSelectedNode,
    MoveNode {
        from: GridPos,
        to: GridPos,
    },
    ConnectNodes {
        from: GridPos,
        to_node: GridPos,
    },
    SetParamSide {
        position: GridPos,
        param_id: String,
        side: String,
    },
    SetOutputSide {
        position: GridPos,
        side: String,
    },
    DisconnectEdge {
        edge_id: String,
    },
    RenameProject {
        next_name: String,
    },
    RequestProjectAction {
        action: PendingProjectAction,
    },
    ConfirmUnsavedSave,
    ConfirmUnsavedDiscard,
    SaveProject {
        force_dialog: bool,
    },
    RestoreRecoverySnapshot,
    DiscardRecoverySnapshot,
    UndoHistory,
    RedoHistory,
    UpsertSampleLoad,
    CreateTrick,
    PlayRuntime,
    StopRuntime,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditorEvent {
    CommandReceived { operation_id: u64, label: String },
    SnapshotHydrated { backend_available: bool },
    CanonicalMutationScheduled,
    RuntimeRefreshScheduled,
    RecoveryWriteScheduled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditorEffect {
    RefreshRuntimeBridge,
    ScheduleRecoveryWrite,
    ReloadSnapshot { reset_selection: bool },
    FinishProjectSwap { reset_selection: bool },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorDispatchOutcome {
    pub saved: bool,
}

pub async fn dispatch_editor_action(
    state: Signal<EditorShellState>,
    action: EditorAction,
) -> EditorDispatchOutcome {
    let operation_id = begin_operation(state, &action);
    trace_event(
        operation_id,
        &EditorEvent::CommandReceived {
            operation_id,
            label: action_label(&action).to_string(),
        },
    );

    let outcome = match action {
        EditorAction::Dispatch(command) => dispatch_editor_command_internal(state, command).await,
        EditorAction::LoadSnapshot { reset_selection } => {
            hydrate_snapshot(state, reset_selection).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::SwitchWorkspace { mode } => {
            switch_workspace(state, mode).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::RefreshRuntimeTelemetry => {
            refresh_runtime_telemetry_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::PlacePiece { piece_id, target } => {
            place_piece_internal(state, piece_id, target).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::DeleteSelectedNode => {
            delete_selected_node_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::MoveNode { from, to } => {
            move_node_internal(state, from, to).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::ConnectNodes { from, to_node } => {
            connect_nodes_internal(state, from, to_node).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::SetParamSide {
            position,
            param_id,
            side,
        } => {
            let next_selected = state.read().selected_node;
            apply_graph_ops_internal(
                state,
                "native_param_set_side",
                vec![GraphOp::ParamSetSide {
                    position,
                    param_id,
                    side,
                }],
                next_selected,
                None,
            )
            .await;
            EditorDispatchOutcome::default()
        }
        EditorAction::SetOutputSide { position, side } => {
            let next_selected = state.read().selected_node;
            apply_graph_ops_internal(
                state,
                "native_output_set_side",
                vec![GraphOp::OutputSetSide { position, side }],
                next_selected,
                None,
            )
            .await;
            EditorDispatchOutcome::default()
        }
        EditorAction::DisconnectEdge { edge_id } => {
            let next_selected = state.read().selected_node;
            apply_graph_ops_internal(
                state,
                "native_disconnect_edge",
                vec![GraphOp::EdgeDisconnect {
                    edge_id: backend::EdgeId(edge_id),
                }],
                next_selected,
                None,
            )
            .await;
            EditorDispatchOutcome::default()
        }
        EditorAction::RenameProject { next_name } => {
            rename_project_internal(state, next_name).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::RequestProjectAction { action } => {
            request_project_action_internal(state, action).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::ConfirmUnsavedSave => {
            let saved = confirm_unsaved_save_internal(state).await;
            EditorDispatchOutcome { saved }
        }
        EditorAction::ConfirmUnsavedDiscard => {
            confirm_unsaved_discard_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::SaveProject { force_dialog } => EditorDispatchOutcome {
            saved: save_project_internal(state, force_dialog).await,
        },
        EditorAction::RestoreRecoverySnapshot => {
            restore_recovery_snapshot_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::DiscardRecoverySnapshot => {
            discard_recovery_snapshot_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::UndoHistory => {
            undo_history_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::RedoHistory => {
            redo_history_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::UpsertSampleLoad => {
            upsert_sample_load_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::CreateTrick => {
            create_trick_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::PlayRuntime => {
            play_runtime_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorAction::StopRuntime => {
            stop_runtime_internal(state).await;
            EditorDispatchOutcome::default()
        }
    };

    finish_operation(state, operation_id);
    outcome
}

pub async fn dispatch_editor_command(
    state: Signal<EditorShellState>,
    command: EditorCommand,
) -> EditorDispatchOutcome {
    dispatch_editor_action(state, EditorAction::Dispatch(command)).await
}

pub async fn load_snapshot(state: Signal<EditorShellState>, reset_selection: bool) {
    let _ = dispatch_editor_action(state, EditorAction::LoadSnapshot { reset_selection }).await;
}

pub async fn switch_workspace_mode(state: Signal<EditorShellState>, mode: WorkspaceMode) {
    let _ = dispatch_editor_action(state, EditorAction::SwitchWorkspace { mode }).await;
}

pub async fn refresh_runtime_telemetry(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::RefreshRuntimeTelemetry).await;
}

pub async fn place_piece(
    state: Signal<EditorShellState>,
    piece_id: String,
    target: Option<GridPos>,
) {
    let _ = dispatch_editor_action(state, EditorAction::PlacePiece { piece_id, target }).await;
}

pub async fn delete_selected_node(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::DeleteSelectedNode).await;
}

pub async fn apply_graph_ops(
    state: Signal<EditorShellState>,
    request_id: &str,
    ops: Vec<GraphOp>,
    next_selected: Option<GridPos>,
) {
    apply_graph_ops_internal(state, request_id, ops, next_selected, None).await;
}

pub async fn move_node(state: Signal<EditorShellState>, from: GridPos, to: GridPos) {
    let _ = dispatch_editor_action(state, EditorAction::MoveNode { from, to }).await;
}

pub async fn set_param_side(
    state: Signal<EditorShellState>,
    position: GridPos,
    param_id: String,
    side: String,
) {
    let _ = dispatch_editor_action(
        state,
        EditorAction::SetParamSide {
            position,
            param_id,
            side,
        },
    )
    .await;
}

pub async fn set_output_side(state: Signal<EditorShellState>, position: GridPos, side: String) {
    let _ = dispatch_editor_action(state, EditorAction::SetOutputSide { position, side }).await;
}

pub async fn disconnect_edge(state: Signal<EditorShellState>, edge_id: String) {
    let _ = dispatch_editor_action(state, EditorAction::DisconnectEdge { edge_id }).await;
}

pub async fn rename_project(state: Signal<EditorShellState>, next_name: String) {
    let _ = dispatch_editor_action(state, EditorAction::RenameProject { next_name }).await;
}

pub async fn request_project_action(state: Signal<EditorShellState>, action: PendingProjectAction) {
    let _ = dispatch_editor_action(state, EditorAction::RequestProjectAction { action }).await;
}

pub async fn confirm_unsaved_save(state: Signal<EditorShellState>) -> bool {
    dispatch_editor_action(state, EditorAction::ConfirmUnsavedSave)
        .await
        .saved
}

pub async fn confirm_unsaved_discard(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::ConfirmUnsavedDiscard).await;
}

pub async fn save_project(state: Signal<EditorShellState>, force_dialog: bool) -> bool {
    dispatch_editor_action(state, EditorAction::SaveProject { force_dialog })
        .await
        .saved
}

pub async fn restore_recovery_snapshot(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::RestoreRecoverySnapshot).await;
}

pub async fn discard_recovery_snapshot(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::DiscardRecoverySnapshot).await;
}

pub async fn undo_history(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::UndoHistory).await;
}

pub async fn redo_history(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::RedoHistory).await;
}

pub async fn upsert_sample_load(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::UpsertSampleLoad).await;
}

pub async fn apply_init_ops(state: Signal<EditorShellState>, ops: Vec<InitStageOp>) {
    apply_init_ops_internal(state, ops).await;
}

pub async fn create_trick(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::CreateTrick).await;
}

pub async fn play_runtime(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::PlayRuntime).await;
}

pub async fn stop_runtime(state: Signal<EditorShellState>) {
    let _ = dispatch_editor_action(state, EditorAction::StopRuntime).await;
}

pub fn refresh_runtime_bridge_state(mut state: Signal<EditorShellState>) {
    refresh_runtime_bridge_state_internal(&mut state.write());
}

async fn dispatch_editor_command_internal(
    mut state: Signal<EditorShellState>,
    command: EditorCommand,
) -> EditorDispatchOutcome {
    match command {
        EditorCommand::OpenPicker => {
            open_picker_internal(&mut state.write(), None, false);
            EditorDispatchOutcome::default()
        }
        EditorCommand::PlacePiece { piece_id } => {
            place_piece_internal(state, piece_id, None).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::SwitchWorkspace { mode } => {
            switch_workspace(state, mode).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::OpenCompile => {
            let mut current = state.write();
            current.compile_open = true;
            current.activity_tab = ActivityTab::Preview;
            EditorDispatchOutcome::default()
        }
        EditorCommand::OpenInspector => {
            if state.read().selected_node.is_some() {
                state.write().inspector.open = true;
            } else {
                state.write().status_message = Some("Select a tile to edit.".to_string());
            }
            EditorDispatchOutcome::default()
        }
        EditorCommand::ProjectAction { action } => {
            request_project_action_internal(state, action).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::SaveProject { force_dialog } => EditorDispatchOutcome {
            saved: save_project_internal(state, force_dialog).await,
        },
        EditorCommand::ExportSong => {
            export_song_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::PlayRuntime => {
            play_runtime_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::StopRuntime => {
            stop_runtime_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::Undo => {
            undo_history_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::Redo => {
            redo_history_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::ToggleConsole => {
            {
                let mut current = state.write();
                current.compile_open = true;
                current.activity_tab = ActivityTab::Diagnostics;
            }
            set_mini_console_visible_internal(state, true).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::ToggleDevInspector => {
            toggle_dev_inspector_internal(state).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::ClearTransientPanels => {
            let mut current = state.write();
            current.compile_open = false;
            current.inspector.open = false;
            close_picker_internal(&mut current);
            clear_drag_state_internal(&mut current);
            EditorDispatchOutcome::default()
        }
        EditorCommand::SelectTile { position, additive } => {
            select_tile_internal(&mut state.write(), position, additive);
            EditorDispatchOutcome::default()
        }
        EditorCommand::SelectCell { position, additive } => {
            select_cell_internal(&mut state.write(), position, additive);
            EditorDispatchOutcome::default()
        }
        EditorCommand::BeginTileDrag { position } => {
            begin_tile_drag_internal(&mut state.write(), position);
            EditorDispatchOutcome::default()
        }
        EditorCommand::PreviewTileDrag { position } => {
            preview_tile_drag_internal(&mut state.write(), position);
            EditorDispatchOutcome::default()
        }
        EditorCommand::CommitTileDrag { position } => {
            commit_tile_drag_internal(state, position).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::BeginConnection { from } => {
            begin_connection_internal(&mut state.write(), from);
            EditorDispatchOutcome::default()
        }
        EditorCommand::PreviewConnection { position } => {
            preview_connection_internal(&mut state.write(), position);
            EditorDispatchOutcome::default()
        }
        EditorCommand::CommitConnection { position } => {
            commit_connection_internal(state, position).await;
            EditorDispatchOutcome::default()
        }
        EditorCommand::BeginMarquee { origin, additive } => {
            begin_marquee_internal(&mut state.write(), origin, additive);
            EditorDispatchOutcome::default()
        }
        EditorCommand::PreviewMarquee { current } => {
            preview_marquee_internal(&mut state.write(), current);
            EditorDispatchOutcome::default()
        }
        EditorCommand::CommitMarquee => {
            commit_marquee_internal(&mut state.write());
            EditorDispatchOutcome::default()
        }
        EditorCommand::ClearBoardInteraction => {
            clear_drag_state_internal(&mut state.write());
            EditorDispatchOutcome::default()
        }
    }
}

async fn hydrate_snapshot(mut state: Signal<EditorShellState>, reset_selection: bool) {
    {
        let mut current = state.write();
        current.loading = true;
        current.lifecycle.hydration = SubsystemReadiness::loading("Refreshing editor snapshot…");
    }

    let mode = state.read().workspace_mode.clone();
    match fetch_snapshot(mode.clone()).await {
        Ok(loaded) => {
            if state.read().workspace_mode != mode {
                return;
            }
            let next = merge_loaded_snapshot(&state.read().clone(), loaded, reset_selection);
            trace_event(
                state
                    .read()
                    .command_trace
                    .active_operation_id
                    .unwrap_or_default(),
                &EditorEvent::SnapshotHydrated {
                    backend_available: next.backend_available,
                },
            );
            state.set(next);
            refresh_runtime_bridge_state(state);
        }
        Err(error) => {
            if state.read().workspace_mode != mode {
                return;
            }
            let mut current = state.write();
            current.loading = false;
            current.status_message = Some(error.clone());
            current.lifecycle.hydration = SubsystemReadiness::failed(error);
        }
    }
}

pub async fn fetch_snapshot(mode: WorkspaceMode) -> Result<LoadedSnapshot, String> {
    match fetch_backend_snapshot(mode.clone()).await {
        Ok(snapshot) => Ok(snapshot),
        Err(error) if is_backend_unavailable(&error) => Ok(LoadedSnapshot {
            backend_available: false,
            project: empty_project_view(),
            graph: empty_graph(),
            catalog: empty_catalog(),
            init_stage: empty_init_stage(),
            sample_library: Some(empty_sample_library()),
            project_preview: empty_project_preview(),
            graph_preview: empty_graph_preview(),
            history_status: default_history_status(),
            runtime_status: default_runtime_status(),
            diagnostics: default_diagnostics_snapshot(),
            recovery_path: None,
        }),
        Err(error) => Err(error),
    }
}

pub async fn fetch_backend_snapshot(mode: WorkspaceMode) -> Result<LoadedSnapshot, String> {
    let include_sample_library = matches!(mode, WorkspaceMode::Init);
    let snapshot = backend::editor_sync(mode.graph_target(), include_sample_library).await?;

    Ok(LoadedSnapshot {
        backend_available: true,
        project: snapshot.project,
        graph: normalize_graph_view(&snapshot.graph),
        catalog: snapshot.catalog,
        init_stage: snapshot.init_stage,
        sample_library: snapshot.sample_library,
        project_preview: snapshot.project_preview,
        graph_preview: snapshot.graph_preview,
        history_status: snapshot.history_status,
        runtime_status: snapshot.runtime_status,
        diagnostics: snapshot.diagnostics,
        recovery_path: snapshot.recovery_path,
    })
}

pub fn merge_loaded_snapshot(
    current: &EditorShellState,
    loaded: LoadedSnapshot,
    reset_selection: bool,
) -> EditorShellState {
    let previous_selected = current.selected_node;
    let previous_selected_nodes = current.selected_nodes.clone();
    let previous_cell = current.selected_cell;
    let mut next = current.clone();
    next.loading = false;
    next.backend_available = loaded.backend_available;
    next.project = loaded.project.clone();
    next.project_name_input = loaded.project.name.clone();
    next.graph = loaded.graph;
    next.catalog = loaded.catalog;
    next.init_stage = loaded.init_stage;
    if let Some(sample_library) = loaded.sample_library {
        next.sample_library = sample_library;
    }
    next.init_cps_input = next.init_stage.cps_expr.clone().unwrap_or_default();
    next.project_preview = loaded.project_preview;
    next.graph_preview = loaded.graph_preview;
    next.history_status = loaded.history_status;
    next.runtime_status = loaded.runtime_status;
    next.diagnostics = loaded.diagnostics;
    next.recovery_path = loaded.recovery_path;
    if !next.recovery_checked {
        next.recovery_open = next.recovery_path.is_some();
        next.recovery_checked = true;
    }
    close_command_palette_internal(&mut next);
    next.interaction_state = None;
    next.drag_hover = None;
    next.drag_preview = None;
    next.drag_preview_nonce = next.drag_preview_nonce.wrapping_add(1);
    next.auto_connect_feedback = None;
    next.inspector.pending_probe = None;
    if loaded.backend_available != current.backend_available {
        let transition_msg = if loaded.backend_available {
            "Backend connected. Switched to live mode."
        } else {
            "Backend disconnected. Editor is waiting for the live backend."
        };
        next.diagnostics.entries.push(backend::DiagnosticEntryDto {
            at_ms: 0,
            kind: "mode_transition".to_string(),
            message: transition_msg.to_string(),
        });
    }
    next.status_message = if loaded.backend_available {
        None
    } else {
        Some(BACKEND_REQUIRED_MESSAGE.to_string())
    };
    if reset_selection {
        next.selected_node = next.graph.nodes.first().map(|node| node.position);
        next.selected_nodes = next.selected_node.into_iter().collect();
        next.selected_cell = next
            .selected_node
            .or_else(|| first_free_position(&next.graph));
    } else {
        next.selected_nodes = previous_selected_nodes
            .into_iter()
            .filter(|pos| next.graph.nodes.iter().any(|node| node.position == *pos))
            .collect();
        next.selected_node = previous_selected
            .filter(|pos| next.graph.nodes.iter().any(|node| &node.position == pos))
            .or_else(|| next.selected_nodes.first().copied())
            .or_else(|| next.graph.nodes.first().map(|node| node.position));
        next.selected_cell = previous_cell.or(next.selected_node);
    }
    normalize_selected_nodes(&mut next);
    next.picker_target = if next.picker_open {
        preferred_picker_target(&next)
    } else {
        None
    };
    sync_editor_inputs_internal(&mut next);
    next.lifecycle.backend = if loaded.backend_available {
        SubsystemReadiness::ready("Live backend connected.")
    } else {
        SubsystemReadiness::unavailable(BACKEND_REQUIRED_MESSAGE)
    };
    next.lifecycle.hydration = SubsystemReadiness::ready("Editor snapshot is in sync.");
    next.lifecycle.recovery = if next.recovery_path.is_some() {
        SubsystemReadiness::recovering("Recovery snapshot available.")
    } else {
        SubsystemReadiness::ready("Recovery idle.")
    };
    next
}

async fn switch_workspace(mut state: Signal<EditorShellState>, mode: WorkspaceMode) {
    {
        let mut current = state.write();
        current.workspace_mode = mode;
        current.lifecycle.hydration = SubsystemReadiness::loading("Switching workspace…");
        close_picker_internal(&mut current);
        close_command_palette_internal(&mut current);
        clear_drag_state_internal(&mut current);
    }
    hydrate_snapshot(state, true).await;
}

async fn refresh_runtime_telemetry_internal(mut state: Signal<EditorShellState>) {
    refresh_runtime_bridge_state(state);
    if !state.read().backend_available {
        return;
    }
    if let Ok(runtime_status) = backend::runtime_status().await {
        state.write().runtime_status = runtime_status;
    }
    if let Ok(history_status) = backend::history_status().await {
        state.write().history_status = history_status;
    }
    if let Ok(diagnostics) = backend::diagnostics_snapshot().await {
        state.write().diagnostics = diagnostics;
    }
}

fn refresh_runtime_bridge_state_internal(current: &mut EditorShellState) {
    if let Ok(status) = runtime::runtime_boot_status() {
        current.runtime_boot = status;
    }
    if let Ok(status) = runtime::runtime_sample_readiness() {
        current.sample_readiness = status;
    }
    if let Ok(status) = runtime::runtime_sample_cache_status() {
        current.sample_cache = status;
    }
    if let Ok(status) = runtime::runtime_init_sample_status() {
        current.init_sample_status = status;
    }

    current.lifecycle.runtime = if current.backend_available {
        SubsystemReadiness::ready("Runtime bridge refreshed.")
    } else {
        SubsystemReadiness::unavailable("Runtime bridge unavailable without the live backend.")
    };
}

async fn place_piece_internal(
    mut state: Signal<EditorShellState>,
    piece_id: String,
    target: Option<GridPos>,
) {
    if !state.read().workspace_mode.picker_allowed() {
        let mut current = state.write();
        close_picker_internal(&mut current);
        current.status_message =
            Some("Tile placement is only available in Runtime or Trick workspaces.".to_string());
        return;
    }

    let graph = state.read().graph.clone();
    let preferred_position = target.or_else(|| state.read().picker_target);
    let position = preferred_position
        .filter(|pos| !is_cell_occupied(&graph, pos))
        .or_else(|| {
            target
                .is_none()
                .then(|| first_free_position(&graph))
                .flatten()
        });

    let Some(position) = position else {
        let message = if target.is_some() {
            "Selected cell is already occupied."
        } else {
            "No free grid cell is available."
        };
        state.write().status_message = Some(message.to_string());
        return;
    };

    if piece_id.starts_with("cadence.atom.") {
        state.write().status_message = Some(
            "Atoms are container-local insertables, not graph tiles. Place a container first, then insert atoms through the atom drawer.".to_string(),
        );
        return;
    }

    let pattern_source = seeded_pattern_surface_for_piece(piece_id.as_str());

    apply_graph_ops_internal(
        state,
        "native_place_piece",
        vec![GraphOp::NodePlace {
            position,
            piece_id,
            inline_params: BTreeMap::new(),
            pattern_source,
        }],
        Some(position),
        None,
    )
    .await;
    auto_connect_positions(state, vec![position]).await;
}

async fn delete_selected_node_internal(mut state: Signal<EditorShellState>) {
    let selected_positions = {
        let shell = state.read();
        if shell.selected_nodes.is_empty() {
            shell.selected_node.into_iter().collect::<Vec<_>>()
        } else {
            shell.selected_nodes.clone()
        }
    };
    if selected_positions.is_empty() {
        return;
    }

    state.write().inspector.open = false;
    apply_graph_ops_internal(
        state,
        "native_delete_node",
        selected_positions
            .into_iter()
            .map(|position| GraphOp::NodeRemove { position })
            .collect(),
        None,
        None,
    )
    .await;
}

async fn move_node_internal(mut state: Signal<EditorShellState>, from: GridPos, to: GridPos) {
    if from == to {
        return;
    }

    let shell = state.read().clone();
    let selection = if shell.selected_nodes.is_empty() {
        shell.selected_node.into_iter().collect::<Vec<_>>()
    } else {
        shell.selected_nodes.clone()
    };

    if selection.len() > 1 && selection.contains(&from) {
        match build_group_move_ops(&shell.graph, &selection, from, to) {
            Ok((ops, next_selected, next_selected_nodes)) => {
                let changed_positions = next_selected_nodes.clone();
                apply_graph_ops_internal(
                    state,
                    "native_move_node_group",
                    ops,
                    Some(next_selected),
                    Some(next_selected_nodes),
                )
                .await;
                auto_connect_positions(state, changed_positions).await;
            }
            Err(error) => {
                state.write().status_message = Some(error);
            }
        }
        return;
    }

    let occupied = is_cell_occupied(&shell.graph, &to);
    let ops = if occupied {
        vec![GraphOp::NodeSwap { a: from, b: to }]
    } else {
        vec![GraphOp::NodeMove { from, to }]
    };

    apply_graph_ops_internal(
        state,
        if occupied {
            "native_swap_node"
        } else {
            "native_move_node"
        },
        ops,
        Some(to),
        None,
    )
    .await;
    let mut changed_positions = vec![to];
    if occupied {
        changed_positions.push(from);
    }
    auto_connect_positions(state, changed_positions).await;
}

async fn connect_nodes_internal(
    mut state: Signal<EditorShellState>,
    from: GridPos,
    to_node: GridPos,
) {
    if from == to_node {
        return;
    }

    if !state.read().backend_available {
        let mut current = state.write();
        current.inspector.pending_probe = None;
        current.status_message = Some(BACKEND_REQUIRED_MESSAGE.to_string());
        return;
    }

    let graph_target = state.read().workspace_mode.graph_target();
    match backend::graph_pick_target_param(from, to_node, graph_target, None).await {
        Ok(probe) => {
            let Some(to_param) = probe.to_param else {
                let mut current = state.write();
                current.inspector.pending_probe = Some(probe.clone());
                current.status_message = Some(
                    probe
                        .detail
                        .clone()
                        .or_else(|| probe.reason.as_ref().map(|reason| format!("{reason:?}")))
                        .unwrap_or_else(|| "Unable to connect the selected tiles.".to_string()),
                );
                return;
            };
            state.write().inspector.pending_probe = None;
            apply_graph_ops_internal(
                state,
                "native_connect_edge",
                vec![GraphOp::EdgeConnect {
                    edge_id: None,
                    from,
                    to_node,
                    to_param,
                }],
                Some(to_node),
                None,
            )
            .await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

fn build_group_move_ops(
    graph: &GraphView,
    selected: &[GridPos],
    from: GridPos,
    to: GridPos,
) -> Result<(Vec<GraphOp>, GridPos, Vec<GridPos>), String> {
    if selected.is_empty() {
        return Err("Select tiles before moving them together.".to_string());
    }
    if !selected.contains(&from) {
        return Err("The dragged tile is no longer part of the current selection.".to_string());
    }

    let delta_col = to.col - from.col;
    let delta_row = to.row - from.row;
    let mut translation = BTreeMap::new();
    for position in selected {
        let translated = GridPos {
            col: position.col + delta_col,
            row: position.row + delta_row,
        };
        if translated.col < 0
            || translated.row < 0
            || translated.col >= graph.cols as i32
            || translated.row >= graph.rows as i32
        {
            return Err("That move would push part of the selection outside the grid.".to_string());
        }
        if translation.values().any(|existing| *existing == translated) {
            return Err(
                "That move would collapse multiple selected tiles onto one cell.".to_string(),
            );
        }
        translation.insert(*position, translated);
    }

    for translated in translation.values() {
        if let Some(node) = graph.nodes.iter().find(|node| node.position == *translated) {
            if !translation.contains_key(&node.position) {
                return Err("That move is blocked by another tile.".to_string());
            }
        }
    }

    let moved_nodes = selected
        .iter()
        .map(|position| {
            graph
                .nodes
                .iter()
                .find(|node| node.position == *position)
                .cloned()
                .ok_or_else(|| "One of the selected tiles no longer exists.".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut ops = Vec::new();
    for position in selected {
        ops.push(GraphOp::NodeRemove {
            position: *position,
        });
    }

    for node in &moved_nodes {
        let next_position = translation[&node.position];
        ops.push(GraphOp::NodePlace {
            position: next_position,
            piece_id: node.piece_id.clone(),
            inline_params: node.inline_params.clone(),
            pattern_source: node.pattern_source.clone(),
        });
        for (param_id, side) in &node.input_sides {
            ops.push(GraphOp::ParamSetSide {
                position: next_position,
                param_id: param_id.clone(),
                side: side.clone(),
            });
        }
        if let Some(side) = &node.output_side {
            ops.push(GraphOp::OutputSetSide {
                position: next_position,
                side: side.clone(),
            });
        }
        if node.label.is_some() {
            ops.push(GraphOp::NodeSetLabel {
                position: next_position,
                label: node.label.clone(),
            });
        }
        if node.node_state.is_some() {
            ops.push(GraphOp::NodeSetState {
                position: next_position,
                state: node.node_state.clone(),
            });
        }
    }

    for edge in graph.edges.iter().filter(|edge| {
        translation.contains_key(&edge.from) || translation.contains_key(&edge.to_node)
    }) {
        ops.push(GraphOp::EdgeConnect {
            edge_id: None,
            from: translation.get(&edge.from).copied().unwrap_or(edge.from),
            to_node: translation
                .get(&edge.to_node)
                .copied()
                .unwrap_or(edge.to_node),
            to_param: edge.to_param.clone(),
        });
    }

    let next_selected_nodes = selected
        .iter()
        .filter_map(|position| translation.get(position).copied())
        .collect::<Vec<_>>();
    let Some(next_selected) = translation.get(&from).copied() else {
        return Err("The dragged tile could not be translated.".to_string());
    };

    Ok((ops, next_selected, next_selected_nodes))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum AutoConnectCandidateClass {
    Direct,
    Repairable,
    Rejected,
}

#[derive(Clone, Debug)]
struct AutoConnectCandidate {
    position: GridPos,
    class: AutoConnectCandidateClass,
    probe: backend::GraphPickTargetParamDto,
}

fn selection_snapshot(state: &EditorShellState) -> (Option<GridPos>, Option<Vec<GridPos>>) {
    let selected_nodes = if state.selected_nodes.is_empty() {
        None
    } else {
        Some(state.selected_nodes.clone())
    };
    (state.selected_node, selected_nodes)
}

fn local_auto_connect_targets(graph: &GraphView, source: GridPos) -> Vec<GridPos> {
    let occupied = graph
        .nodes
        .iter()
        .map(|node| node.position)
        .collect::<BTreeSet<_>>();
    let mut candidates = [
        GridPos {
            col: source.col,
            row: source.row - 1,
        },
        GridPos {
            col: source.col - 1,
            row: source.row,
        },
        GridPos {
            col: source.col + 1,
            row: source.row,
        },
        GridPos {
            col: source.col,
            row: source.row + 1,
        },
    ]
    .into_iter()
    .filter(|position| occupied.contains(position))
    .collect::<Vec<_>>();
    candidates.sort_by_key(|position| {
        (
            (position.col - source.col).abs() + (position.row - source.row).abs(),
            position.row,
            position.col,
        )
    });
    candidates
}

fn auto_connect_candidate_class(
    probe: &backend::GraphPickTargetParamDto,
) -> AutoConnectCandidateClass {
    if probe.to_param.is_some() {
        AutoConnectCandidateClass::Direct
    } else if probe
        .suggestions
        .iter()
        .all(|suggestion| !matches!(suggestion, backend::RepairSuggestion::MoveNode { .. }))
        && probe.suggestions.iter().any(|suggestion| {
            matches!(
                suggestion,
                backend::RepairSuggestion::SetOutputSide { .. }
                    | backend::RepairSuggestion::SetParamSide { .. }
                    | backend::RepairSuggestion::DisconnectEdge { .. }
            )
        })
    {
        AutoConnectCandidateClass::Repairable
    } else {
        AutoConnectCandidateClass::Rejected
    }
}

fn should_replace_auto_connect_candidate(
    best: Option<&AutoConnectCandidate>,
    candidate: &AutoConnectCandidate,
) -> bool {
    match best {
        None => true,
        Some(current) => {
            (
                candidate.class,
                candidate.position.row,
                candidate.position.col,
            ) < (current.class, current.position.row, current.position.col)
        }
    }
}

fn auto_connect_feedback_from_probe(
    source: GridPos,
    target: GridPos,
    probe: &backend::GraphPickTargetParamDto,
) -> AutoConnectFeedback {
    let reason = probe
        .detail
        .clone()
        .or_else(|| probe.reason.as_ref().map(|reason| format!("{reason:?}")))
        .unwrap_or_else(|| "No nearby compatible target.".to_string());
    AutoConnectFeedback {
        position: source,
        kind: AutoConnectOutcomeKind::Blocked,
        headline: format!(
            "Tile [{}, {}] stays unconnected near [{}, {}]",
            source.col, source.row, target.col, target.row
        ),
        detail: reason,
    }
}

fn auto_connect_success_feedback(
    source: GridPos,
    target: GridPos,
    param_id: &str,
    repaired: bool,
) -> AutoConnectFeedback {
    AutoConnectFeedback {
        position: source,
        kind: AutoConnectOutcomeKind::Connected,
        headline: format!(
            "Tile [{}, {}] connected to [{}, {}]",
            source.col, source.row, target.col, target.row
        ),
        detail: if repaired {
            format!("Adjusted local wiring and connected through `{param_id}`.")
        } else {
            format!("Connected through `{param_id}`.")
        },
    }
}

fn auto_connect_idle_feedback(source: GridPos) -> AutoConnectFeedback {
    AutoConnectFeedback {
        position: source,
        kind: AutoConnectOutcomeKind::Unchanged,
        headline: format!(
            "Tile [{}, {}] has no nearby connection",
            source.col, source.row
        ),
        detail: "No adjacent compatible target was found.".to_string(),
    }
}

fn repair_ops_for_probe(probe: &backend::GraphPickTargetParamDto) -> Vec<GraphOp> {
    probe
        .suggestions
        .iter()
        .filter_map(|suggestion| match suggestion {
            backend::RepairSuggestion::SetOutputSide { position, side } => {
                Some(GraphOp::OutputSetSide {
                    position: *position,
                    side: tile_side_to_string(*side).to_string(),
                })
            }
            backend::RepairSuggestion::SetParamSide {
                position,
                param_id,
                side,
            } => Some(GraphOp::ParamSetSide {
                position: *position,
                param_id: param_id.clone(),
                side: tile_side_to_string(*side).to_string(),
            }),
            backend::RepairSuggestion::DisconnectEdge { edge_id } => {
                Some(GraphOp::EdgeDisconnect {
                    edge_id: edge_id.clone(),
                })
            }
            backend::RepairSuggestion::MoveNode { .. } => None,
        })
        .collect()
}

async fn choose_auto_connect_candidate(
    state: &EditorShellState,
    source: GridPos,
) -> Result<Option<AutoConnectCandidate>, String> {
    let graph_target = state.workspace_mode.graph_target();
    choose_auto_connect_candidate_for_graph(&state.graph, source, |target| {
        let graph_target = graph_target.clone();
        async move { backend::graph_pick_target_param(source, target, graph_target, None).await }
    })
    .await
}

async fn choose_auto_connect_candidate_for_graph<F, Fut>(
    graph: &GraphView,
    source: GridPos,
    mut probe_target: F,
) -> Result<Option<AutoConnectCandidate>, String>
where
    F: FnMut(GridPos) -> Fut,
    Fut: Future<Output = Result<backend::GraphPickTargetParamDto, String>>,
{
    let candidates = local_auto_connect_targets(graph, source);
    let mut best = None::<AutoConnectCandidate>;
    for target in candidates {
        let probe = probe_target(target).await?;
        let candidate = AutoConnectCandidate {
            position: target,
            class: auto_connect_candidate_class(&probe),
            probe,
        };
        if should_replace_auto_connect_candidate(best.as_ref(), &candidate) {
            best = Some(candidate);
        }
    }
    Ok(best)
}

fn seeded_pattern_surface_for_piece(piece_id: &str) -> Option<backend::PatternSurface> {
    let root = match piece_id {
        "cadence.container.basic" => backend::PatternRoot {
            position: backend::PatternPos { col: 0, row: 0 },
            container: backend::PatternContainer {
                kind: backend::PatternContainerKind::Basic,
                items: Vec::new(),
            },
        },
        "cadence.container.subdivide" => backend::PatternRoot {
            position: backend::PatternPos { col: 0, row: 0 },
            container: backend::PatternContainer {
                kind: backend::PatternContainerKind::Subdivide,
                items: Vec::new(),
            },
        },
        "cadence.container.alternate" => backend::PatternRoot {
            position: backend::PatternPos { col: 0, row: 0 },
            container: backend::PatternContainer {
                kind: backend::PatternContainerKind::Alternate,
                items: Vec::new(),
            },
        },
        "cadence.container.parallel" => backend::PatternRoot {
            position: backend::PatternPos { col: 0, row: 0 },
            container: backend::PatternContainer {
                kind: backend::PatternContainerKind::Parallel,
                items: Vec::new(),
            },
        },
        _ => return None,
    };

    Some(backend::PatternSurface { roots: vec![root] })
}

pub fn invalidate_drag_preview(state: &mut EditorShellState) {
    state.drag_preview = None;
    state.drag_preview_nonce = state.drag_preview_nonce.wrapping_add(1);
}

async fn auto_connect_position(
    state: Signal<EditorShellState>,
    source: GridPos,
) -> Result<AutoConnectFeedback, String> {
    let mut shell = state.read().clone();
    let Some(_node) = shell
        .graph
        .nodes
        .iter()
        .find(|node| node.position == source)
    else {
        return Ok(auto_connect_idle_feedback(source));
    };

    let mut stale_edge_ids = Vec::new();
    for edge in shell.graph.edges.iter().filter(|edge| edge.from == source) {
        let existing_probe = backend::graph_pick_target_param(
            source,
            edge.to_node,
            shell.workspace_mode.graph_target(),
            Some(edge.to_param.clone()),
        )
        .await?;
        if existing_probe.to_param.as_deref() == Some(edge.to_param.as_str()) {
            return Ok(auto_connect_success_feedback(
                source,
                edge.to_node,
                edge.to_param.as_str(),
                false,
            ));
        }
        stale_edge_ids.push(edge.id.clone());
    }

    if !stale_edge_ids.is_empty() {
        let (next_selected, next_selected_nodes) = selection_snapshot(&shell);
        apply_graph_ops_internal(
            state,
            "native_auto_connect_disconnect_stale",
            stale_edge_ids
                .into_iter()
                .map(|edge_id| GraphOp::EdgeDisconnect {
                    edge_id: backend::EdgeId(edge_id),
                })
                .collect(),
            next_selected,
            next_selected_nodes,
        )
        .await;
        shell = state.read().clone();
    }

    let Some(candidate) = choose_auto_connect_candidate(&shell, source).await? else {
        return Ok(auto_connect_idle_feedback(source));
    };

    match candidate.class {
        AutoConnectCandidateClass::Direct => {
            let (next_selected, next_selected_nodes) = selection_snapshot(&shell);
            apply_graph_ops_internal(
                state,
                "native_auto_connect_local",
                vec![GraphOp::EdgeConnect {
                    edge_id: None,
                    from: source,
                    to_node: candidate.position,
                    to_param: candidate
                        .probe
                        .to_param
                        .clone()
                        .expect("direct candidate should include a param"),
                }],
                next_selected,
                next_selected_nodes,
            )
            .await;
            Ok(auto_connect_success_feedback(
                source,
                candidate.position,
                candidate.probe.to_param.as_deref().unwrap_or("target"),
                false,
            ))
        }
        AutoConnectCandidateClass::Repairable => {
            let repair_ops = repair_ops_for_probe(&candidate.probe);
            if repair_ops.is_empty() {
                return Ok(auto_connect_feedback_from_probe(
                    source,
                    candidate.position,
                    &candidate.probe,
                ));
            }
            let (next_selected, next_selected_nodes) = selection_snapshot(&shell);
            apply_graph_ops_internal(
                state,
                "native_auto_connect_repair",
                repair_ops,
                next_selected,
                next_selected_nodes,
            )
            .await;

            let graph_target = state.read().workspace_mode.graph_target();
            let reprobe =
                backend::graph_pick_target_param(source, candidate.position, graph_target, None)
                    .await?;
            let Some(to_param) = reprobe.to_param.clone() else {
                return Ok(auto_connect_feedback_from_probe(
                    source,
                    candidate.position,
                    &reprobe,
                ));
            };
            let (next_selected, next_selected_nodes) = selection_snapshot(&state.read().clone());
            apply_graph_ops_internal(
                state,
                "native_auto_connect_local",
                vec![GraphOp::EdgeConnect {
                    edge_id: None,
                    from: source,
                    to_node: candidate.position,
                    to_param: to_param.clone(),
                }],
                next_selected,
                next_selected_nodes,
            )
            .await;
            Ok(auto_connect_success_feedback(
                source,
                candidate.position,
                to_param.as_str(),
                true,
            ))
        }
        AutoConnectCandidateClass::Rejected => Ok(auto_connect_feedback_from_probe(
            source,
            candidate.position,
            &candidate.probe,
        )),
    }
}

async fn auto_connect_positions(mut state: Signal<EditorShellState>, positions: Vec<GridPos>) {
    let ordered = positions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if ordered.is_empty() || !state.read().backend_available {
        return;
    }

    let mut last_feedback = None;
    for source in ordered {
        match auto_connect_position(state, source).await {
            Ok(feedback) => {
                last_feedback = Some(feedback);
            }
            Err(error) => {
                state.write().status_message = Some(error);
                return;
            }
        }
    }
    state.write().auto_connect_feedback = last_feedback;
}

async fn apply_graph_ops_internal(
    mut state: Signal<EditorShellState>,
    request_id: &str,
    ops: Vec<GraphOp>,
    next_selected: Option<GridPos>,
    next_selected_nodes: Option<Vec<GridPos>>,
) {
    if ops.is_empty() {
        return;
    }

    if !state.read().backend_available {
        let mut current = state.write();
        current.inspector.pending_probe = None;
        current.status_message = Some(BACKEND_REQUIRED_MESSAGE.to_string());
        return;
    }

    let graph_target = state.read().workspace_mode.graph_target();
    let previous = state.read().clone();
    {
        let mut current = state.write();
        if let Err(error) = apply_graph_ops_locally(
            &mut current,
            &ops,
            next_selected,
            next_selected_nodes.clone(),
        ) {
            current.status_message = Some(error);
            return;
        }
        current.inspector.pending_probe = None;
        current.lifecycle.hydration =
            SubsystemReadiness::loading("Applying graph mutation through the live backend…");
    }
    trace_event(
        state
            .read()
            .command_trace
            .active_operation_id
            .unwrap_or_default(),
        &EditorEvent::CanonicalMutationScheduled,
    );
    match backend::graph_apply_ops(ops, Some(request_id.to_string()), graph_target).await {
        Ok(result) => {
            let mut current = state.write();
            apply_graph_result(
                &mut current,
                result,
                next_selected,
                next_selected_nodes.clone(),
            );
            drop(current);
            let operation_id = state
                .read()
                .command_trace
                .active_operation_id
                .unwrap_or_default();
            run_effects(state, post_mutation_effects(false), operation_id).await;
        }
        Err(error) => {
            let mut rollback = previous;
            close_picker_internal(&mut rollback);
            clear_drag_state_internal(&mut rollback);
            rollback.status_message = Some(error);
            rollback.lifecycle.hydration =
                SubsystemReadiness::failed("Graph mutation failed; state rolled back.");
            state.set(rollback);
        }
    }
}

async fn rename_project_internal(mut state: Signal<EditorShellState>, next_name: String) {
    let trimmed = next_name.trim();
    if trimmed.is_empty() {
        return;
    }
    if !state.read().backend_available {
        let mut current = state.write();
        current.project.name = trimmed.to_string();
        current.project_name_input = trimmed.to_string();
        return;
    }
    if let Err(err) = backend::project_rename(trimmed.to_string()).await {
        state.write().status_message = Some(err);
        return;
    }
    let operation_id = state
        .read()
        .command_trace
        .active_operation_id
        .unwrap_or_default();
    run_effects(state, post_mutation_effects(false), operation_id).await;
}

async fn request_project_action_internal(
    mut state: Signal<EditorShellState>,
    action: PendingProjectAction,
) {
    if !state.read().backend_available {
        state.write().status_message =
            Some("Project actions require the live backend.".to_string());
        return;
    }
    if state.read().project.dirty {
        state.write().pending_project_action = Some(action);
        return;
    }
    execute_project_action_internal(state, action).await;
}

async fn execute_project_action_internal(
    mut state: Signal<EditorShellState>,
    action: PendingProjectAction,
) {
    match action {
        PendingProjectAction::NewProject => {
            match backend::project_new(Some("Untitled".to_string())).await {
                Ok(_) => {
                    let operation_id = state
                        .read()
                        .command_trace
                        .active_operation_id
                        .unwrap_or_default();
                    run_effects(
                        state,
                        vec![EditorEffect::FinishProjectSwap {
                            reset_selection: true,
                        }],
                        operation_id,
                    )
                    .await;
                }
                Err(error) => {
                    state.write().status_message = Some(error);
                }
            }
        }
        PendingProjectAction::OpenProject => {
            if !state.read().backend_available {
                state.write().status_message =
                    Some("Open Project requires the live backend.".to_string());
                return;
            }
            match backend::project_pick_open_path().await {
                Ok(Some(path)) => {
                    if let Err(error) = backend::project_open_path(path.clone()).await {
                        state.write().status_message = Some(error);
                        return;
                    }
                    let operation_id = state
                        .read()
                        .command_trace
                        .active_operation_id
                        .unwrap_or_default();
                    run_effects(
                        state,
                        vec![EditorEffect::FinishProjectSwap {
                            reset_selection: true,
                        }],
                        operation_id,
                    )
                    .await;
                }
                Ok(None) => {}
                Err(error) => {
                    state.write().status_message = Some(error);
                }
            }
        }
        PendingProjectAction::QuitApp => {
            if let Err(error) = backend::app_quit().await {
                state.write().status_message = Some(error);
            }
        }
        PendingProjectAction::CloseWindow => {
            if let Err(error) = backend::window_close_main().await {
                state.write().status_message = Some(error);
            }
        }
    }
}

async fn confirm_unsaved_save_internal(mut state: Signal<EditorShellState>) -> bool {
    if save_project_internal(state, false).await {
        let action = state.write().pending_project_action.take();
        if let Some(action) = action {
            execute_project_action_internal(state, action).await;
        }
        true
    } else {
        false
    }
}

async fn confirm_unsaved_discard_internal(mut state: Signal<EditorShellState>) {
    let action = state.write().pending_project_action.take();
    if let Some(action) = action {
        execute_project_action_internal(state, action).await;
    }
}

async fn save_project_internal(mut state: Signal<EditorShellState>, force_dialog: bool) -> bool {
    if !state.read().backend_available {
        state.write().status_message = Some("Save requires the live backend.".to_string());
        return false;
    }

    let path = if force_dialog || state.read().project.path.is_none() {
        match backend::project_pick_save_path().await {
            Ok(path) => path,
            Err(error) => {
                state.write().status_message = Some(error);
                return false;
            }
        }
    } else {
        state.read().project.path.clone()
    };

    let result = match path {
        Some(path) => backend::project_save(Some(path)).await,
        None => return false,
    };

    match result {
        Ok(message) => {
            {
                let mut current = state.write();
                current.recovery_generation = current.recovery_generation.saturating_add(1);
                current.recovery_path = None;
                current.recovery_open = false;
                current.lifecycle.recovery =
                    SubsystemReadiness::ready("Recovery cleared after save.");
            }
            hydrate_snapshot(state, false).await;
            state.write().status_message = Some(message);
            true
        }
        Err(error) => {
            state.write().status_message = Some(error);
            false
        }
    }
}

async fn restore_recovery_snapshot_internal(mut state: Signal<EditorShellState>) {
    if !state.read().backend_available {
        state.write().recovery_open = false;
        return;
    }
    match backend::project_recovery_load().await {
        Ok(_) => {
            {
                let mut current = state.write();
                current.recovery_open = false;
                current.pending_project_action = None;
                current.status_message = Some("Restored recovery snapshot.".to_string());
                current.lifecycle.recovery =
                    SubsystemReadiness::recovering("Recovery snapshot restored.");
            }
            let operation_id = state
                .read()
                .command_trace
                .active_operation_id
                .unwrap_or_default();
            run_effects(
                state,
                vec![EditorEffect::FinishProjectSwap {
                    reset_selection: true,
                }],
                operation_id,
            )
            .await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

async fn discard_recovery_snapshot_internal(mut state: Signal<EditorShellState>) {
    if !state.read().backend_available {
        state.write().recovery_open = false;
        return;
    }
    match backend::project_recovery_clear().await {
        Ok(message) => {
            let mut current = state.write();
            current.recovery_open = false;
            current.recovery_path = None;
            current.status_message = Some(message);
            current.lifecycle.recovery = SubsystemReadiness::ready("Recovery snapshot discarded.");
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

async fn undo_history_internal(mut state: Signal<EditorShellState>) {
    if !state.read().backend_available {
        return;
    }
    match backend::history_undo().await {
        Ok(status) => {
            state.write().history_status = status;
            let operation_id = state
                .read()
                .command_trace
                .active_operation_id
                .unwrap_or_default();
            run_effects(state, post_mutation_effects(false), operation_id).await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

async fn redo_history_internal(mut state: Signal<EditorShellState>) {
    if !state.read().backend_available {
        return;
    }
    match backend::history_redo().await {
        Ok(status) => {
            state.write().history_status = status;
            let operation_id = state
                .read()
                .command_trace
                .active_operation_id
                .unwrap_or_default();
            run_effects(state, post_mutation_effects(false), operation_id).await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

async fn upsert_sample_load_internal(mut state: Signal<EditorShellState>) {
    if !state.read().backend_available {
        state.write().status_message = Some("Sample loads require the live backend.".to_string());
        return;
    }

    let sample_id = state.read().sample_id_input.trim().to_string();
    let source = state.read().sample_source_input.trim().to_string();
    if sample_id.is_empty() || source.is_empty() {
        state.write().status_message =
            Some("Sample loads need both an id and a source.".to_string());
        return;
    }

    let aliases_input = state.read().sample_aliases_input.clone();
    let aliases = if aliases_input.trim().is_empty() {
        BTreeMap::new()
    } else {
        match serde_json::from_str::<BTreeMap<String, String>>(&aliases_input) {
            Ok(value) => value,
            Err(error) => {
                state.write().status_message =
                    Some(format!("Sample aliases must be valid JSON: {error}"));
                return;
            }
        }
    };

    apply_init_ops_internal(
        state,
        vec![InitStageOp::SampleLoadUpsert {
            id: sample_id,
            source,
            aliases,
        }],
    )
    .await;
}

async fn create_trick_internal(mut state: Signal<EditorShellState>) {
    if !state.read().backend_available {
        state.write().status_message =
            Some("Trick creation requires the live backend.".to_string());
        return;
    }

    let trick_name = state.read().trick_name_input.trim().to_string();
    if trick_name.is_empty() {
        state.write().status_message = Some("Trick name cannot be empty.".to_string());
        return;
    }

    let trick_id = slugify_identifier(&trick_name);
    apply_init_ops_internal(
        state,
        vec![InitStageOp::TrickCreate {
            id: trick_id,
            name: trick_name,
            graph: None,
        }],
    )
    .await;
}

async fn apply_init_ops_internal(mut state: Signal<EditorShellState>, ops: Vec<InitStageOp>) {
    if ops.is_empty() {
        return;
    }
    match backend::project_init_apply(ops).await {
        Ok(_) => {
            state.write().sample_id_input.clear();
            state.write().sample_source_input.clear();
            state.write().sample_aliases_input.clear();
            state.write().trick_name_input.clear();
            let operation_id = state
                .read()
                .command_trace
                .active_operation_id
                .unwrap_or_default();
            run_effects(state, post_mutation_effects(false), operation_id).await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

async fn export_song_internal(mut state: Signal<EditorShellState>) {
    if !state.read().backend_available {
        state.write().status_message = Some("Export Song requires the live backend.".to_string());
        return;
    }

    let path = match backend::export_pick_song_path().await {
        Ok(Some(path)) => path,
        Ok(None) => return,
        Err(error) => {
            state.write().status_message = Some(error);
            return;
        }
    };

    let cpm = state.read().cpm_input.trim().parse::<f32>().ok();
    match backend::export_song(path, cpm).await {
        Ok(result) => {
            {
                let mut current = state.write();
                current.status_message = Some(result.message.clone());
                if !result.exported {
                    current.compile_open = true;
                    current.activity_tab = ActivityTab::Preview;
                }
            }
            refresh_runtime_telemetry_internal(state).await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

async fn play_runtime_internal(mut state: Signal<EditorShellState>) {
    if !state.read().backend_available {
        state.write().status_message =
            Some("Runtime playback requires the live backend.".to_string());
        return;
    }

    if let Err(error) = runtime::prime_audio_from_gesture().await {
        state.write().status_message = Some(error);
        return;
    }
    if let Err(error) = runtime::ensure_runtime_ready().await {
        state.write().status_message = Some(error);
        refresh_runtime_bridge_state(state);
        return;
    }

    let cpm = state.read().cpm_input.trim().parse::<f32>().ok();
    let request_id = state.read().runtime_status.rev.saturating_add(1);
    let commit = match backend::runtime_commit(RuntimeCommitArgs {
        cpm,
        force: Some(false),
        playing: Some(true),
        request_id: Some(request_id),
    })
    .await
    {
        Ok(commit) => commit,
        Err(error) => {
            state.write().status_message = Some(error);
            return;
        }
    };

    state.write().runtime_status = commit.status.clone();

    if !commit.success {
        {
            let mut current = state.write();
            current.status_message = Some(
                commit
                    .error
                    .unwrap_or_else(|| "Playback preview could not start.".to_string()),
            );
            current.compile_open = true;
            current.activity_tab = ActivityTab::Issues;
            current.lifecycle.runtime =
                SubsystemReadiness::failed("Runtime commit rejected playback preview.");
        }
        refresh_runtime_telemetry_internal(state).await;
        return;
    }

    refresh_runtime_telemetry_internal(state).await;

    let runtime_status = state.read().runtime_status.clone();
    if runtime_status.playing {
        state.write().status_message = Some(format!(
            "Playback is ready across {} output lane(s).",
            commit.output_count
        ));
        state.write().lifecycle.runtime = SubsystemReadiness::ready("Playback preview is running.");
    } else {
        let issue = runtime_status.last_error.clone().unwrap_or_else(|| {
            "Runtime accepted the play request but did not start playback.".to_string()
        });
        let mut current = state.write();
        current.status_message = Some(issue.clone());
        current.compile_open = true;
        current.activity_tab = ActivityTab::Issues;
        current.lifecycle.runtime = SubsystemReadiness::failed(issue);
    }
}

async fn stop_runtime_internal(mut state: Signal<EditorShellState>) {
    if state.read().backend_available {
        match backend::runtime_stop().await {
            Ok(status) => {
                state.write().runtime_status = status;
            }
            Err(error) => {
                state.write().status_message = Some(error);
            }
        }
    }
    refresh_runtime_bridge_state(state);
}

async fn set_mini_console_visible_internal(mut state: Signal<EditorShellState>, visible: bool) {
    if state.read().backend_available
        && let Err(error) = backend::ui_set_mini_console_visible(visible).await
    {
        state.write().status_message = Some(error);
        return;
    }
    set_runtime_panel_visibility(&mut state.write(), visible);
}

async fn toggle_dev_inspector_internal(mut state: Signal<EditorShellState>) {
    let next = !state.read().diagnostics.devtools_visible;
    if let Err(error) = backend::ui_set_devtools_visible(next).await {
        state.write().status_message = Some(error);
    } else {
        state.write().diagnostics.devtools_visible = next;
    }
}

fn set_runtime_panel_visibility(state: &mut EditorState, visible: bool) {
    state.diagnostics.mini_console_visible = visible;
    state.compile_open = visible;
    if visible {
        state.activity_tab = ActivityTab::Diagnostics;
    } else {
        state.status_message = None;
    }
}

fn schedule_recovery_write_internal(mut state: Signal<EditorShellState>) {
    if !state.read().backend_available {
        return;
    }
    let generation = {
        let mut current = state.write();
        current.recovery_generation = current.recovery_generation.saturating_add(1);
        current.lifecycle.recovery =
            SubsystemReadiness::loading("Scheduling recovery snapshot write…");
        current.recovery_generation
    };
    spawn(async move {
        if runtime::sleep_ms(800).await.is_err() {
            return;
        }
        if state.read().recovery_generation != generation {
            return;
        }
        if !state.read().project.dirty {
            return;
        }
        match backend::project_recovery_write().await {
            Ok(message) => {
                let mut current = state.write();
                current.recovery_path = parse_recovery_path(message.as_str());
                current.lifecycle.recovery = current
                    .recovery_path
                    .as_ref()
                    .map(|_| SubsystemReadiness::recovering("Recovery snapshot updated."))
                    .unwrap_or_else(|| SubsystemReadiness::ready("Recovery idle."));
            }
            Err(error) => {
                state.write().status_message = Some(error.clone());
                state.write().lifecycle.recovery = SubsystemReadiness::failed(error);
            }
        }
    });
}

async fn run_effects(
    state: Signal<EditorShellState>,
    effects: Vec<EditorEffect>,
    operation_id: u64,
) {
    for effect in effects {
        trace_effect(operation_id, "scheduled", &effect);
        match effect {
            EditorEffect::RefreshRuntimeBridge => {
                refresh_runtime_bridge_state(state);
                trace_effect(
                    operation_id,
                    "completed",
                    &EditorEffect::RefreshRuntimeBridge,
                );
            }
            EditorEffect::ScheduleRecoveryWrite => {
                trace_event(operation_id, &EditorEvent::RecoveryWriteScheduled);
                schedule_recovery_write_internal(state);
                trace_effect(
                    operation_id,
                    "completed",
                    &EditorEffect::ScheduleRecoveryWrite,
                );
            }
            EditorEffect::ReloadSnapshot { reset_selection } => {
                trace_event(operation_id, &EditorEvent::RuntimeRefreshScheduled);
                hydrate_snapshot(state, reset_selection).await;
                trace_effect(
                    operation_id,
                    "completed",
                    &EditorEffect::ReloadSnapshot { reset_selection },
                );
            }
            EditorEffect::FinishProjectSwap { reset_selection } => {
                let _ = backend::runtime_stop().await;
                refresh_runtime_bridge_state(state);
                hydrate_snapshot(state, reset_selection).await;
                trace_effect(
                    operation_id,
                    "completed",
                    &EditorEffect::FinishProjectSwap { reset_selection },
                );
            }
        }
    }
}

fn post_mutation_effects(reset_selection: bool) -> Vec<EditorEffect> {
    vec![
        EditorEffect::ReloadSnapshot { reset_selection },
        EditorEffect::ScheduleRecoveryWrite,
        EditorEffect::RefreshRuntimeBridge,
    ]
}

fn begin_operation(mut state: Signal<EditorShellState>, action: &EditorAction) -> u64 {
    let mut current = state.write();
    let operation_id = current.command_trace.next_operation_id;
    current.command_trace.next_operation_id =
        current.command_trace.next_operation_id.saturating_add(1);
    current.command_trace.active_operation_id = Some(operation_id);
    current.command_trace.active_operation_label = Some(action_label(action).to_string());
    operation_id
}

fn finish_operation(mut state: Signal<EditorShellState>, operation_id: u64) {
    let mut current = state.write();
    current.command_trace.active_operation_id = None;
    current.command_trace.active_operation_label = None;
    current.command_trace.last_completed_operation_id = Some(operation_id);
}

fn action_label(action: &EditorAction) -> &'static str {
    match action {
        EditorAction::Dispatch(_) => "dispatch_command",
        EditorAction::LoadSnapshot { .. } => "load_snapshot",
        EditorAction::SwitchWorkspace { .. } => "switch_workspace",
        EditorAction::RefreshRuntimeTelemetry => "refresh_runtime_telemetry",
        EditorAction::PlacePiece { .. } => "place_piece",
        EditorAction::DeleteSelectedNode => "delete_selected_node",
        EditorAction::MoveNode { .. } => "move_node",
        EditorAction::ConnectNodes { .. } => "connect_nodes",
        EditorAction::SetParamSide { .. } => "set_param_side",
        EditorAction::SetOutputSide { .. } => "set_output_side",
        EditorAction::DisconnectEdge { .. } => "disconnect_edge",
        EditorAction::RenameProject { .. } => "rename_project",
        EditorAction::RequestProjectAction { .. } => "request_project_action",
        EditorAction::ConfirmUnsavedSave => "confirm_unsaved_save",
        EditorAction::ConfirmUnsavedDiscard => "confirm_unsaved_discard",
        EditorAction::SaveProject { .. } => "save_project",
        EditorAction::RestoreRecoverySnapshot => "restore_recovery_snapshot",
        EditorAction::DiscardRecoverySnapshot => "discard_recovery_snapshot",
        EditorAction::UndoHistory => "undo_history",
        EditorAction::RedoHistory => "redo_history",
        EditorAction::UpsertSampleLoad => "upsert_sample_load",
        EditorAction::CreateTrick => "create_trick",
        EditorAction::PlayRuntime => "play_runtime",
        EditorAction::StopRuntime => "stop_runtime",
    }
}

fn trace_event(operation_id: u64, event: &EditorEvent) {
    eprintln!("[editor-op:{operation_id}] event={event:?}");
}

fn trace_effect(operation_id: u64, phase: &str, effect: &EditorEffect) {
    eprintln!("[editor-op:{operation_id}] effect.{phase}={effect:?}");
}

fn open_picker_internal(
    state: &mut EditorShellState,
    target: Option<GridPos>,
    reset_filters: bool,
) {
    state.picker_open = true;
    close_command_palette_internal(state);
    if reset_filters {
        state.picker_query.clear();
        state.picker_category = None;
    }
    state.picker_focus_nonce = state.picker_focus_nonce.wrapping_add(1);
    state.picker_target = target.or_else(|| preferred_picker_target(state));
    if let Some(target) = state.picker_target {
        state.selected_cell = Some(target);
        state.selected_node = None;
        state.selected_nodes.clear();
        sync_editor_inputs_internal(state);
    }
}

fn close_picker_internal(state: &mut EditorShellState) {
    state.picker_open = false;
    state.picker_target = None;
}

fn close_command_palette_internal(state: &mut EditorShellState) {
    state.command_palette_open = false;
    state.command_palette_query.clear();
    state.command_palette_selected = 0;
}

fn clear_drag_state_internal(state: &mut EditorShellState) {
    state.interaction_state = None;
    state.drag_hover = None;
    invalidate_drag_preview(state);
}

fn select_tile_internal(state: &mut EditorShellState, position: GridPos, additive: bool) {
    if additive {
        if state.selected_nodes.contains(&position) {
            state
                .selected_nodes
                .retain(|selected| *selected != position);
        } else {
            state.selected_nodes.push(position);
        }
        state.selected_node = state.selected_nodes.first().copied().or(Some(position));
    } else {
        state.selected_node = Some(position);
        state.selected_nodes = vec![position];
    }
    state.selected_cell = Some(position);
    state.inspector.open = true;
    normalize_selected_nodes(state);
    sync_editor_inputs_internal(state);
}

fn select_cell_internal(state: &mut EditorShellState, position: GridPos, additive: bool) {
    state.selected_cell = Some(position);
    if !additive {
        state.selected_node = None;
        state.selected_nodes.clear();
        state.inspector.open = false;
    }
    sync_editor_inputs_internal(state);
}

fn begin_tile_drag_internal(state: &mut EditorShellState, position: GridPos) {
    select_tile_internal(state, position, false);
    let board_page_origin = board_origin_for_state(state);
    let current_page = page_point_for_grid(position, state);
    state.interaction_state = Some(InteractionState::Dragging(DragSession::NodeMove {
        from: position,
        pointer_id: 0,
        board_page_origin,
        current_page,
    }));
    state.drag_hover = Some(drag_hover_for_position(state, position, position));
    state.drag_preview = Some(move_preview_feedback(position));
}

fn preview_tile_drag_internal(state: &mut EditorShellState, position: GridPos) {
    let from = match state.interaction_state.as_ref() {
        Some(InteractionState::Dragging(DragSession::NodeMove { from, .. })) => *from,
        _ => return,
    };
    let board_page_origin = board_origin_for_state(state);
    let current_page = page_point_for_grid(position, state);
    state.interaction_state = Some(InteractionState::Dragging(DragSession::NodeMove {
        from,
        pointer_id: 0,
        board_page_origin,
        current_page,
    }));
    let hover = drag_hover_for_position(state, from, position);
    state.drag_hover = Some(hover.clone());
    state.drag_preview = Some(match hover.status {
        DragHoverStatus::Invalid => blocked_preview_feedback(position, hover.reason.clone()),
        _ => move_preview_feedback(position),
    });
}

async fn commit_tile_drag_internal(mut state: Signal<EditorShellState>, position: GridPos) {
    let from = {
        let current = state.read();
        match current.interaction_state.as_ref() {
            Some(InteractionState::Dragging(DragSession::NodeMove { from, .. })) => *from,
            _ => {
                drop(current);
                select_tile_internal(&mut state.write(), position, false);
                return;
            }
        }
    };
    clear_drag_state_internal(&mut state.write());
    move_node_internal(state, from, position).await;
}

fn begin_connection_internal(state: &mut EditorShellState, from: GridPos) {
    select_tile_internal(state, from, false);
    let board_page_origin = board_origin_for_state(state);
    let current_page = page_point_for_grid(from, state);
    state.interaction_state = Some(InteractionState::Dragging(DragSession::EdgeFrom {
        from,
        pointer_id: 0,
        board_page_origin,
        current_page,
    }));
    state.drag_preview = Some(DragPreviewFeedback {
        position: from,
        drag_kind: DragPreviewKind::NodeMove,
        kind: DragPreviewOutcomeKind::Connect,
        target: None,
        target_label: None,
        headline: "Connect tiles".to_string(),
        detail: "Release over another tile to connect it.".to_string(),
    });
}

fn preview_connection_internal(state: &mut EditorShellState, position: GridPos) {
    let from = match state.interaction_state.as_ref() {
        Some(InteractionState::Dragging(DragSession::EdgeFrom { from, .. })) => *from,
        _ => return,
    };
    let board_page_origin = board_origin_for_state(state);
    let current_page = page_point_for_grid(position, state);
    state.interaction_state = Some(InteractionState::Dragging(DragSession::EdgeFrom {
        from,
        pointer_id: 0,
        board_page_origin,
        current_page,
    }));
    state.drag_hover = Some(drag_hover_for_position(state, from, position));
    state.drag_preview = Some(DragPreviewFeedback {
        position,
        drag_kind: DragPreviewKind::NodeMove,
        kind: DragPreviewOutcomeKind::Connect,
        target: Some(position),
        target_label: None,
        headline: "Will connect".to_string(),
        detail: "Release to connect these tiles.".to_string(),
    });
}

async fn commit_connection_internal(mut state: Signal<EditorShellState>, position: GridPos) {
    let from = match state.read().interaction_state.as_ref() {
        Some(InteractionState::Dragging(DragSession::EdgeFrom { from, .. })) => *from,
        _ => return,
    };
    clear_drag_state_internal(&mut state.write());
    connect_nodes_internal(state, from, position).await;
}

fn begin_marquee_internal(state: &mut EditorShellState, origin: GridPos, additive: bool) {
    let board_page_origin = board_origin_for_state(state);
    let origin_page = page_point_for_grid(origin, state);
    state.interaction_state = Some(InteractionState::Dragging(DragSession::CanvasMarquee {
        pointer_id: 0,
        board_page_origin,
        anchor: PointerPoint {
            x: origin_page.x - board_page_origin.x,
            y: origin_page.y - board_page_origin.y,
        },
        current: PointerPoint {
            x: origin_page.x - board_page_origin.x,
            y: origin_page.y - board_page_origin.y,
        },
        additive,
    }));
    state.selected_cell = Some(origin);
    state.drag_preview = None;
}

fn preview_marquee_internal(state: &mut EditorShellState, current: GridPos) {
    let origin = match state.interaction_state.as_ref() {
        Some(InteractionState::Dragging(DragSession::CanvasMarquee {
            board_page_origin,
            anchor,
            additive,
            ..
        })) => Some((*board_page_origin, *anchor, *additive)),
        _ => None,
    };
    let Some((board_page_origin, anchor, additive)) = origin else {
        return;
    };
    let current_page = page_point_for_grid(current, state);
    state.interaction_state = Some(InteractionState::Dragging(DragSession::CanvasMarquee {
        pointer_id: 0,
        board_page_origin,
        anchor,
        current: PointerPoint {
            x: current_page.x - board_page_origin.x,
            y: current_page.y - board_page_origin.y,
        },
        additive,
    }));
}

fn commit_marquee_internal(state: &mut EditorShellState) {
    let (anchor, current, additive) = match state.interaction_state.as_ref() {
        Some(InteractionState::Dragging(DragSession::CanvasMarquee {
            anchor,
            current,
            additive,
            ..
        })) => (*anchor, *current, *additive),
        _ => return,
    };
    let selected = state
        .graph
        .nodes
        .iter()
        .filter_map(|node| {
            let page = page_point_for_grid(node.position, state);
            let x =
                page.x - f64::from(state.graph.cols.min(i32::MAX as u32) as i32 * 0) - f64::from(0);
            let y =
                page.y - f64::from(state.graph.rows.min(i32::MAX as u32) as i32 * 0) - f64::from(0);
            point_in_rect(PointerPoint { x, y }, anchor, current).then_some(node.position)
        })
        .collect::<Vec<_>>();
    if !additive {
        state.selected_nodes.clear();
    }
    for position in selected {
        if !state.selected_nodes.contains(&position) {
            state.selected_nodes.push(position);
        }
    }
    state.selected_node = state.selected_nodes.first().copied();
    clear_drag_state_internal(state);
    normalize_selected_nodes(state);
    sync_editor_inputs_internal(state);
}

fn board_origin_for_state(_state: &EditorShellState) -> PointerPoint {
    PointerPoint { x: 0.0, y: 0.0 }
}

fn page_point_for_grid(position: GridPos, _state: &EditorShellState) -> PointerPoint {
    PointerPoint {
        x: f64::from(
            crate::domain::GRID_ORIGIN_X
                + position.col * crate::domain::CELL_W
                + crate::domain::CELL_W / 2,
        ),
        y: f64::from(
            crate::domain::GRID_ORIGIN_Y
                + position.row * crate::domain::CELL_H
                + crate::domain::CELL_H / 2,
        ),
    }
}

fn drag_hover_for_position(
    state: &EditorShellState,
    from: GridPos,
    position: GridPos,
) -> DragHover {
    let outside_grid = position.col < 0
        || position.row < 0
        || position.col >= state.graph.cols as i32
        || position.row >= state.graph.rows as i32;
    if outside_grid {
        return DragHover {
            position,
            status: DragHoverStatus::Invalid,
            reason: Some(DragHoverReason::OutsideGrid),
        };
    }
    if position == from {
        return DragHover {
            position,
            status: DragHoverStatus::Valid,
            reason: Some(DragHoverReason::MoveOnly),
        };
    }
    let occupied = is_cell_occupied(&state.graph, &position);
    DragHover {
        position,
        status: if occupied {
            DragHoverStatus::Swap
        } else {
            DragHoverStatus::Valid
        },
        reason: if occupied {
            Some(DragHoverReason::OccupiedTarget)
        } else {
            Some(DragHoverReason::MoveOnly)
        },
    }
}

fn move_preview_feedback(position: GridPos) -> DragPreviewFeedback {
    DragPreviewFeedback {
        position,
        drag_kind: DragPreviewKind::NodeMove,
        kind: DragPreviewOutcomeKind::MoveOnly,
        target: None,
        target_label: None,
        headline: "Move tile".to_string(),
        detail: "Release to move the selected tile.".to_string(),
    }
}

fn blocked_preview_feedback(
    position: GridPos,
    reason: Option<DragHoverReason>,
) -> DragPreviewFeedback {
    let detail = match reason {
        Some(DragHoverReason::OutsideGrid) => "Blocked by outside-grid bounds",
        Some(DragHoverReason::OccupiedTarget) => "Blocked by occupied target",
        Some(DragHoverReason::Collision) => "Blocked by collision",
        _ => "Blocked by invalid target",
    };
    DragPreviewFeedback {
        position,
        drag_kind: DragPreviewKind::NodeMove,
        kind: DragPreviewOutcomeKind::Blocked,
        target: None,
        target_label: None,
        headline: detail.to_string(),
        detail: detail.to_string(),
    }
}

fn point_in_rect(point: PointerPoint, a: PointerPoint, b: PointerPoint) -> bool {
    let left = a.x.min(b.x);
    let right = a.x.max(b.x);
    let top = a.y.min(b.y);
    let bottom = a.y.max(b.y);
    point.x >= left && point.x <= right && point.y >= top && point.y <= bottom
}

fn preferred_picker_target(state: &EditorShellState) -> Option<GridPos> {
    state
        .selected_cell
        .filter(|pos| !is_cell_occupied(&state.graph, pos))
        .or_else(|| first_free_position(&state.graph))
}

fn sync_editor_inputs_internal(state: &mut EditorShellState) {
    let Some(node) = selected_node_view(state).cloned() else {
        state.inspector.label_input.clear();
        state.inspector.param_inputs.clear();
        return;
    };
    state.inspector.label_input = node.label.clone().unwrap_or_default();
    state.inspector.param_inputs.clear();
}

fn parse_recovery_path(message: &str) -> Option<String> {
    message
        .split_once("wrote recovery snapshot to ")
        .map(|(_, path)| path.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::PieceDef,
        application::editor::{ReadinessState, empty_project_view},
        domain::GraphNodeView,
    };

    fn make_piece(id: &str) -> PieceDef {
        PieceDef {
            id: id.to_string(),
            label: id.to_string(),
            category: "test".to_string(),
            semantic_kind: "test".to_string(),
            namespace: "cadence".to_string(),
            params: vec![crate::adapter::ParamDef {
                id: "value".to_string(),
                label: "value".to_string(),
                side: "left".to_string(),

                text_semantics: None,
                input_context: None,
                variadic_group: None,
                required: false,
            }],

            output_side: Some("right".to_string()),
            description: None,
            tags: Vec::new(),

            bundle_input: None,
            temporal_kind: String::new(),
            fan_in: String::new(),
            fan_out: String::new(),
        }
    }

    #[test]
    fn post_mutation_effects_are_centralized_and_ordered() {
        assert_eq!(
            post_mutation_effects(false),
            vec![
                EditorEffect::ReloadSnapshot {
                    reset_selection: false
                },
                EditorEffect::ScheduleRecoveryWrite,
                EditorEffect::RefreshRuntimeBridge,
            ]
        );
    }

    #[test]
    fn merge_loaded_snapshot_updates_lifecycle_and_syncs_selection() {
        let mut current = EditorState::default();
        current.backend_available = false;
        current.catalog = vec![make_piece("piece")];
        current.selected_cell = Some(GridPos { col: 2, row: 3 });

        let loaded = LoadedSnapshot {
            backend_available: true,
            project: empty_project_view(),
            graph: GraphView {
                name: "runtime".to_string(),
                cols: 8,
                rows: 8,
                nodes: vec![GraphNodeView {
                    position: GridPos { col: 0, row: 0 },
                    piece_id: "piece".to_string(),
                    inline_params: BTreeMap::new(),
                    pattern_source: None,
                    input_sides: BTreeMap::new(),
                    output_side: Some("right".to_string()),
                    label: Some("kick".to_string()),
                    node_state: None,
                }],
                edges: Vec::new(),
            },
            catalog: vec![make_piece("piece")],
            init_stage: empty_init_stage(),
            sample_library: Some(empty_sample_library()),
            project_preview: empty_project_preview(),
            graph_preview: empty_graph_preview(),
            history_status: default_history_status(),
            runtime_status: default_runtime_status(),
            diagnostics: default_diagnostics_snapshot(),
            recovery_path: Some("/tmp/cadence.recovery.json".to_string()),
        };

        let merged = merge_loaded_snapshot(&current, loaded, true);
        assert_eq!(merged.lifecycle.backend.state, ReadinessState::Ready);
        assert_eq!(merged.lifecycle.hydration.state, ReadinessState::Ready);
        assert_eq!(merged.lifecycle.recovery.state, ReadinessState::Recovering);
        assert_eq!(merged.selected_node, Some(GridPos { col: 0, row: 0 }));
        assert_eq!(merged.inspector.label_input, "kick".to_string());
    }

    #[test]
    fn runtime_bridge_refresh_marks_backend_unavailable_when_offline() {
        let mut state = EditorState::default();
        state.backend_available = false;
        refresh_runtime_bridge_state_internal(&mut state);
        assert_eq!(state.lifecycle.runtime.state, ReadinessState::Unavailable);
    }

    #[test]
    fn local_auto_connect_targets_are_ranked_by_distance_then_reading_order() {
        let graph = GraphView {
            name: "runtime".to_string(),
            cols: 8,
            rows: 8,
            nodes: vec![
                GraphNodeView {
                    position: GridPos { col: 2, row: 2 },
                    piece_id: "source".to_string(),
                    inline_params: BTreeMap::new(),
                    pattern_source: None,
                    input_sides: BTreeMap::new(),
                    output_side: Some("right".to_string()),
                    label: None,
                    node_state: None,
                },
                GraphNodeView {
                    position: GridPos { col: 2, row: 1 },
                    piece_id: "north".to_string(),
                    inline_params: BTreeMap::new(),
                    pattern_source: None,
                    input_sides: BTreeMap::new(),
                    output_side: Some("right".to_string()),
                    label: None,
                    node_state: None,
                },
                GraphNodeView {
                    position: GridPos { col: 1, row: 2 },
                    piece_id: "west".to_string(),
                    inline_params: BTreeMap::new(),
                    pattern_source: None,
                    input_sides: BTreeMap::new(),
                    output_side: Some("right".to_string()),
                    label: None,
                    node_state: None,
                },
                GraphNodeView {
                    position: GridPos { col: 3, row: 2 },
                    piece_id: "east".to_string(),
                    inline_params: BTreeMap::new(),
                    pattern_source: None,
                    input_sides: BTreeMap::new(),
                    output_side: Some("right".to_string()),
                    label: None,
                    node_state: None,
                },
                GraphNodeView {
                    position: GridPos { col: 2, row: 3 },
                    piece_id: "south".to_string(),
                    inline_params: BTreeMap::new(),
                    pattern_source: None,
                    input_sides: BTreeMap::new(),
                    output_side: Some("right".to_string()),
                    label: None,
                    node_state: None,
                },
            ],
            edges: Vec::new(),
        };

        let targets = local_auto_connect_targets(&graph, GridPos { col: 2, row: 2 });
        assert_eq!(
            targets,
            vec![
                GridPos { col: 2, row: 1 },
                GridPos { col: 1, row: 2 },
                GridPos { col: 3, row: 2 },
                GridPos { col: 2, row: 3 },
            ]
        );
    }

    #[test]
    fn auto_connect_candidate_class_prefers_direct_then_repairable() {
        let direct = backend::GraphPickTargetParamDto {
            to_param: Some("pattern".to_string()),
            implicit_bridge: None,
            reason: None,
            detail: None,
            suggestions: Vec::new(),
        };
        let repairable = backend::GraphPickTargetParamDto {
            to_param: None,
            implicit_bridge: None,
            reason: Some(backend::EdgeConnectProbeReason::SideMismatch),
            detail: Some("fix side".to_string()),
            suggestions: vec![backend::RepairSuggestion::SetOutputSide {
                position: GridPos { col: 0, row: 0 },
                side: backend::TileSide::Left,
            }],
        };
        let rejected = backend::GraphPickTargetParamDto {
            to_param: None,
            implicit_bridge: None,
            reason: Some(backend::EdgeConnectProbeReason::NotAdjacent),
            detail: Some("move it".to_string()),
            suggestions: vec![backend::RepairSuggestion::MoveNode {
                node: GridPos { col: 0, row: 0 },
                to: GridPos { col: 1, row: 0 },
            }],
        };

        assert_eq!(
            auto_connect_candidate_class(&direct),
            AutoConnectCandidateClass::Direct
        );
        assert_eq!(
            auto_connect_candidate_class(&repairable),
            AutoConnectCandidateClass::Repairable
        );
        assert_eq!(
            auto_connect_candidate_class(&rejected),
            AutoConnectCandidateClass::Rejected
        );
    }

    #[test]
    fn merge_loaded_snapshot_clears_stale_auto_connect_feedback() {
        let mut current = EditorState::default();
        current.auto_connect_feedback = Some(AutoConnectFeedback {
            position: GridPos { col: 3, row: 2 },
            kind: AutoConnectOutcomeKind::Blocked,
            headline: "old".to_string(),
            detail: "old".to_string(),
        });

        let loaded = LoadedSnapshot {
            backend_available: true,
            project: empty_project_view(),
            graph: empty_graph(),
            catalog: empty_catalog(),
            init_stage: empty_init_stage(),
            sample_library: Some(empty_sample_library()),
            project_preview: empty_project_preview(),
            graph_preview: empty_graph_preview(),
            history_status: default_history_status(),
            runtime_status: default_runtime_status(),
            diagnostics: default_diagnostics_snapshot(),
            recovery_path: None,
        };

        let merged = merge_loaded_snapshot(&current, loaded, false);
        assert_eq!(merged.auto_connect_feedback, None);
    }

    #[test]
    fn merge_loaded_snapshot_clears_stale_drag_preview() {
        let mut current = EditorState::default();
        current.drag_preview = Some(DragPreviewFeedback {
            position: GridPos { col: 4, row: 1 },
            drag_kind: DragPreviewKind::NodeMove,
            kind: DragPreviewOutcomeKind::Connect,
            target: Some(GridPos { col: 5, row: 1 }),
            target_label: Some("Gain.amount".to_string()),
            headline: "Will connect".to_string(),
            detail: "stale".to_string(),
        });

        let loaded = LoadedSnapshot {
            backend_available: true,
            project: empty_project_view(),
            graph: empty_graph(),
            catalog: empty_catalog(),
            init_stage: empty_init_stage(),
            sample_library: Some(empty_sample_library()),
            project_preview: empty_project_preview(),
            graph_preview: empty_graph_preview(),
            history_status: default_history_status(),
            runtime_status: default_runtime_status(),
            diagnostics: default_diagnostics_snapshot(),
            recovery_path: None,
        };

        let merged = merge_loaded_snapshot(&current, loaded, false);
        assert_eq!(merged.drag_preview, None);
    }
}
