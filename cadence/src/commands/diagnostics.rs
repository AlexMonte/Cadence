//! Tauri commands for the mini-console feed and debug-UI visibility flags.

use serde::{Deserialize, Serialize};

use crate::commands::project_commands::SharedAppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// UI-facing serialization of one backend diagnostic entry.
pub struct DiagnosticEntryDto {
    pub at_ms: u64,
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Snapshot of the diagnostic feed plus debug panel visibility.
pub struct DiagnosticsSnapshotDto {
    pub entries: Vec<DiagnosticEntryDto>,
    pub mini_console_visible: bool,
    pub devtools_visible: bool,
}

#[derive(Debug, Clone, Deserialize)]
/// Visibility toggle for the mini console.
pub struct MiniConsoleVisibilityArgs {
    pub visible: bool,
}

#[derive(Debug, Clone, Deserialize)]
/// Visibility toggle for the devtools/inspector window.
pub struct DevtoolsVisibilityArgs {
    pub visible: bool,
}

#[tauri::command]
/// Return the current diagnostics feed and related UI flags.
pub fn diagnostics_snapshot(
    state: tauri::State<'_, SharedAppState>,
) -> Result<DiagnosticsSnapshotDto, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;

    Ok(DiagnosticsSnapshotDto {
        entries: store
            .diagnostics
            .iter()
            .map(|entry| DiagnosticEntryDto {
                at_ms: entry.at_ms,
                kind: entry.kind.clone(),
                message: entry.message.clone(),
            })
            .collect(),
        mini_console_visible: store.mini_console_visible,
        devtools_visible: store.devtools_visible,
    })
}

#[tauri::command]
/// Persist mini-console visibility in backend state.
pub fn ui_set_mini_console_visible(
    state: tauri::State<'_, SharedAppState>,
    args: MiniConsoleVisibilityArgs,
) -> Result<(), String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    store.mini_console_visible = args.visible;
    store.push_diagnostic("ui_mini_console", format!("visible={}", args.visible));
    Ok(())
}

#[tauri::command]
/// Persist devtools visibility in backend state.
pub fn ui_set_devtools_visible(
    state: tauri::State<'_, SharedAppState>,
    args: DevtoolsVisibilityArgs,
) -> Result<(), String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "app state lock poisoned".to_string())?;
    store.devtools_visible = args.visible;
    store.push_diagnostic("ui_devtools", format!("visible={}", args.visible));
    Ok(())
}
