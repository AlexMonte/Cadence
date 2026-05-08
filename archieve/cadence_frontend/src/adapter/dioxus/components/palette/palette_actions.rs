use crate::adapter::GridPos;
use crate::adapter::backend::{
    GraphOp, PatternAtom, PatternContainer, PatternContainerKind, PatternItem, PatternItemKind,
    PatternPos, PatternSurface,
};
use crate::adapter::dioxus::editor_service::invalidate_drag_preview;
use crate::adapter::dioxus::editor_service::apply_graph_ops;
use crate::application::editor::EditorShellState;
use crate::application::editor::piece_def_for_id;
use crate::domain::{first_free_position, is_cell_occupied};
use dioxus::prelude::*;

pub(crate) fn toggle_picker_panel(state: &mut EditorShellState) {
    if state.contextual_library_open {
        state.contextual_library_open = false;
        state.picker_open = false;
        state.picker_target = None;
        state.contextual_library_anchor = None;
        state.selected_container_insertion_index = None;
        return;
    }

    open_picker(state, None, false);

    if state.selected_node.is_some() {
        state.contextual_library_mode = crate::application::editor::ContextualLibraryMode::Atoms;
    } else {
        state.contextual_library_mode = crate::application::editor::ContextualLibraryMode::Tiles;
    }
}

pub(crate) fn open_picker(
    state: &mut EditorShellState,
    target: Option<GridPos>,
    reset_filters: bool,
) {
    state.picker_open = true;
    state.contextual_library_open = true;
    state.contextual_library_mode = crate::application::editor::ContextualLibraryMode::Tiles;
    state.command_palette_open = false;
    state.command_palette_query.clear();
    state.command_palette_selected = 0;
    if reset_filters {
        state.picker_query.clear();
        state.picker_category = None;
    }
    state.picker_focus_nonce = state.picker_focus_nonce.wrapping_add(1);
    state.picker_target = target.or_else(|| {
        state
            .selected_cell
            .filter(|pos| !is_cell_occupied(&state.graph, pos))
            .or_else(|| first_free_position(&state.graph))
    });
    state.contextual_library_anchor = None;
    if let Some(target) = state.picker_target {
        state.selected_cell = Some(target);
        state.selected_node = None;
        state.selected_nodes.clear();
        state.inspector.label_input.clear();
        state.inspector.param_inputs.clear();
        state.selected_container_insertion_index = None;
    }
}

pub(crate) async fn insert_piece_into_selected_container(
    state: Signal<EditorShellState>,
    piece_id: String,
) {
    let insertion_index = state.read().selected_container_insertion_index;
    insert_piece_into_selected_container_at(state, piece_id, insertion_index).await;
}

pub(crate) async fn insert_piece_into_selected_container_at(
    mut state: Signal<EditorShellState>,
    piece_id: String,
    insertion_index: Option<usize>,
) {
    let insertion = {
        let snapshot = state.read();
        match snapshot.selected_node.or(snapshot.selected_cell) {
            None => None,
            Some(position) => match snapshot
                .graph
                .nodes
                .iter()
                .find(|node| node.position == position)
            {
                None => None,
                Some(node) => match piece_def_for_id(&snapshot.catalog, node.piece_id.as_str()) {
                    None => None,
                    Some(piece) if !piece.id.starts_with("cadence.container.") => None,
                    Some(_) => {
                        let next_surface = insert_piece_at_surface_index(
                            node.pattern_source.clone(),
                            piece_id.as_str(),
                            insertion_index,
                        );
                        Some((position, next_surface))
                    }
                },
            },
        }
    };

    let Some((position, next_surface)) = insertion else {
        let message = {
            let snapshot = state.read();
            let insertable_kind = if piece_id.starts_with("cadence.atom.") {
                "Atoms"
            } else {
                "Containers"
            };
            match snapshot.selected_node.or(snapshot.selected_cell) {
                None => format!("Select a container tile before inserting {insertable_kind}."),
                Some(position) => match snapshot
                    .graph
                    .nodes
                    .iter()
                    .find(|node| node.position == position)
                {
                    None => "Selected container tile is missing.".to_string(),
                    Some(node) => match piece_def_for_id(&snapshot.catalog, node.piece_id.as_str())
                    {
                        None => "Selected container tile is unknown.".to_string(),
                        Some(piece) if !piece.id.starts_with("cadence.container.") => {
                            format!(
                                "{insertable_kind} can only be inserted into a selected container tile."
                            )
                        }
                        Some(_) => format!(
                            "Unable to insert the selected {}.",
                            if piece_id.starts_with("cadence.atom.") {
                                "atom"
                            } else {
                                "container"
                            }
                        ),
                    },
                },
            }
        };
        state.write().status_message = Some(message);
        return;
    };

    {
        let mut snapshot = state.write();
        snapshot.selected_container_insertion_index = None;
    }

    apply_graph_ops(
        state,
        "native_insert_container_piece",
        vec![GraphOp::NodeSetPatternSurface {
            position,
            pattern_source: Some(next_surface),
        }],
        Some(position),
    )
    .await;
}

pub(crate) fn clear_drag_state(state: &mut EditorShellState) {
    state.interaction_state = None;
    state.drag_hover = None;
    invalidate_drag_preview(state);
}

pub(crate) fn clear_interaction_state(state: &mut EditorShellState) {
    clear_drag_state(state);
}

pub(crate) fn close_picker(state: &mut EditorShellState) {
    state.picker_open = false;
    state.picker_target = None;
    state.contextual_library_open = false;
    state.contextual_library_anchor = None;
    state.selected_container_insertion_index = None;
}

fn insert_piece_at_surface_index(
    surface: Option<PatternSurface>,
    piece_id: &str,
    insertion_index: Option<usize>,
) -> PatternSurface {
    let mut surface = surface.unwrap_or_else(default_basic_surface);
    if surface.roots.is_empty() {
        return default_basic_surface_with_item(piece_id);
    }

    if let Some(root) = surface.roots.first_mut() {
        let next_item = PatternItem {
            position: PatternPos { col: 0, row: 0 },
            kind: item_kind_for_piece(piece_id),
        };
        let insert_at = insertion_index
            .unwrap_or(root.container.items.len())
            .min(root.container.items.len());
        root.container.items.insert(insert_at, next_item);
        reindex_container_items(&mut root.container);
    }

    surface
}

fn reindex_container_items(container: &mut PatternContainer) {
    for (index, item) in container.items.iter_mut().enumerate() {
        item.position = PatternPos {
            col: index as i32,
            row: 0,
        };
    }
}

fn default_basic_surface() -> PatternSurface {
    PatternSurface {
        roots: vec![crate::adapter::backend::PatternRoot {
            position: PatternPos { col: 0, row: 0 },
            container: PatternContainer {
                kind: PatternContainerKind::Basic,
                items: Vec::new(),
            },
        }],
    }
}

fn default_basic_surface_with_item(piece_id: &str) -> PatternSurface {
    let mut surface = default_basic_surface();
    if let Some(root) = surface.roots.first_mut() {
        root.container.items.push(PatternItem {
            position: PatternPos { col: 0, row: 0 },
            kind: item_kind_for_piece(piece_id),
        });
    }
    surface
}

fn item_kind_for_piece(piece_id: &str) -> PatternItemKind {
    if piece_id.starts_with("cadence.container.") {
        PatternItemKind::Container(Box::new(container_for_piece(piece_id)))
    } else {
        PatternItemKind::Atom(atom_for_piece(piece_id))
    }
}

fn container_for_piece(piece_id: &str) -> PatternContainer {
    let kind = match piece_id {
        "cadence.container.subdivide" => PatternContainerKind::Subdivide,
        "cadence.container.alternate" => PatternContainerKind::Alternate,
        "cadence.container.parallel" => PatternContainerKind::Parallel,
        _ => PatternContainerKind::Basic,
    };
    PatternContainer {
        kind,
        items: Vec::new(),
    }
}

fn atom_for_piece(piece_id: &str) -> PatternAtom {
    match piece_id {
        "cadence.atom.scalar" => PatternAtom::Scalar { value: 0.0 },
        "cadence.atom.rest" => PatternAtom::Rest,
        "cadence.atom.operator.elongation" => PatternAtom::Operator {
            operator: crate::adapter::backend::PatternOperatorKind::Elongation,
        },
        "cadence.atom.operator.pitch_shift" => PatternAtom::Operator {
            operator: crate::adapter::backend::PatternOperatorKind::PitchShift,
        },
        "cadence.atom.operator.slow" => PatternAtom::Operator {
            operator: crate::adapter::backend::PatternOperatorKind::Slow,
        },
        "cadence.atom.operator.fast" => PatternAtom::Operator {
            operator: crate::adapter::backend::PatternOperatorKind::Fast,
        },
        _ => PatternAtom::Note {
            value: "a".to_string(),
        },
    }
}
