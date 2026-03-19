use super::*;

pub(super) async fn process_menu_actions(mut state: Signal<EditorShellState>) {
    let actions = runtime::take_menu_actions().unwrap_or_default();
    for action in actions {
        match action.as_str() {
            "file.new" => request_project_action(state, PendingProjectAction::NewProject).await,
            "file.open" => request_project_action(state, PendingProjectAction::OpenProject).await,
            "file.quit" => request_project_action(state, PendingProjectAction::QuitApp).await,
            "file.save" => {
                let _ = save_project(state, false).await;
            }
            "file.save_as" => {
                let _ = save_project(state, true).await;
            }
            "file.export_song" => export_song(state).await,
            "edit.undo" => undo_history(state).await,
            "edit.redo" => redo_history(state).await,
            "window.close_requested" => {
                request_project_action(state, PendingProjectAction::CloseWindow).await;
            }
            "view.toggle_mini_console" => {
                let next = !state.read().diagnostics.mini_console_visible;
                set_mini_console_visible(state, next).await;
            }
            "view.toggle_dev_inspector" => {
                let next = !state.read().diagnostics.devtools_visible;
                if let Err(error) = tauri::ui_set_devtools_visible(next).await {
                    state.write().status_message = Some(error);
                } else {
                    state.write().diagnostics.devtools_visible = next;
                }
            }
            _ => {}
        }
    }
}

pub(super) async fn refresh_runtime_telemetry(mut state: Signal<EditorShellState>) {
    refresh_runtime_bridge_state(state);
    if !state.read().tauri_available {
        return;
    }
    if let Ok(runtime_status) = tauri::runtime_status().await {
        state.write().runtime_status = runtime_status;
    }
    if let Ok(history_status) = tauri::history_status().await {
        state.write().history_status = history_status;
    }
    if let Ok(diagnostics) = tauri::diagnostics_snapshot().await {
        state.write().diagnostics = diagnostics;
    }
}

pub(super) async fn export_song(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        state.write().status_message =
            Some("Export Song requires the desktop backend.".to_string());
        return;
    }

    let path = match tauri::export_pick_song_path().await {
        Ok(Some(path)) => path,
        Ok(None) => return,
        Err(error) => {
            state.write().status_message = Some(error);
            return;
        }
    };

    let cpm = state.read().cpm_input.trim().parse::<f32>().ok();
    match tauri::export_song(path, cpm, None, Some(tauri::TerminalStrategy::Stack)).await {
        Ok(result) => {
            {
                let mut current = state.write();
                current.status_message = Some(result.message.clone());
                if !result.exported {
                    current.compile_open = true;
                }
            }
            refresh_runtime_telemetry(state).await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

pub(super) fn refresh_runtime_bridge_state(mut state: Signal<EditorShellState>) {
    if let Ok(status) = runtime::runtime_boot_status() {
        state.write().runtime_boot = status;
    }
    if let Ok(status) = runtime::runtime_sample_readiness() {
        state.write().sample_readiness = status;
    }
    if let Ok(status) = runtime::runtime_sample_cache_status() {
        state.write().sample_cache = status;
    }
    if let Ok(status) = runtime::runtime_init_sample_status() {
        state.write().init_sample_status = status;
    }
}

pub(super) async fn play_runtime(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        state.write().status_message =
            Some("Runtime playback requires the desktop backend.".to_string());
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
    let commit = match tauri::runtime_commit(RuntimeCommitArgs {
        cpm,
        force: Some(false),
        playing: Some(true),
        code_override: None,
        terminal_strategy: Some(tauri::TerminalStrategy::Stack),
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

    state.write().runtime_status = RuntimeStatusDto {
        rev: commit.rev,
        playing: commit.playing,
        has_program: commit.runtime_code.is_some(),
        last_error: commit.error.clone(),
        play_elapsed_ms: commit.play_elapsed_ms,
    };

    if !commit.success {
        state.write().status_message = Some(
            commit
                .error
                .unwrap_or_else(|| "Runtime compile failed.".to_string()),
        );
        return;
    }

    if let Err(error) = runtime::run_cadence_program(&runtime::RuntimeProgramPayload {
        cps_expr: commit.cps_expr.clone(),
        sample_loads: commit.sample_loads.clone(),
        declaration_code: commit.declaration_code.clone(),
        runtime_code: commit.runtime_code.clone(),
    })
    .await
    {
        state.write().status_message = Some(error);
        let _ = tauri::runtime_stop()
            .await
            .map(|status| state.write().runtime_status = status);
        refresh_runtime_bridge_state(state);
        return;
    }

    state.write().status_message = Some(format!(
        "Runtime ready. Voices: {} | rev {}",
        commit.voice_count, commit.rev
    ));
    refresh_runtime_bridge_state(state);
}

pub(super) async fn stop_runtime(mut state: Signal<EditorShellState>) {
    let _ = runtime::stop_program().await;
    if state.read().tauri_available {
        match tauri::runtime_stop().await {
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

pub(super) async fn undo_history(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        return;
    }
    match tauri::history_undo().await {
        Ok(status) => {
            state.write().history_status = status;
            load_snapshot(state, false).await;
            schedule_recovery_write(state);
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

pub(super) async fn redo_history(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        return;
    }
    match tauri::history_redo().await {
        Ok(status) => {
            state.write().history_status = status;
            load_snapshot(state, false).await;
            schedule_recovery_write(state);
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

pub(super) async fn set_mini_console_visible(mut state: Signal<EditorShellState>, visible: bool) {
    if state.read().tauri_available
        && let Err(error) = tauri::ui_set_mini_console_visible(visible).await
    {
        state.write().status_message = Some(error);
        return;
    }
    state.write().diagnostics.mini_console_visible = visible;
    if !visible {
        state.write().status_message = None;
    }
}

pub(super) fn schedule_recovery_write(mut state: Signal<EditorShellState>) {
    if !state.read().tauri_available {
        return;
    }
    let generation = {
        let mut current = state.write();
        current.recovery_generation = current.recovery_generation.saturating_add(1);
        current.recovery_generation
    };
    spawn(async move {
        if runtime::sleep_ms(800).await.is_err() {
            return;
        }
        if state.read().recovery_generation != generation {
            return;
        }
        match tauri::project_recovery_write().await {
            Ok(message) => {
                let mut current = state.write();
                current.recovery_path = parse_recovery_path(message.as_str());
            }
            Err(error) => {
                state.write().status_message = Some(error);
            }
        }
    });
}
