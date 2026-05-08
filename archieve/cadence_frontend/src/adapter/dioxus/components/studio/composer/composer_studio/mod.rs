use dioxus::prelude::*;

use crate::adapter::dioxus::components::studio::composer::{
    PatternPreviewRail, TileDetailPanel, TileSelectionBanner,
};
use crate::adapter::dioxus::components::studio::palette::TilePalette;
use crate::application::editor::EditorShellState;
use crate::infrastructure::ui::EditorCanvasHost;

#[component]
pub(crate) fn ComposerStudio(state: Signal<EditorShellState>, snapshot: EditorShellState) -> Element {
    let board_column_class = if snapshot.inspector.open {
        "editor-board is-inspector-open"
    } else {
        "editor-board"
    };
    let board_column_style = if snapshot.inspector.open {
        "grid-template-columns: minmax(0, 1fr) minmax(280px, 360px);"
    } else {
        "grid-template-columns: minmax(0, 1fr) 0px;"
    };

    rsx! {
        section {
            id: "grid-window",
            class: "grid-window",
            section {
                class: "editor-stage editor-shell",
                div {
                    class: "{board_column_class}",
                    style: "{board_column_style}",
                    section {
                        class: "canvas-workspace",
                        TileSelectionBanner {
                            state,
                            snapshot: snapshot.clone(),
                        }
                        PatternPreviewRail {
                            state,
                            snapshot: snapshot.clone(),
                        }
                        EditorCanvasHost {
                            state,
                            snapshot: snapshot.clone(),
                        }
                    }
                    TileDetailPanel {
                        state,
                        snapshot: snapshot.clone(),
                    }
                }
                TilePalette {
                    state,
                    snapshot,
                }
            }
        }
    }
}
