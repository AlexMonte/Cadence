//! Board placement ghost position — shared between drag preview and slot highlight.
//!
//! Center comes only from session hover geometry (`hover.or(last_hover)`). Cursor
//! world positions must not invent a slot here; that belongs to
//! `sync_placement_hover_from_pointer` → `VisibleBoardState::pick_at`.

use bevy::prelude::*;
use tessera::prelude::TileFootprint;

use crate::{
    application::editor::PlacementHover,
    application::pipeline::scene_sync::VisibleNodeKind,
    domain::{
        board::{
            SurfaceLayoutKind,
            geometry::{BOARD_PLANE_Y, SLOT_SIZE},
        },
        document::{PlacementAddress, RootBoardTileKind, root_board_tile_footprint},
    },
};

use super::board_geometry::{stack_flat_tile_center, tessera_slot_center};
use super::musaic_tile::{
    TILE_INSET, board_footprint_for_kind, stack_footprint_for_kind, tile_center_y_for_footprint,
};

/// Resolve world-space center for the placement ghost.
///
/// Priority: active hover → last hover. No cursor→slot invent path.
pub fn resolve_placement_preview_center(
    session: &crate::application::editor::EditorSession,
    visible: &crate::application::pipeline::scene_sync::VisibleBoardState,
    preview_kind: VisibleNodeKind,
) -> Option<Vec3> {
    if !matches!(
        session.mode,
        crate::application::editor::EditorMode::Placing(_)
    ) {
        return None;
    }

    let hover = session.placement_hover()?;
    let on_root_board = visible.layout == SurfaceLayoutKind::Board;
    let visual_footprint = if visible.layout == SurfaceLayoutKind::Stack {
        stack_footprint_for_kind(preview_kind)
    } else {
        board_footprint_for_kind(preview_kind)
    };
    let tessera_footprint = tessera_footprint_for_preview(preview_kind, on_root_board);

    center_for_hover(
        hover,
        visible.layout,
        preview_kind,
        visual_footprint,
        tessera_footprint,
    )
}

pub fn tessera_footprint_for_preview(kind: VisibleNodeKind, on_root_board: bool) -> TileFootprint {
    if on_root_board {
        root_board_tile_footprint(RootBoardTileKind::from(kind))
    } else {
        TileFootprint::unit()
    }
}

fn center_for_hover(
    hover: PlacementHover,
    layout: SurfaceLayoutKind,
    preview_kind: VisibleNodeKind,
    visual_footprint: f32,
    tessera_footprint: TileFootprint,
) -> Option<Vec3> {
    match (layout, hover.address) {
        (SurfaceLayoutKind::Board, PlacementAddress::BoardSlot(slot)) => {
            let y = tile_center_y_for_footprint(visual_footprint, BOARD_PLANE_Y);
            Some(tessera_slot_center(slot, tessera_footprint, y))
        }
        (SurfaceLayoutKind::Stack, PlacementAddress::StackIndex(index)) => {
            Some(stack_flat_tile_center(index, preview_kind, BOARD_PLANE_Y))
        }
        (_, PlacementAddress::BoardSlot(slot)) => {
            let footprint = SLOT_SIZE - TILE_INSET;
            Some(tile_world_position(
                PlacementAddress::BoardSlot(slot),
                footprint,
            ))
        }
        (_, PlacementAddress::StackIndex(index)) => {
            Some(stack_flat_tile_center(index, preview_kind, BOARD_PLANE_Y))
        }
    }
}

fn tile_world_position(address: PlacementAddress, footprint: f32) -> Vec3 {
    let y = tile_center_y_for_footprint(footprint, BOARD_PLANE_Y);
    match address {
        PlacementAddress::BoardSlot(slot) => tessera_slot_center(slot, TileFootprint::unit(), y),
        PlacementAddress::StackIndex(index) => {
            stack_flat_tile_center(index, VisibleNodeKind::Atom, BOARD_PLANE_Y)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::editor::EditorSession;
    use crate::domain::{
        board::{BoardSlot, BoardSurfaceId},
        document::{ContainerKind, TileSpawnKind},
    };

    fn session_placing_sequence() -> EditorSession {
        let mut session = EditorSession::default();
        session
            .begin_placing(TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            })
            .unwrap();
        session
    }

    #[test]
    fn last_hover_persists_when_cursor_leaves_board() {
        let mut session = session_placing_sequence();
        let surface = BoardSurfaceId(1);
        let slot = BoardSlot::new(2, 3);
        if let Some(placement) = session.placement_mut() {
            placement.last_hover = Some(PlacementHover {
                surface,
                address: PlacementAddress::BoardSlot(slot),
            });
        }

        let visible = crate::application::pipeline::scene_sync::VisibleBoardState {
            active_surface: Some(surface),
            layout: SurfaceLayoutKind::Board,
            ..Default::default()
        };

        let center = resolve_placement_preview_center(
            &session,
            &visible,
            VisibleNodeKind::Container,
        )
        .expect("last hover should anchor preview");

        assert!(center.y > 0.0);
        assert!(center.x.is_finite() && center.z.is_finite());
    }

    #[test]
    fn ghost_center_none_when_hover_and_last_hover_none() {
        let session = session_placing_sequence();
        let visible = crate::application::pipeline::scene_sync::VisibleBoardState {
            active_surface: Some(BoardSurfaceId(1)),
            layout: SurfaceLayoutKind::Board,
            ..Default::default()
        };

        assert!(
            resolve_placement_preview_center(&session, &visible, VisibleNodeKind::Container)
                .is_none(),
            "ghost must not invent a slot from cursor when hover and last_hover are unset"
        );
    }

    #[test]
    fn active_hover_wins_over_last_hover() {
        let mut session = session_placing_sequence();
        let surface = BoardSurfaceId(1);
        if let Some(placement) = session.placement_mut() {
            placement.last_hover = Some(PlacementHover {
                surface,
                address: PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
            });
            placement.hover = Some(PlacementHover {
                surface,
                address: PlacementAddress::BoardSlot(BoardSlot::new(4, 1)),
            });
        }

        let visible = crate::application::pipeline::scene_sync::VisibleBoardState {
            active_surface: Some(surface),
            layout: SurfaceLayoutKind::Board,
            ..Default::default()
        };

        let center = resolve_placement_preview_center(
            &session,
            &visible,
            VisibleNodeKind::Container,
        )
        .expect("active hover");

        let expected = tessera_slot_center(
            BoardSlot::new(4, 1),
            tessera_footprint_for_preview(VisibleNodeKind::Container, true),
            center.y,
        );
        assert_eq!(center.x, expected.x);
        assert_eq!(center.z, expected.z);
    }
}
