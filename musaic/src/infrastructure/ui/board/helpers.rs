use bevy::prelude::*;

use crate::application::pipeline::scene_sync::{VisibleBoardNode, VisibleNodeKind};
use crate::domain::board::{BoardSlot, SLOT_SIZE, STACK_DEPTH, STACK_LENGTH, SurfaceLayoutKind, geometry::BOARD_PLANE_Y};
use crate::domain::document::{PlacementAddress, StackIndex};
use crate::infrastructure::ui::board_geometry::{stack_column_center, stack_flat_tile_center, tessera_slot_center};
use crate::infrastructure::ui::musaic_tile::{TILE_INSET, tile_center_y_for_footprint};

pub(super) const SLOT_HEIGHT: f32 = BOARD_PLANE_Y;
pub(super) const STACK_CELL_WIDTH: f32 = SLOT_SIZE - TILE_INSET;
pub(super) const STACK_CELL_DEPTH: f32 = STACK_DEPTH - 0.36;
pub(super) const STACK_INSERT_WIDTH: f32 = 0.10;

pub(super) const CONNECTION_THICKNESS: f32 = 0.05;
pub(super) const CONNECTION_HEIGHT: f32 = 0.12;
pub(super) const CONNECTION_LIFT: f32 = SLOT_HEIGHT + 0.06;

pub(super) const BOARD_THICKNESS: f32 = 0.02;
pub(super) const STACK_MARGIN: f32 = 0.18;

pub(super) fn tessera_footprint_for_node(node: &VisibleBoardNode) -> tessera::prelude::TileFootprint {
    node.tessera_footprint
        .unwrap_or(tessera::prelude::TileFootprint::unit())
}

pub(super) fn slot_position(slot: BoardSlot, y: f32) -> Vec3 {
    tessera_slot_center(slot, tessera::prelude::TileFootprint::unit(), y)
}

pub(super) fn tile_world_position(address: PlacementAddress, footprint: f32) -> Vec3 {
    let y = tile_center_y_for_footprint(footprint, SLOT_HEIGHT);
    match address {
        PlacementAddress::BoardSlot(slot) => {
            tessera_slot_center(slot, tessera::prelude::TileFootprint::unit(), y)
        }
        PlacementAddress::StackIndex(index) => {
            stack_flat_tile_center(index, VisibleNodeKind::Atom, SLOT_HEIGHT)
        }
    }
}

pub(super) fn stack_insert_marker_center(index: StackIndex) -> Vec3 {
    let y = tile_center_y_for_footprint(STACK_CELL_WIDTH * 0.5, SLOT_HEIGHT);
    Vec3::new(stack_column_center(index), y, 0.0)
}

pub(super) fn hovered_slot_to_visual_slot(address: PlacementAddress) -> BoardSlot {
    match address {
        PlacementAddress::BoardSlot(slot) => slot,
        PlacementAddress::StackIndex(index) => BoardSlot::new(index.0 as i32, 0),
    }
}

pub(super) fn visual_slot_to_address(layout: SurfaceLayoutKind, slot: BoardSlot) -> PlacementAddress {
    match layout {
        SurfaceLayoutKind::Board => PlacementAddress::BoardSlot(slot),
        SurfaceLayoutKind::Stack => {
            PlacementAddress::StackIndex(StackIndex(slot.x.max(0) as usize))
        }
    }
}

pub(super) fn stack_inner_width() -> f32 {
    STACK_LENGTH * SLOT_SIZE
}

pub(super) fn stack_width() -> f32 {
    stack_inner_width() + STACK_MARGIN * 2.0
}

