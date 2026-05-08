use dioxus::prelude::*;

use crate::adapter::dioxus::components::palette::command_palette_overlay;
use crate::adapter::dioxus::components::layout::modal::unsaved::UnsavedModal;
use crate::adapter::dioxus::editor_service::{
    confirm_unsaved_discard, confirm_unsaved_save, discard_recovery_snapshot,
    restore_recovery_snapshot,
};
use crate::application::editor::*;

use crate::adapter::dioxus::theme as shell_theme;

#[component]
pub(crate) fn OverlayRegion(state: Signal<EditorShellState>, snapshot: EditorShellState) -> Element {
    rsx! {
        {command_palette_overlay(state, &snapshot)}

        if let Some(message) = snapshot.status_message.clone() {
            section {
                id: "status-toast",
                class: shell_theme::STATUS_TOAST,
                "aria-live": "polite",
                strong { "Cadence" }
                p { "{message}" }
                button {
                    r#type: "button",
                    onclick: move |_| {
                        state.write().status_message = None;
                    },
                    "Dismiss"
                }
            }
        }

        UnsavedModal {
            is_visible: snapshot.pending_project_action.is_some() || snapshot.recovery_open,
            title: if snapshot.recovery_open {
                "Recovery Snapshot Found".to_string()
            } else {
                "Unsaved Changes".to_string()
            },
            message: if snapshot.recovery_open {
                snapshot
                    .recovery_path
                    .clone()
                    .map(|path| format!("Cadence found a recovery snapshot at {path}. Restore it before continuing?"))
                    .unwrap_or_else(|| "Cadence found a recovery snapshot. Restore it before continuing?".to_string())
            } else {
                "You have unsaved changes. Save before continuing?".to_string()
            },
            save_label: if snapshot.recovery_open {
                "Restore".to_string()
            } else {
                "Save".to_string()
            },
            discard_label: if snapshot.recovery_open {
                "Discard Recovery".to_string()
            } else {
                "Discard".to_string()
            },
            cancel_label: "Cancel".to_string(),
            on_save: move |_| {
                let state = state;
                spawn(async move {
                    if state.read().recovery_open {
                        restore_recovery_snapshot(state).await;
                    } else {
                        confirm_unsaved_save(state).await;
                    }
                });
            },
            on_discard: move |_| {
                let state = state;
                spawn(async move {
                    if state.read().recovery_open {
                        discard_recovery_snapshot(state).await;
                    } else {
                        confirm_unsaved_discard(state).await;
                    }
                });
            },
            on_cancel: move |_| {
                let mut shell = state.write();
                shell.pending_project_action = None;
                shell.recovery_open = false;
            },
        }
    }
}
