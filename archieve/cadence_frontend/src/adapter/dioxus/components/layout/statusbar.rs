use dioxus::prelude::*;

use crate::application::editor::*;

use crate::adapter::dioxus::theme as shell_theme;

#[component]
pub(crate) fn StatusBarRegion(snapshot: EditorShellState) -> Element {
    let runtime_boot_label = format_runtime_boot_label(&snapshot.runtime_boot);
    let runtime_playback_label = format_runtime_playback_label(&snapshot);
    let data_mode_label = backend_mode_label(snapshot.backend_available);
    let data_mode_detail = backend_mode_detail(snapshot.backend_available);
    let backend_mode_class = backend_mode_state_class(snapshot.backend_available);
    let status_source_class = shell_theme::status_source(backend_mode_class);
    let status_project = if snapshot.project.dirty {
        "Unsaved changes".to_string()
    } else {
        "All changes saved".to_string()
    };
    let status_selection = selected_piece_def(&snapshot)
        .map(|piece| {
            format!(
                "Tile: {}",
                compact_piece_label(Some(piece), selected_node_view(&snapshot))
            )
        })
        .unwrap_or_else(|| "Tile: none".to_string());

    rsx! {
        footer {
            class: shell_theme::STATUSBAR,
            span {
                id: "status-source",
                class: status_source_class,
                title: data_mode_detail,
                "{data_mode_label}"
            }
            span { id: "status-host", "{runtime_boot_label}" }
            span { id: "status-playback", "{runtime_playback_label}" }
            span { id: "status-selection", "{status_selection}" }
            span { id: "status-project", "{status_project}" }
        }
    }
}
