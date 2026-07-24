//! Board placement ghost position — shared between drag preview and slot highlight.

use bevy::prelude::*;
use tessera::prelude::TileFootprint;

use crate::{
    application::editor::{BoardPlacementPointer, EditorSession, PlacementHover},
    application::pipeline::scene_sync::VisibleNodeKind,
    domain::{
        board::{
            SurfaceLayoutKind,
            geometry::{BOARD_PLANE_Y, SLOT_SIZE, slot_at_world_position},
        },
        document::{PlacementAddress, RootBoardTileKind, StackIndex, root_board_tile_footprint},
    },
};

use super::board_geometry::{stack_flat_tile_center, tessera_slot_center};
use super::musaic_tile::{
    board_footprint_for_kind, stack_footprint_for_kind, tile_center_y_for_footprint,
};

const TILE_INSET: f32 = 0.18;

/// Resolve world-space center for the placement ghost.
///
/// Priority: active hover → cursor slot snap → cursor on board plane → last hover.
pub fn resolve_placement_preview_center(
    session: &EditorSession,
    pointer: &BoardPlacementPointer,
    visible: &crate::application::pipeline::scene_sync::VisibleBoardState,
    preview_kind: VisibleNodeKind,
) -> Option<Vec3> {
    let placement = match &session.mode {
        crate::application::editor::EditorMode::Placing(p) => p,
        _ => return None,
    };
    let on_root_board = visible.layout == SurfaceLayoutKind::Board;
    let visual_footprint = if visible.layout == SurfaceLayoutKind::Stack {
        stack_footprint_for_kind(preview_kind)
    } else {
        board_footprint_for_kind(preview_kind)
    };
    let tessera_footprint = tessera_footprint_for_preview(preview_kind, on_root_board);

    if let Some(hover) = placement.hover {
        return center_for_hover(
            hover,
            visible.layout,
            preview_kind,
            visual_footprint,
            tessera_footprint,
        );
    }

    if let Some(world) = pointer.cursor_world {
        if visible.layout == SurfaceLayoutKind::Stack {
            if let Some(slot) = slot_at_world_position(world, visible.layout) {
                return Some(stack_flat_tile_center(
                    StackIndex(slot.x as usize),
                    preview_kind,
                    BOARD_PLANE_Y,
                ));
            }
        } else if let Some(slot) = slot_at_world_position(world, visible.layout) {
            // The root board is unbounded: every world position resolves to a
            // slot, so the ghost always previews the true landing slot.
            let y = tile_center_y_for_footprint(visual_footprint, BOARD_PLANE_Y);
            return Some(tessera_slot_center(slot, tessera_footprint, y));
        }
    }

    placement.last_hover.and_then(|hover| {
        center_for_hover(
            hover,
            visible.layout,
            preview_kind,
            visual_footprint,
            tessera_footprint,
        )
    })
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
        let pointer = BoardPlacementPointer::default();

        let center = resolve_placement_preview_center(
            &session,
            &pointer,
            &visible,
            VisibleNodeKind::Container,
        )
        .expect("last hover should anchor preview");

        assert!(center.y > 0.0);
        assert!(center.x.is_finite() && center.z.is_finite());
    }

    #[test]
    fn cursor_far_from_origin_snaps_to_slot_on_unbounded_board() {
        let session = session_placing_sequence();
        let visible = crate::application::pipeline::scene_sync::VisibleBoardState {
            active_surface: Some(BoardSurfaceId(1)),
            layout: SurfaceLayoutKind::Board,
            ..Default::default()
        };
        let world = Vec3::new(99.0, 0.0, 99.0);
        let pointer = BoardPlacementPointer {
            cursor_world: Some(world),
            ..Default::default()
        };

        let center = resolve_placement_preview_center(
            &session,
            &pointer,
            &visible,
            VisibleNodeKind::Container,
        )
        .expect("the board is unbounded — any cursor position resolves to a slot");

        // The ghost snaps to the containing slot's tessera center, matching
        // where the tile would actually land.
        let slot = slot_at_world_position(world, SurfaceLayoutKind::Board)
            .expect("unbounded board resolves every world position");
        let expected = tessera_slot_center(
            slot,
            tessera_footprint_for_preview(VisibleNodeKind::Container, true),
            center.y,
        );
        assert_eq!(center.x, expected.x);
        assert_eq!(center.z, expected.z);
        assert!(center.y > 0.0);
    }
}
