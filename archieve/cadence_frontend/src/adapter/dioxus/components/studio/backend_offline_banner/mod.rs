use dioxus::prelude::*;

use crate::adapter::dioxus::editor_service::load_snapshot;
use crate::application::editor::EditorShellState;

#[component]
pub(crate) fn BackendOfflineBanner(state: Signal<EditorShellState>) -> Element {
    rsx! {
        section {
            id: "backend-unavailable",
            class: "backend-unavailable",
            strong { "Native backend unavailable" }
            p {
                "Open Cadence through the desktop app to load a project, edit tiles, and control playback."
            }
            button {
                id: "retry-backend-connect",
                r#type: "button",
                onclick: move |_| {
                    spawn(async move {
                        load_snapshot(state, true).await;
                    });
                },
                "Retry connection"
            }
        }
    }
}
