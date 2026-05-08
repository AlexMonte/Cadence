use dioxus::prelude::*;

use crate::adapter::dioxus::components::studio::tile_detail_panel_content;
use crate::adapter::dioxus::editor_service::delete_selected_node;
use crate::application::editor::{
    EditorShellState, piece_category_class, selected_piece_def,
};

#[component]
pub(crate) fn TileDetailPanel(state: Signal<EditorShellState>, snapshot: EditorShellState) -> Element {
    let selected_piece = selected_piece_def(&snapshot);
    let selected_count = snapshot.selected_nodes.len();
    let multi_selected = selected_count > 1;
    let has_selected_tile = snapshot.selected_node.is_some();
    let selection_category_class = selected_piece
        .map(|piece| piece_category_class(piece.category.as_str()))
        .unwrap_or("piece-neutral");
    let selection_panel_class = if snapshot.inspector.open {
        format!(
            "side-config selection-panel pixel-material pixel-panel pixel-panel--stone {selection_category_class} is-open"
        )
    } else {
        format!(
            "side-config selection-panel pixel-material pixel-panel pixel-panel--stone {selection_category_class}"
        )
    };

    rsx! {
        aside {
            id: "side-config",
            class: "{selection_panel_class}",
            "aria-label": "Tile inspector",
            header {
                class: "side-config-head",
                div {
                    class: "panel-head-copy",
                    span { id: "side-config-title", "Selection" }
                    span {
                        class: "panel-head-subtitle",
                        if multi_selected {
                            "Review and manage the selected group"
                        } else if has_selected_tile {
                            "Edit the selected tile"
                        } else {
                            "Choose a tile to inspect"
                        }
                    }
                }
                div {
                    class: "panel-head-actions",
                    if has_selected_tile {
                        button {
                            id: "delete-node-inline",
                            r#type: "button",
                            onclick: move |_| {
                                spawn(async move {
                                    delete_selected_node(state).await;
                                });
                            },
                            if multi_selected {
                                "Delete Tiles"
                            } else {
                                "Delete Tile"
                            }
                        }
                    }
                    button {
                        id: "selection-close",
                        r#type: "button",
                        onclick: move |_| {
                            state.write().inspector.open = false;
                        },
                        "Hide"
                    }
                }
            }
            div {
                id: "side-config-body",
                class: "side-config-body",
                {tile_detail_panel_content(state, &snapshot)}
            }
        }
    }
}
