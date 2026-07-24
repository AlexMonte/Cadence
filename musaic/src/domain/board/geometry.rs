use bevy::prelude::*;

use super::{BoardSlot, SurfaceLayoutKind};

pub const VIEWPORT_COLUMNS: i32 = 32;
pub const VIEWPORT_ROWS: i32 = 32;
pub const SLOT_SIZE: f32 = 1.0;
pub const BOARD_PLANE_Y: f32 = 0.0;
pub const STACK_LENGTH: f32 = 8.0;
pub const STACK_DEPTH: f32 = 1.0;

/// World-space left edge of stack column zero.
pub fn stack_left_edge() -> f32 {
    -(STACK_LENGTH * SLOT_SIZE) * 0.5
}

pub fn stack_column_center(column: usize) -> f32 {
    stack_left_edge() + column as f32 * SLOT_SIZE + SLOT_SIZE * 0.5
}

pub fn slot_at_world_position(world: Vec3, layout: SurfaceLayoutKind) -> Option<BoardSlot> {
    match layout {
        SurfaceLayoutKind::Board => board_slot_at_world_position(world),
        SurfaceLayoutKind::Stack => stack_slot_at_world_position(world),
    }
}

/// The root board is unbounded (`BoardSlot` is `i32`); this maps any world
/// point on the board plane to its slot. `VIEWPORT_COLUMNS`/`VIEWPORT_ROWS`
/// only define the world origin offset and default framing, not placement
/// bounds.
pub fn board_slot_at_world_position(world: Vec3) -> Option<BoardSlot> {
    let x_origin = -((VIEWPORT_COLUMNS as f32 - 1.0) * SLOT_SIZE) * 0.5;
    let z_origin = -((VIEWPORT_ROWS as f32 - 1.0) * SLOT_SIZE) * 0.5;

    let col = ((world.x - x_origin) / SLOT_SIZE).round() as i32;
    let row = ((world.z - z_origin) / SLOT_SIZE).round() as i32;

    Some(BoardSlot::new(col, row))
}

pub fn stack_slot_at_world_position(world: Vec3) -> Option<BoardSlot> {
    let col = ((world.x - stack_left_edge()) / SLOT_SIZE).floor() as i32;
    let row = 0i32;

    if col < 0 || col as f32 >= STACK_LENGTH {
        return None;
    }

    Some(BoardSlot::new(col, row))
}

pub fn board_width() -> f32 {
    VIEWPORT_COLUMNS as f32 * SLOT_SIZE
}

pub fn board_height() -> f32 {
    VIEWPORT_ROWS as f32 * SLOT_SIZE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_column_centers_round_trip_at_both_boundaries() {
        for column in [0, STACK_LENGTH as usize - 1] {
            let slot = stack_slot_at_world_position(Vec3::new(
                stack_column_center(column),
                BOARD_PLANE_Y,
                0.0,
            ));
            assert_eq!(slot, Some(BoardSlot::new(column as i32, 0)));
        }
    }
}
