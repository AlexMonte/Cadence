//! Thin app lifecycle commands used by the frontend-owned confirmation flows.

use tauri::{AppHandle, Manager};

use crate::commands::project_commands::SharedAppState;

const MAIN_WINDOW_LABEL: &str = "main";

pub fn take_main_window_close_allowance(state: &SharedAppState) -> bool {
    match state.allow_main_window_close.lock() {
        Ok(mut allow_close) => {
            let allowed = *allow_close;
            *allow_close = false;
            allowed
        }
        Err(_) => false,
    }
}

#[tauri::command]
pub fn app_quit(app: AppHandle) -> Result<(), String> {
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn window_close_main(
    app: AppHandle,
    state: tauri::State<'_, SharedAppState>,
) -> Result<(), String> {
    {
        let mut allow_close = state
            .allow_main_window_close
            .lock()
            .map_err(|_| "close gate lock poisoned".to_string())?;
        *allow_close = true;
    }

    let window = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .ok_or_else(|| "main window unavailable".to_string())?;
    window.close().map_err(|err| err.to_string())
}
