use dioxus::prelude::*;

use crate::application::editor::{
    EditorShellState, compact_piece_label, piece_category_class, selected_node_view,
    selected_piece_def,
};

#[component]
pub(crate) fn TileSelectionBanner(
    mut state: Signal<EditorShellState>,
    snapshot: EditorShellState,
) -> Element {
    let selected_piece = selected_piece_def(&snapshot);
    let selected_tile = selected_node_view(&snapshot);
    let selected_count = snapshot.selected_nodes.len();
    let multi_selected = selected_count > 1;
    let selection_category_class = selected_piece
        .map(|piece| piece_category_class(piece.category.as_str()))
        .unwrap_or("piece-neutral");
    let selection_banner_class = format!(
        "selection-banner-card pixel-material pixel-panel pixel-panel--wood {selection_category_class}"
    );
    let selection_label = if multi_selected {
        format!("{selected_count} tiles selected")
    } else {
        selected_piece
            .map(|piece| compact_piece_label(Some(piece), selected_tile))
            .unwrap_or_else(|| "No tile selected".to_string())
    };
    let selection_subtitle = if snapshot.loading {
        "Refreshing the editor…".to_string()
    } else if multi_selected {
        "Group actions stay enabled here. Collapse back to one tile when you want per-tile wiring and port edits.".to_string()
    } else if let Some(piece) = selected_piece {
        piece
            .description
            .clone()
            .unwrap_or_else(|| "Selected tile ready to edit.".to_string())
    } else {
        "Choose a tile from the library, then click it on the canvas to shape it here.".to_string()
    };
    let selection_meta = if multi_selected {
        selected_tile.map(|tile| {
            format!(
                "Primary tile [{}, {}] • {} total",
                tile.position.col, tile.position.row, selected_count
            )
        })
    } else {
        selected_tile.map(|tile| format!("Cell [{}, {}]", tile.position.col, tile.position.row))
    };

    rsx! {
        div {
            class: "{selection_banner_class}",
            div {
                class: "selection-banner-main",
                button {
                    id: "selected-piece-name",
                    class: "selection-banner",
                    r#type: "button",
                    onclick: move |_| {
                        state.write().inspector.open = true;
                    },
                    "{selection_label}"
                }
                p {
                    class: "selection-banner-copy",
                    "{selection_subtitle}"
                }
            }
            if let Some(selection_meta) = selection_meta {
                span {
                    class: "selection-banner-meta",
                    "{selection_meta}"
                }
            }
        }
    }
}
