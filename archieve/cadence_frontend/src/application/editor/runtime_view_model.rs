use crate::adapter::{RuntimeBootPhase, RuntimeBootStatus, RuntimeProgramStateDto};

use super::{BACKEND_REQUIRED_MESSAGE, EditorShellState};

pub(crate) fn activity_severity_pill_class(severity: &str) -> &'static str {
    match severity {
        "error" => "activity-pill is-error",
        "warning" => "activity-pill is-warning",
        "info" => "activity-pill is-info",
        _ => "activity-pill",
    }
}

pub(crate) fn backend_mode_label(backend_available: bool) -> &'static str {
    if backend_available {
        "LIVE BACKEND"
    } else {
        "BACKEND REQUIRED"
    }
}

pub(crate) fn backend_mode_detail(backend_available: bool) -> &'static str {
    if backend_available {
        "Connected to live project data and playback controls from the Cadence backend."
    } else {
        BACKEND_REQUIRED_MESSAGE
    }
}

pub(crate) fn backend_mode_state_class(backend_available: bool) -> &'static str {
    if backend_available {
        "is-live"
    } else {
        "is-unavailable"
    }
}

pub(crate) fn format_runtime_boot_label(status: &RuntimeBootStatus) -> String {
    match status.phase {
        RuntimeBootPhase::Idle => "Audio idle".to_string(),
        RuntimeBootPhase::Booting => "Connecting audio".to_string(),
        RuntimeBootPhase::Ready => "Audio ready".to_string(),
        RuntimeBootPhase::Error => format!(
            "Audio issue: {}",
            status
                .detail
                .clone()
                .unwrap_or_else(|| "runtime".to_string())
        ),
    }
}

pub(crate) fn format_runtime_playback_label(state: &EditorShellState) -> String {
    match (
        state.runtime_status.playing,
        &state.runtime_status.program_state,
        state.runtime_status.last_error.as_ref(),
    ) {
        (true, RuntimeProgramStateDto::StaleLastGood, Some(error)) => {
            format!("Playing last good render: {error}")
        }
        (true, _, _) => "Playing".to_string(),
        (_, RuntimeProgramStateDto::StaleLastGood, Some(error)) => {
            format!("Last good render loaded: {error}")
        }
        (_, RuntimeProgramStateDto::StaleLastGood, None) => {
            "Last good render loaded".to_string()
        }
        (_, RuntimeProgramStateDto::Current, Some(error)) => {
            format!("Playback issue: {error}")
        }
        (_, RuntimeProgramStateDto::Current, None) => "Ready to play".to_string(),
        (_, RuntimeProgramStateDto::None, Some(error)) => {
            format!("Playback issue: {error}")
        }
        (_, RuntimeProgramStateDto::None, None) => "Idle".to_string(),
    }
}

pub(crate) fn render_console_output(state: &EditorShellState) -> String {
    let mut lines = Vec::new();
    if let Some(message) = state.status_message.as_ref() {
        lines.push(format!("Latest update: {message}"));
    }
    for diagnostic in &state.diagnostics.entries {
        lines.push(format!("[{}] {}", diagnostic.kind, diagnostic.message));
    }
    if lines.is_empty() {
        "Console idle.".to_string()
    } else {
        lines.join("\n")
    }
}
