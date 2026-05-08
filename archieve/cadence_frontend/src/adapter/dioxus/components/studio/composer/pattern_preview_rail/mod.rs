use dioxus::prelude::*;

use crate::application::editor::{
    ActivityTab, EditorShellState, compile_modal_view, tile_notation_cues,
};
use crate::infrastructure::ui::modal::compiled_modal::CompileTimeline;

#[component]
pub(crate) fn PatternPreviewRail(
    mut state: Signal<EditorShellState>,
    snapshot: EditorShellState,
) -> Element {
    let compile_view = compile_modal_view(&snapshot);
    let tile_notation_cues = tile_notation_cues(&snapshot);
    let preview_tile_count = tile_notation_cues.len();
    let preview_lane_count = compile_view.timeline_lanes.len();
    let has_preview_timeline = !compile_view.timeline_lanes.is_empty();
    let preview_summary = if !has_preview_timeline {
        "Preview the song to surface cadence notation across tiles and timing lanes.".to_string()
    } else {
        format!(
            "{} tile(s) • {} event(s) • {} lane(s) • {}",
            preview_tile_count,
            compile_view.preview_event_count,
            preview_lane_count,
            compile_view.playhead_label
        )
    };
    let show_preview_rail = has_preview_timeline || compile_view.banner_text.is_some();
    if !show_preview_rail {
        return rsx! {};
    }

    rsx! {
        section {
            class: "canvas-preview-rail pixel-material pixel-panel pixel-panel--parchment",
            header {
                class: "canvas-preview-rail-head",
                div {
                    class: "canvas-preview-rail-copy",
                    strong { "Cadence notation map" }
                    span {
                        class: "canvas-preview-rail-sub",
                        "{preview_summary}"
                    }
                }
                div {
                    class: "canvas-preview-rail-actions",
                    button {
                        r#type: "button",
                        class: "panel-toggle-button",
                        onclick: move |_| {
                            let mut current = state.write();
                            current.compile_open = true;
                            current.activity_tab = ActivityTab::Preview;
                        },
                        "Preview details"
                    }
                }
            }
            if let Some(banner_text) = compile_view.banner_text.clone() {
                p {
                    class: "compile-banner",
                    "{banner_text}"
                }
            }
            if has_preview_timeline {
                CompileTimeline {
                    lanes: compile_view.timeline_lanes.clone(),
                    playhead_pct: compile_view.playhead_pct,
                    playhead_label: compile_view.playhead_label.clone(),
                    compact: true,
                }
            }
        }
    }
}
