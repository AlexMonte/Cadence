//! Panel projection logic — derives inspector views from canonical editor state.

use crate::application::editor::workspace::{EditorAttention, FocusTarget, WorkspaceMode};
use crate::domain::board::BoardSurfaceId;

/// Default drawer visibility from focus (when the user has not toggled override).
///
/// Only placement targets on the active surface (empty slot / stack insert)
/// open the drawer by default. Tile/atom/port focus defaults it closed — the
/// user is inspecting, not placing. A user override of `Some(true)` keeps the
/// drawer open across focus changes (manage a tile's ports, then keep placing).
pub fn default_drawer_open_for_focus(focus: &FocusTarget, active_surface: BoardSurfaceId) -> bool {
    match focus {
        FocusTarget::EmptySlot { surface, .. } | FocusTarget::StackInsert { surface, .. } => {
            *surface == active_surface
        }
        FocusTarget::Tile { .. }
        | FocusTarget::Atom { .. }
        | FocusTarget::Port { .. }
        | FocusTarget::None
        | FocusTarget::TimelineEvent { .. } => false,
    }
}

/// Effective drawer open: user override when set, otherwise focus-derived default.
pub fn effective_drawer_open(override_open: Option<bool>, attention: &EditorAttention) -> bool {
    override_open.unwrap_or_else(|| {
        if attention.workspace_mode != WorkspaceMode::Compose {
            return false;
        }
        default_drawer_open_for_focus(&attention.focus, attention.active_board())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::editor::workspace::{ActiveSpace, EditorAttention};
    use crate::domain::board::BoardSlot;

    #[test]
    fn empty_slot_on_active_surface_defaults_open() {
        let surface = BoardSurfaceId(1);
        let attention = EditorAttention {
            workspace_mode: WorkspaceMode::Compose,
            active_space: ActiveSpace::Board(surface),
            focus: FocusTarget::EmptySlot {
                surface,
                slot: BoardSlot::new(0, 0),
            },
        };
        assert!(effective_drawer_open(None, &attention));
    }

    #[test]
    fn override_wins_over_focus_default() {
        let surface = BoardSurfaceId(1);
        let attention = EditorAttention {
            workspace_mode: WorkspaceMode::Compose,
            active_space: ActiveSpace::Board(surface),
            focus: FocusTarget::EmptySlot {
                surface,
                slot: BoardSlot::new(0, 0),
            },
        };
        assert!(!effective_drawer_open(Some(false), &attention));
    }

    #[test]
    fn tile_focus_defaults_drawer_closed() {
        use tessera::prelude::NodeId;

        let surface = BoardSurfaceId(1);
        let attention = EditorAttention {
            workspace_mode: WorkspaceMode::Compose,
            active_space: ActiveSpace::Board(surface),
            focus: FocusTarget::Tile {
                node: NodeId::new("tile"),
            },
        };
        assert!(!effective_drawer_open(None, &attention));
    }

    #[test]
    fn user_override_keeps_drawer_open_on_tile_focus() {
        use tessera::prelude::NodeId;

        let surface = BoardSurfaceId(1);
        let attention = EditorAttention {
            workspace_mode: WorkspaceMode::Compose,
            active_space: ActiveSpace::Board(surface),
            focus: FocusTarget::Tile {
                node: NodeId::new("tile"),
            },
        };
        assert!(effective_drawer_open(Some(true), &attention));
    }
}
