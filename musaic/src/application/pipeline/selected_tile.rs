//! Small, authored tile preview shared by the inspector header and contents grid.

use std::collections::{BTreeMap, BTreeSet};

use tessera::prelude::NodeId;

use super::scene_sync::{TileSurfaceContent, surface_content::surface_content_for_node};
use crate::domain::document::{AtomValue, DocumentNodeKind, DocumentQueries, PlacementAddress};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTilePaint {
    pub coordinate: String,
    pub glyph: String,
    pub pitched: bool,
    pub content: TileSurfaceContent,
    pub flow: Option<(
        tessera::prelude::FlowControlNode,
        tessera::prelude::NodeSpatialBindings,
    )>,
}

impl Default for SelectedTilePaint {
    fn default() -> Self {
        Self {
            coordinate: String::new(),
            glyph: String::new(),
            pitched: false,
            content: TileSurfaceContent::Empty,
            flow: None,
        }
    }
}

pub fn selected_tile_paint(queries: &DocumentQueries<'_>, node: &NodeId) -> SelectedTilePaint {
    let content = surface_content_for_node(queries, node, &BTreeMap::new(), &BTreeSet::new());
    let glyph = match &content {
        TileSurfaceContent::Scalar { display } | TileSurfaceContent::Compound { display, .. } => {
            display.clone()
        }
        TileSurfaceContent::Transform { label, .. } => match label.as_str() {
            "Fast" => "×".into(),
            "Slow" => "÷".into(),
            "Gain" => "g".into(),
            _ => label.chars().take(3).collect(),
        },
        TileSurfaceContent::Container { kind, .. } => container_glyph(*kind).into(),
        TileSurfaceContent::Wire { .. } => "→".into(),
        TileSurfaceContent::Empty => match queries.node_kind(node) {
            Some(DocumentNodeKind::Output(_)) => "→".into(),
            _ => "·".into(),
        },
    };
    let coordinate = queries
        .location_of(node)
        .map(|location| match location.address {
            PlacementAddress::BoardSlot(slot) => grid_coordinate(slot.x, slot.y),
            PlacementAddress::StackIndex(index) => format!("Tile {}", index.0 + 1),
        })
        .unwrap_or_default();
    let pitched = matches!(queries.node_kind(node), Some(DocumentNodeKind::Atom(atom)) if matches!(atom.atom, AtomValue::NoteName(_) | AtomValue::Octave(_) | AtomValue::Accidental(_)));
    let flow = if let Some(DocumentNodeKind::FlowControl(control)) = queries.node_kind(node) {
        Some((
            control.clone(),
            queries
                .document
                .connections
                .bindings
                .get(node)
                .cloned()
                .unwrap_or_else(|| {
                    tessera::prelude::default_spatial_bindings(
                        &tessera::prelude::RootSurfaceNodeKind::FlowControl(control.clone()),
                    )
                }),
        ))
    } else {
        None
    };
    SelectedTilePaint {
        flow,
        coordinate,
        glyph,
        pitched,
        content,
    }
}

pub fn container_glyph(kind: crate::domain::document::ContainerKind) -> &'static str {
    use crate::domain::document::ContainerKind;
    match kind {
        ContainerKind::Sequence => "[ ]",
        ContainerKind::Arrangement => "Song",
        ContainerKind::Subdivision => "[·]",
        ContainerKind::Alternating => "< >",
        ContainerKind::Parallel => "||",
    }
}

pub(crate) fn grid_coordinate(x: i32, y: i32) -> String {
    format!("{}{}", grid_column(x), grid_row(y))
}

/// Extend lettered columns to the left of A: -A, -B, ... (display only).
pub(crate) fn grid_column(x: i32) -> String {
    let mut column = if x < 0 {
        x.unsigned_abs()
    } else {
        x as u32 + 1
    };
    let mut letters = Vec::new();
    while column > 0 {
        letters.push((b'A' + ((column - 1) % 26) as u8) as char);
        column = (column - 1) / 26;
    }
    let label = letters.into_iter().rev().collect::<String>();
    if x < 0 { format!("−{label}") } else { label }
}

pub(crate) fn grid_row(y: i32) -> String {
    (i64::from(y) + 1).to_string()
}

#[cfg(test)]
mod coordinate_tests {
    use super::*;
    #[test]
    fn rulers_and_inspector_share_signed_coordinates_without_overflow() {
        for (x, y, expected) in [
            (0, 0, "A1"),
            (25, 8, "Z9"),
            (26, 9, "AA10"),
            (-1, 0, "−A1"),
            (-27, -1, "−AA0"),
            (0, -2, "A-1"),
        ] {
            assert_eq!(grid_coordinate(x, y), expected);
            assert_eq!(
                grid_coordinate(x, y),
                format!("{}{}", grid_column(x), grid_row(y))
            );
        }
        assert!(!grid_column(i32::MIN).is_empty());
        assert_eq!(grid_row(i32::MAX), "2147483648");
    }
}
