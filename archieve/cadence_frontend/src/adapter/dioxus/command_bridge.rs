//! Dioxus-triggered intent should cross into the application layer here.

pub use super::editor_service::{
    dispatch_editor_action, dispatch_editor_command, load_snapshot, refresh_runtime_telemetry,
};
