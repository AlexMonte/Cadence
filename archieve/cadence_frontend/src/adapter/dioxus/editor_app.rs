use dioxus::prelude::*;

use crate::adapter::backend::runtime as backend_runtime;
use crate::adapter::dioxus::editor_service::{load_snapshot, refresh_runtime_telemetry};
use crate::adapter::dioxus::runtime::process_menu_actions;
use crate::adapter::runtime;
use crate::application::editor::EditorShellState;
use crate::infrastructure::ui::layout::EditorFrame;

#[component]
pub fn EditorApp() -> Element {
    let state = use_signal(EditorShellState::default);
    let mut booted = use_signal(|| false);

    use_effect(move || {
        if *booted.read() {
            return;
        }
        booted.set(true);
        spawn(async move {
            load_snapshot(state, true).await;
        });
        spawn(async move {
            let _ = backend_runtime::install_menu_bridge();
            loop {
                process_menu_actions(state).await;
                refresh_runtime_telemetry(state).await;
                if runtime::sleep_ms(500).await.is_err() {
                    break;
                }
            }
        });
    });

    rsx! {
        EditorFrame { state }
    }
}
