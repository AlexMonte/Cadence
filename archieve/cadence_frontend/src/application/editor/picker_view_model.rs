use super::*;
use crate::adapter::PieceDef;
use crate::domain::GraphNodeView;

pub(crate) fn filtered_catalog(state: &EditorShellState) -> Vec<PieceDef> {
    let query = state.picker_query.trim().to_ascii_lowercase();
    picker_catalog(state)
        .into_iter()
        .filter(|piece| {
            if let Some(active_category) = state.picker_category.as_deref()
                && picker_category_for_piece(piece) != active_category
            {
                return false;
            }
            if query.is_empty() {
                return true;
            }
            piece.id.to_ascii_lowercase().contains(&query)
                || piece.label.to_ascii_lowercase().contains(&query)
                || picker_category_for_piece(piece)
                    .to_ascii_lowercase()
                    .contains(&query)
                || piece
                    .tags
                    .iter()
                    .any(|tag| tag.to_ascii_lowercase().contains(&query))
                || piece
                    .description
                    .as_ref()
                    .map(|value| value.to_ascii_lowercase().contains(&query))
                    .unwrap_or(false)
        })
        .collect()
}

pub(crate) fn filtered_atom_catalog(state: &EditorShellState) -> Vec<PieceDef> {
    let query = state.picker_query.trim().to_ascii_lowercase();
    atom_catalog(state)
        .into_iter()
        .filter(|piece| {
            if query.is_empty() {
                return true;
            }
            piece.id.to_ascii_lowercase().contains(&query)
                || piece.label.to_ascii_lowercase().contains(&query)
                || picker_category_for_piece(piece)
                    .to_ascii_lowercase()
                    .contains(&query)
                || piece
                    .tags
                    .iter()
                    .any(|tag| tag.to_ascii_lowercase().contains(&query))
                || piece
                    .description
                    .as_ref()
                    .map(|value| value.to_ascii_lowercase().contains(&query))
                    .unwrap_or(false)
        })
        .collect()
}

pub(crate) fn picker_catalog(state: &EditorShellState) -> Vec<PieceDef> {
    state
        .catalog
        .iter()
        .filter(|piece| !is_deprecated_catalog_piece(piece) && !is_atom_catalog_piece(piece))
        .cloned()
        .collect()
}

pub(crate) fn atom_catalog(state: &EditorShellState) -> Vec<PieceDef> {
    state
        .catalog
        .iter()
        .filter(|piece| !is_deprecated_catalog_piece(piece) && is_atom_catalog_piece(piece))
        .cloned()
        .collect()
}

pub(crate) fn picker_category_tabs(catalog: &[PieceDef]) -> Vec<(&'static str, &'static str)> {
    let mut tabs = vec![("all", "All")];
    for (category_id, category_label) in [
        ("generator", "Generators"),
        ("transform", "Transforms"),
        ("control", "Controls"),
        ("output", "Outputs"),
        ("connector", "Connectors"),
        ("constant", "Constants"),
        ("trick", "Patterns"),
        ("container", "Containers"),
    ] {
        if catalog
            .iter()
            .filter(|piece| !is_deprecated_catalog_piece(piece) && !is_atom_catalog_piece(piece))
            .any(|piece| picker_category_for_piece(piece) == category_id)
        {
            tabs.push((category_id, category_label));
        }
    }
    tabs
}

pub(crate) fn is_deprecated_catalog_piece(piece: &PieceDef) -> bool {
    matches!(
        piece.id.as_str(),
        "cadence.number" | "cadence.text" | "cadence.pattern_notation" | "cadence.pattern_input"
    )
}

pub(crate) fn is_atom_catalog_piece(piece: &PieceDef) -> bool {
    piece.id.starts_with("cadence.atom.")
}

pub(crate) fn is_container_catalog_piece(piece: &PieceDef) -> bool {
    piece.id.starts_with("cadence.container.")
}

pub(crate) fn picker_category_for_piece(piece: &PieceDef) -> &str {
    if is_container_catalog_piece(piece) {
        "container"
    } else {
        piece.category.as_str()
    }
}

pub(crate) fn compact_piece_label(
    piece: Option<&PieceDef>,
    node: Option<&GraphNodeView>,
) -> String {
    match (piece, node) {
        (Some(piece), Some(node)) => node
            .label
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| piece.label.clone()),
        (Some(piece), None) => piece.label.clone(),
        (None, Some(node)) => node.piece_id.clone(),
        (None, None) => "None".to_string(),
    }
}

pub(crate) fn piece_category_class(category: &str) -> &'static str {
    match category {
        "container" => "piece-container",
        "generator" => "piece-generator",
        "transform" => "piece-transform",
        "constant" => "piece-constant",
        "output" => "piece-output",
        "control" => "piece-control",
        "trick" => "piece-trick",
        "connector" => "piece-connector",
        "atom" => "piece-atom",
        _ => "piece-generator",
    }
}
