use super::*;

pub(super) async fn rename_project(mut state: Signal<EditorShellState>, next_name: String) {
    let trimmed = next_name.trim();
    if trimmed.is_empty() {
        return;
    }
    if !state.read().tauri_available {
        let mut current = state.write();
        current.project.name = trimmed.to_string();
        current.project_name_input = trimmed.to_string();
        return;
    }
    tauri::project_rename(trimmed.to_string())
        .await
        .map_err(|err| state.write().status_message = Some(err))
        .ok();
    load_snapshot(state, false).await;
    schedule_recovery_write(state);
}

pub(super) async fn open_project(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        state.write().status_message =
            Some("Open Project requires the desktop backend.".to_string());
        return;
    }

    match tauri::project_pick_open_path().await {
        Ok(Some(path)) => {
            if let Err(error) = tauri::project_open_path(path.clone()).await {
                state.write().status_message = Some(error);
                return;
            }
            finish_project_swap(state, true).await;
        }
        Ok(None) => {}
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

pub(super) async fn request_project_action(
    mut state: Signal<EditorShellState>,
    action: PendingProjectAction,
) {
    if !state.read().tauri_available {
        state.write().status_message =
            Some("Project actions require the desktop backend.".to_string());
        return;
    }
    if state.read().project.dirty {
        state.write().pending_project_action = Some(action);
        return;
    }
    execute_project_action(state, action).await;
}

pub(super) async fn execute_project_action(
    mut state: Signal<EditorShellState>,
    action: PendingProjectAction,
) {
    match action {
        PendingProjectAction::NewProject => {
            match tauri::project_new(Some("Untitled".to_string())).await {
                Ok(_) => {
                    finish_project_swap(state, true).await;
                }
                Err(error) => {
                    state.write().status_message = Some(error);
                }
            }
        }
        PendingProjectAction::OpenProject => {
            open_project(state).await;
        }
        PendingProjectAction::QuitApp => {
            if let Err(error) = tauri::app_quit().await {
                state.write().status_message = Some(error);
            }
        }
        PendingProjectAction::CloseWindow => {
            if let Err(error) = tauri::window_close_main().await {
                state.write().status_message = Some(error);
            }
        }
    }
}

pub(super) async fn confirm_unsaved_save(mut state: Signal<EditorShellState>) {
    if save_project(state, false).await {
        let action = state.write().pending_project_action.take();
        if let Some(action) = action {
            execute_project_action(state, action).await;
        }
    }
}

pub(super) async fn confirm_unsaved_discard(mut state: Signal<EditorShellState>) {
    let action = state.write().pending_project_action.take();
    if let Some(action) = action {
        execute_project_action(state, action).await;
    }
}

pub(super) async fn save_project(mut state: Signal<EditorShellState>, force_dialog: bool) -> bool {
    if !state.read().tauri_available {
        state.write().status_message = Some("Save requires the desktop backend.".to_string());
        return false;
    }

    let path = if force_dialog || state.read().project.path.is_none() {
        match tauri::project_pick_save_path().await {
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
        Some(path) => tauri::project_save(Some(path)).await,
        None => return false,
    };

    match result {
        Ok(message) => {
            state.write().status_message = Some(message);
            load_snapshot(state, false).await;
            true
        }
        Err(error) => {
            state.write().status_message = Some(error);
            false
        }
    }
}

pub(super) async fn restore_recovery_snapshot(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        state.write().recovery_open = false;
        return;
    }
    match tauri::project_recovery_load().await {
        Ok(_) => {
            {
                let mut current = state.write();
                current.recovery_open = false;
                current.pending_project_action = None;
                current.status_message = Some("Restored recovery snapshot.".to_string());
            }
            finish_project_swap(state, true).await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

pub(super) async fn discard_recovery_snapshot(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        state.write().recovery_open = false;
        return;
    }
    match tauri::project_recovery_clear().await {
        Ok(message) => {
            let mut current = state.write();
            current.recovery_open = false;
            current.recovery_path = None;
            current.status_message = Some(message);
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

pub(super) async fn finish_project_swap(state: Signal<EditorShellState>, reset_selection: bool) {
    let _ = runtime::stop_program().await;
    refresh_runtime_bridge_state(state);
    load_snapshot(state, reset_selection).await;
}
