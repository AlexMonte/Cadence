use tessera::prelude::NodeId;

use super::types::RenderBoardFocus;
use crate::{
    application::editor::{EditorAttention, workspace::FocusTarget},
    domain::board::BoardSurfaceId,
    domain::document::queries::DocumentQueries,
};

/// Resolves the node receiving tile-level focus from canonical editor attention.
pub fn focused_node_from_attention(attention: &EditorAttention) -> Option<NodeId> {
    match &attention.focus {
        FocusTarget::Tile { node }
        | FocusTarget::Atom { node }
        | FocusTarget::Port { node, .. } => Some(node.clone()),
        _ => None,
    }
}

/// Maps canonical editor focus to a render focus on the active surface.
pub fn render_focus_from_attention(
    queries: &DocumentQueries<'_>,
    attention: &EditorAttention,
    active_surface: BoardSurfaceId,
) -> Option<RenderBoardFocus> {
    match &attention.focus {
        FocusTarget::EmptySlot { surface, slot } if *surface == active_surface => {
            Some(RenderBoardFocus::BoardSlot(*slot))
        }
        FocusTarget::StackInsert { surface, index } if *surface == active_surface => {
            Some(RenderBoardFocus::StackInsert(*index))
        }
        FocusTarget::Tile { node }
        | FocusTarget::Atom { node }
        | FocusTarget::Port { node, .. } => queries
            .authored_tile(node)
            .and_then(|tile| tile.placement)
            .filter(|placement| placement.surface == active_surface)
            .map(|placement| RenderBoardFocus::Tile {
                node: node.clone(),
                address: placement.address,
            }),
        _ => None,
    }
}

/// Whether the visible board projection must be rebuilt this frame.
pub fn needs_visible_board_rebuild(
    scene_dirty: bool,
    attention_changed: bool,
    selection_changed: bool,
    visible_active_surface: Option<BoardSurfaceId>,
    attention_active_board: BoardSurfaceId,
) -> bool {
    scene_dirty
        || attention_changed
        || selection_changed
        || visible_active_surface != Some(attention_active_board)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::editor::EditorAttention,
        domain::board::BoardSurfaceId,
        domain::document::{PortId, StackIndex},
    };

    #[test]
    fn focused_node_from_tile_atom_and_port_focus() {
        let mut attention = EditorAttention::new(BoardSurfaceId(0));
        let node = NodeId::new("tile_a");

        // Reader-only: assign focus field directly (validation is tested elsewhere).
        attention.focus = FocusTarget::Tile { node: node.clone() };
        assert_eq!(focused_node_from_attention(&attention), Some(node.clone()));

        attention.focus = FocusTarget::Atom { node: node.clone() };
        assert_eq!(focused_node_from_attention(&attention), Some(node.clone()));

        attention.focus = FocusTarget::Port {
            node: node.clone(),
            port: PortId(0),
        };
        assert_eq!(focused_node_from_attention(&attention), Some(node));
    }

    #[test]
    fn focused_node_none_for_non_tile_focus() {
        let mut attention = EditorAttention::new(BoardSurfaceId(0));
        attention.focus = FocusTarget::EmptySlot {
            surface: BoardSurfaceId(0),
            slot: crate::domain::board::BoardSlot::new(0, 0),
        };
        assert_eq!(focused_node_from_attention(&attention), None);

        attention.focus = FocusTarget::StackInsert {
            surface: BoardSurfaceId(0),
            index: StackIndex(0),
        };
        assert_eq!(focused_node_from_attention(&attention), None);

        attention.focus = FocusTarget::None;
        assert_eq!(focused_node_from_attention(&attention), None);
    }

    #[test]
    fn needs_rebuild_on_selection_or_attention_change_without_scene_dirty() {
        assert!(needs_visible_board_rebuild(
            false,
            true,
            false,
            Some(BoardSurfaceId(0)),
            BoardSurfaceId(0),
        ));
        assert!(needs_visible_board_rebuild(
            false,
            false,
            true,
            Some(BoardSurfaceId(0)),
            BoardSurfaceId(0),
        ));
    }

    #[test]
    fn skips_rebuild_when_clean_and_unchanged() {
        assert!(!needs_visible_board_rebuild(
            false,
            false,
            false,
            Some(BoardSurfaceId(0)),
            BoardSurfaceId(0),
        ));
    }

    #[test]
    fn needs_rebuild_on_active_surface_mismatch() {
        assert!(needs_visible_board_rebuild(
            false,
            false,
            false,
            Some(BoardSurfaceId(0)),
            BoardSurfaceId(1),
        ));
    }
}
