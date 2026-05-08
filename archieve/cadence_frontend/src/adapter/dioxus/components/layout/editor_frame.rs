use dioxus::prelude::*;

use crate::adapter::dioxus::components::palette::handle_editor_keydown;
use crate::application::editor::EditorShellState;

use crate::adapter::dioxus::components::layout::{
    ActivityDrawerRegion, OverlayRegion, StatusBarRegion, TopbarRegion,
};
use crate::adapter::dioxus::components::studio::StudioRegion;
use crate::adapter::dioxus::theme as shell_theme;

#[component]
pub(crate) fn EditorFrame(state: Signal<EditorShellState>) -> Element {
    let snapshot = state.read().clone();
    let keydown_snapshot = snapshot.clone();
    rsx! {
        main {
            class: shell_theme::APP_FRAME,
            tabindex: "0",
            onkeydown: move |event| {
                handle_editor_keydown(event, state, &keydown_snapshot);
            },

            TopbarRegion { state, snapshot: snapshot.clone() }

            if !snapshot.backend_available {
                div {
                    id: "backend-banner",
                    class: shell_theme::BACKEND_BANNER,
                    "Offline mode: reopen Cadence through the desktop app to edit, save, and preview the song."
                }
            }

            StudioRegion { state, snapshot: snapshot.clone() }
            OverlayRegion { state, snapshot: snapshot.clone() }
            ActivityDrawerRegion { state, snapshot: snapshot.clone() }

            StatusBarRegion { snapshot: snapshot.clone() }
        }
    }
}
