//! Shared board slot/stack world-space geometry for placed tiles and previews.

use bevy::prelude::*;
use tessera::prelude::TileFootprint;

use crate::{
    application::pipeline::scene_sync::VisibleNodeKind,
    domain::{
        board::{
            BoardSlot, board_height, board_width,
            geometry::{SLOT_SIZE, stack_column_center as domain_stack_column_center},
        },
        document::{PlacementAddress, StackIndex},
    },
};

use super::musaic_tile::{stack_footprint_for_kind, tile_center_y_for_footprint};

pub fn tessera_slot_center(slot: BoardSlot, footprint: TileFootprint, y: f32) -> Vec3 {
    Vec3::new(
        slot.x as f32 * SLOT_SIZE + footprint.width as f32 * SLOT_SIZE * 0.5 - board_width() * 0.5,
        y,
        slot.y as f32 * SLOT_SIZE + footprint.height as f32 * SLOT_SIZE * 0.5
            - board_height() * 0.5,
    )
}

pub fn stack_column_center(index: StackIndex) -> f32 {
    domain_stack_column_center(index.0)
}

pub fn stack_flat_tile_center(index: StackIndex, kind: VisibleNodeKind, ground_y: f32) -> Vec3 {
    let footprint = stack_footprint_for_kind(kind);
    let y = tile_center_y_for_footprint(footprint, ground_y);
    Vec3::new(stack_column_center(index), y, 0.0)
}

pub fn placement_center_for_address(
    address: PlacementAddress,
    tessera_footprint: TileFootprint,
    kind: VisibleNodeKind,
    visual_footprint: f32,
    ground_y: f32,
) -> Vec3 {
    let y = tile_center_y_for_footprint(visual_footprint, ground_y);
    match address {
        PlacementAddress::BoardSlot(slot) => tessera_slot_center(slot, tessera_footprint, y),
        PlacementAddress::StackIndex(index) => stack_flat_tile_center(index, kind, ground_y),
    }
}
