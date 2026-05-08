use dioxus::prelude::*;

use crate::adapter::dioxus::components::palette::{
    clear_interaction_state, insert_piece_into_selected_container,
};
use crate::adapter::dioxus::editor_service::{invalidate_drag_preview, place_piece};
use crate::application::editor::{
    ContextualLibraryMode, DragSession, EditorShellState, InteractionState, PointerPoint,
    compact_piece_label, filtered_atom_catalog, filtered_catalog, is_container_catalog_piece,
    picker_category_for_piece, picker_category_tabs, piece_category_class, piece_def_for_id,
    selected_node_view,
};

#[component]
pub(crate) fn TilePalette(state: Signal<EditorShellState>, snapshot: EditorShellState) -> Element {
    let picker_available = snapshot.backend_available && snapshot.workspace_mode.picker_allowed();
    let contextual_library_open = picker_available && snapshot.contextual_library_open;
    let show_atom_palette = contextual_library_open
        && matches!(snapshot.contextual_library_mode, ContextualLibraryMode::Atoms);
    let show_tile_palette = contextual_library_open && !show_atom_palette;
    let palette_panel_style = if contextual_library_open {
        let anchor = snapshot
            .contextual_library_anchor
            .unwrap_or(PointerPoint { x: 96.0, y: 120.0 });
        let left = anchor.x.max(16.0);
        let top = anchor.y.max(72.0);
        format!("--contextual-left: {left:.0}px; --contextual-top: {top:.0}px;")
    } else {
        "display: none;".to_string()
    };
    let filtered_catalog = filtered_catalog(&snapshot);
    let picker_tabs = picker_category_tabs(&snapshot.catalog);
    let picker_search_key = format!("picker-search-{}", snapshot.picker_focus_nonce);
    let selected_tile = selected_node_view(&snapshot);
    let selected_container_label = selected_tile
        .and_then(|tile| {
            piece_def_for_id(&snapshot.catalog, &tile.piece_id).map(|piece| (tile, piece))
        })
        .filter(|(_tile, piece)| is_container_catalog_piece(piece))
        .map(|(tile, piece)| compact_piece_label(Some(piece), Some(tile)));
    let atom_catalog = filtered_atom_catalog(&snapshot);
    let atom_palette_items = atom_catalog
        .iter()
        .map(|piece| {
            let piece_id = piece.id.clone();
            let atom_label = piece
                .label
                .strip_suffix(" atom")
                .or_else(|| piece.label.strip_suffix(" op"))
                .unwrap_or(piece.label.as_str())
                .to_string();
            let click_piece_id = piece_id.clone();
            let html_drag_piece_id = piece_id.clone();
            let insertion_hint = selected_container_label
                .as_ref()
                .map(|label| format!("Insert into {label}"))
                .unwrap_or_else(|| "Select a container to insert".to_string());
            rsx! {
                button {
                    key: "atom-{piece.id}",
                    class: "grid-piece-picker-item atom-drawer-item piece-constant",
                    "data-piece-id": "{piece.id}",
                    r#type: "button",
                    draggable: "true",
                    title: "{insertion_hint}",
                    onclick: move |_| {
                        {
                            let mut shell = state.write();
                            clear_interaction_state(&mut shell);
                            shell.contextual_library_open = true;
                            shell.contextual_library_mode = ContextualLibraryMode::Atoms;
                        }
                        let piece_id = click_piece_id.clone();
                        spawn(async move {
                            insert_piece_into_selected_container(state, piece_id).await;
                        });
                    },
                    ondragstart: move |_| {
                        let mut shell = state.write();
                        shell.interaction_state =
                            Some(InteractionState::Dragging(DragSession::PickerPiece {
                                piece_id: html_drag_piece_id.clone(),
                            }));
                        shell.drag_hover = None;
                        invalidate_drag_preview(&mut shell);
                        shell.status_message = Some(
                            "Atoms are container-local. Click to insert into the selected container tile.".to_string(),
                        );
                    },
                    ondragend: move |_| {
                        let mut shell = state.write();
                        clear_interaction_state(&mut shell);
                    },
                    div {
                        class: "tile-card-head",
                        span { class: "library-item-title", "{atom_label}" }
                        span { class: "tile-card-kind", "atom" }
                    }
                    p {
                        class: "library-item-description",
                        "{insertion_hint}"
                    }
                    if let Some(description) = piece.description.as_deref() {
                        p {
                            class: "library-item-description",
                            "{description}"
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    let tile_palette_items = filtered_catalog
        .iter()
        .filter(|piece| !piece.id.starts_with("cadence.atom."))
        .map(|piece| {
            let piece_id = piece.id.clone();
            let tile_kind_label = if is_container_catalog_piece(piece) {
                "container"
            } else {
                piece.category.as_str()
            };
            let tile_glyph = if is_container_catalog_piece(piece) {
                "slot"
            } else if piece.id == "args_connector" {
                "args"
            } else if piece.category == "output" {
                "out"
            } else if piece.category == "transform" {
                "fx"
            } else {
                "tile"
            };
            let insertion_hint = if is_container_catalog_piece(piece) {
                selected_container_label
                    .as_ref()
                    .map(|label| format!("Insert into {label}"))
                    .unwrap_or_else(|| "Place on graph or select a container to insert".to_string())
            } else {
                "Place on graph".to_string()
            };
            let button_class = format!(
                "grid-piece-picker-item tile-drawer-card {}",
                piece_category_class(picker_category_for_piece(piece))
            );
            let click_piece_id = piece_id.clone();
            let html_drag_piece_id = piece_id.clone();
            let insert_into_selected_container =
                is_container_catalog_piece(piece) && selected_container_label.is_some();
            rsx! {
                button {
                    key: "{piece.id}",
                    class: "{button_class}",
                    "data-piece-id": "{piece.id}",
                    r#type: "button",
                    draggable: "true",
                    title: "{insertion_hint}",
                    onclick: move |_| {
                        {
                            let mut shell = state.write();
                            clear_interaction_state(&mut shell);
                            shell.contextual_library_open = true;
                            shell.contextual_library_mode = if insert_into_selected_container {
                                ContextualLibraryMode::Atoms
                            } else {
                                ContextualLibraryMode::Tiles
                            };
                        }
                        let piece_id = click_piece_id.clone();
                        spawn(async move {
                            if insert_into_selected_container {
                                insert_piece_into_selected_container(state, piece_id).await;
                            } else {
                                let target = state.read().picker_target;
                                place_piece(state, piece_id, target).await;
                            }
                        });
                    },
                    ondragstart: move |_| {
                        let mut shell = state.write();
                        shell.interaction_state =
                            Some(InteractionState::Dragging(DragSession::PickerPiece {
                                piece_id: html_drag_piece_id.clone(),
                            }));
                        shell.drag_hover = None;
                        invalidate_drag_preview(&mut shell);
                        if insert_into_selected_container {
                            shell.status_message = Some(
                                "Containers can be placed on the graph or inserted into the selected container tile.".to_string(),
                            );
                        }
                    },
                    ondragend: move |_| {
                        let mut shell = state.write();
                        clear_interaction_state(&mut shell);
                    },
                    div {
                        class: "tile-card-head",
                        span { class: "library-item-glyph", "{tile_glyph}" }
                        span { class: "library-item-title", "{piece.label}" }
                        span { class: "tile-card-kind", "{tile_kind_label}" }
                    }
                    div {
                        class: "library-item-meta",
                        span { "{piece.namespace}" }
                        span { "{piece.id}" }
                    }
                    p {
                        class: "library-item-description",
                        "{insertion_hint}"
                    }
                    if let Some(description) = piece.description.as_deref() {
                        p {
                            class: "library-item-description",
                            "{description}"
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    rsx! {
        aside {
            id: "grid-piece-picker",
            class: if contextual_library_open {
                "grid-piece-picker contextual-library-floating pixel-material pixel-panel pixel-panel--parchment is-open"
            } else {
                "grid-piece-picker contextual-library-floating pixel-material pixel-panel pixel-panel--parchment"
            },
            style: "{palette_panel_style}",
            "aria-label": if show_atom_palette {
                "Atom library"
            } else {
                "Tile library"
            },
            header {
                class: "grid-piece-picker-head",
                div {
                    class: "panel-head-copy",
                    strong {
                        if show_atom_palette {
                            "Atom Library"
                        } else {
                            "Tile Library"
                        }
                    }
                    span {
                        class: "panel-head-subtitle",
                        if show_atom_palette {
                            if let Some(container_label) = selected_container_label.clone() {
                                "Inserting into {container_label}"
                            } else {
                                "Select a container tile to insert atoms."
                            }
                        } else {
                            "Right-click the canvas to place tiles. Select a container to switch to atoms."
                        }
                    }
                }
                button {
                    r#type: "button",
                    class: "panel-toggle-button",
                    onclick: move |_| {
                        let mut shell = state.write();
                        shell.contextual_library_open = false;
                    },
                    if contextual_library_open { "Hide" } else { "Library" }
                }
            }
            if show_tile_palette {
                div {
                    class: "picker-category-tabs compact-drawer-tabs",
                    for (category_id, category_label) in picker_tabs {
                        button {
                            key: "picker-category-{category_id}",
                            class: if snapshot.picker_category.as_deref() == Some(category_id) {
                                "picker-category-tab is-active"
                            } else if category_id == "all" && snapshot.picker_category.is_none() {
                                "picker-category-tab is-active"
                            } else {
                                "picker-category-tab"
                            },
                            r#type: "button",
                            onclick: move |_| {
                                let mut shell = state.write();
                                shell.picker_category = if category_id == "all" {
                                    None
                                } else {
                                    Some(category_id.to_string())
                                };
                            },
                            "{category_label}"
                        }
                    }
                }
            }
            input {
                key: "{picker_search_key}",
                id: "grid-piece-picker-search",
                class: "grid-piece-picker-search compact-drawer-search",
                r#type: "search",
                autofocus: "true",
                value: snapshot.picker_query.clone(),
                placeholder: if show_atom_palette {
                    "Search atoms by name or tag"
                } else {
                    "Search tiles by name, category, or tag"
                },
                "aria-label": if show_atom_palette {
                    "Search atoms"
                } else {
                    "Search tiles"
                },
                oninput: move |event| {
                    state.write().picker_query = event.value();
                }
            }
            div {
                id: "grid-piece-picker-list",
                class: if show_atom_palette {
                    "grid-piece-picker-list atom-drawer-list"
                } else {
                    "grid-piece-picker-list tile-drawer-list"
                },
                if show_atom_palette {
                    if atom_palette_items.is_empty() {
                        p {
                            class: "library-empty",
                            "No atoms match the current search."
                        }
                    } else {
                        {atom_palette_items.into_iter()}
                    }
                } else if tile_palette_items.is_empty() {
                    p {
                        class: "library-empty",
                        "No tiles match the current search."
                    }
                } else {
                    {tile_palette_items.into_iter()}
                }
            }
        }
    }
}
