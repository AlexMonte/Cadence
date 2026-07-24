//! Pure cursor interaction policy (icon mapping, pick blocking).

use bevy::window::SystemCursorIcon;

use super::session::EditorSession;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorInteractionPhase {
    #[default]
    Idle,
    Panning,
    HoveringTile,
    Dragging,
}

pub fn cursor_icon_for(phase: CursorInteractionPhase, pan_dragging: bool) -> SystemCursorIcon {
    match phase {
        CursorInteractionPhase::Idle => SystemCursorIcon::Default,
        CursorInteractionPhase::Panning => {
            if pan_dragging {
                SystemCursorIcon::Grabbing
            } else {
                SystemCursorIcon::Grab
            }
        }
        CursorInteractionPhase::HoveringTile => SystemCursorIcon::Pointer,
        CursorInteractionPhase::Dragging => SystemCursorIcon::Grabbing,
    }
}

pub fn should_block_board_pick(cursor: CursorInteractionPhase, session: &EditorSession) -> bool {
    session.blocks_board_picks()
        || matches!(
            cursor,
            CursorInteractionPhase::Panning | CursorInteractionPhase::Dragging
        )
}

pub fn panning_active(viewport_pan: bool, middle_orbit: bool) -> bool {
    viewport_pan || middle_orbit
}

pub fn pan_dragging(viewport_pan: bool, cursor_moved: bool) -> bool {
    viewport_pan && cursor_moved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::TileSpawnKind;

    #[test]
    fn cursor_icon_maps_phases() {
        assert_eq!(
            cursor_icon_for(CursorInteractionPhase::Idle, false),
            SystemCursorIcon::Default
        );
        assert_eq!(
            cursor_icon_for(CursorInteractionPhase::HoveringTile, false),
            SystemCursorIcon::Pointer
        );
        assert_eq!(
            cursor_icon_for(CursorInteractionPhase::Dragging, false),
            SystemCursorIcon::Grabbing
        );
        assert_eq!(
            cursor_icon_for(CursorInteractionPhase::Panning, false),
            SystemCursorIcon::Grab
        );
        assert_eq!(
            cursor_icon_for(CursorInteractionPhase::Panning, true),
            SystemCursorIcon::Grabbing
        );
    }

    #[test]
    fn panning_and_dragging_block_board_picks() {
        let session = EditorSession::default();
        assert!(should_block_board_pick(
            CursorInteractionPhase::Panning,
            &session
        ));
        assert!(should_block_board_pick(
            CursorInteractionPhase::Dragging,
            &session
        ));
        assert!(!should_block_board_pick(
            CursorInteractionPhase::HoveringTile,
            &session
        ));
    }

    #[test]
    fn viewport_pan_active_blocks_via_panning_phase() {
        assert!(panning_active(true, false));
        assert!(!panning_active(false, false));
    }

    #[test]
    fn placement_session_blocks_board_picks() {
        let mut session = EditorSession::default();
        session
            .begin_placing(TileSpawnKind::Output {
                name: "main".into(),
            })
            .unwrap();
        assert!(should_block_board_pick(
            CursorInteractionPhase::Idle,
            &session
        ));
    }
}
