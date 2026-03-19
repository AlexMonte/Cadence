use super::*;

pub(super) async fn load_snapshot(mut state: Signal<EditorShellState>, reset_selection: bool) {
    state.write().loading = true;

    let mode = state.read().workspace_mode.clone();
    match fetch_snapshot(mode.clone()).await {
        Ok(loaded) => {
            if state.read().workspace_mode != mode {
                return;
            }
            let next = merge_loaded_snapshot(&state.read().clone(), loaded, reset_selection);
            state.set(next);
            refresh_runtime_bridge_state(state);
        }
        Err(error) => {
            if state.read().workspace_mode != mode {
                return;
            }
            let mut current = state.write();
            current.loading = false;
            current.status_message = Some(error);
        }
    }
}

pub(super) async fn fetch_snapshot(mode: WorkspaceMode) -> Result<LoadedSnapshot, String> {
    match fetch_tauri_snapshot(mode.clone()).await {
        Ok(snapshot) => Ok(snapshot),
        Err(error) if is_tauri_unavailable(&error) => Ok(LoadedSnapshot {
            tauri_available: false,
            project: empty_project_view(),
            graph: empty_graph(),
            catalog: empty_catalog(),
            init_stage: empty_init_stage(),
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

pub(super) fn merge_loaded_snapshot(
    current: &EditorShellState,
    loaded: LoadedSnapshot,
    reset_selection: bool,
) -> EditorShellState {
    let previous_selected = current.selected_node;
    let previous_cell = current.selected_cell;
    let mut next = current.clone();
    next.loading = false;
    next.tauri_available = loaded.tauri_available;
    next.project = loaded.project.clone();
    next.project_name_input = loaded.project.name.clone();
    next.graph = loaded.graph;
    next.catalog = loaded.catalog;
    next.init_stage = loaded.init_stage;
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
    close_picker(&mut next);
    close_command_palette(&mut next);
    next.drag_session = None;
    next.drag_hover = None;
    next.pending_probe = None;
    if loaded.tauri_available != current.tauri_available {
        let transition_msg = if loaded.tauri_available {
            "Backend connected. Switched to live mode."
        } else {
            "Backend disconnected. Editor is waiting for the desktop backend."
        };
        next.diagnostics.entries.push(tauri::DiagnosticEntryDto {
            at_ms: 0,
            kind: "mode_transition".to_string(),
            message: transition_msg.to_string(),
        });
    }
    next.status_message = if loaded.tauri_available {
        None
    } else {
        Some(BACKEND_REQUIRED_MESSAGE.to_string())
    };
    if reset_selection {
        next.selected_node = next.graph.nodes.first().map(|node| node.position);
        next.selected_cell = next
            .selected_node
            .or_else(|| first_free_position(&next.graph));
    } else {
        next.selected_node = previous_selected
            .filter(|pos| next.graph.nodes.iter().any(|node| &node.position == pos))
            .or_else(|| next.graph.nodes.first().map(|node| node.position));
        next.selected_cell = previous_cell.or(next.selected_node);
    }
    sync_editor_inputs(&mut next);
    next
}

pub(super) async fn fetch_tauri_snapshot(mode: WorkspaceMode) -> Result<LoadedSnapshot, String> {
    let project = tauri::project_bootstrap().await?;
    let init_stage = tauri::project_init_snapshot().await?;
    let graph_target = mode.graph_target();
    let graph = normalize_graph_view(&tauri::graph_snapshot(graph_target.clone()).await?);
    let catalog = tauri::graph_piece_catalog(graph_target.clone()).await?;
    let graph_preview = tauri::graph_compile_preview(graph_target).await?;
    let project_preview = tauri::project_compile_preview().await?;
    let history_status = tauri::history_status().await?;
    let runtime_status = tauri::runtime_status().await?;
    let diagnostics = tauri::diagnostics_snapshot().await?;
    let recovery_path = tauri::project_recovery_status().await?.path;

    Ok(LoadedSnapshot {
        tauri_available: true,
        project,
        graph,
        catalog,
        init_stage,
        project_preview,
        graph_preview,
        history_status,
        runtime_status,
        diagnostics,
        recovery_path,
    })
}

pub(super) async fn switch_workspace_mode(
    mut state: Signal<EditorShellState>,
    mode: WorkspaceMode,
) {
    {
        let mut current = state.write();
        current.workspace_mode = mode;
        close_picker(&mut current);
        close_command_palette(&mut current);
        clear_drag_state(&mut current);
    }
    load_snapshot(state, true).await;
}
