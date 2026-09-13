use bevy::prelude::*;

use super::{BoardSlot, SurfaceLayoutKind};

pub const VIEWPORT_COLUMNS: i32 = 32;
pub const VIEWPORT_ROWS: i32 = 32;
pub const SLOT_SIZE: f32 = 1.0;
pub const BOARD_PLANE_Y: f32 = 0.0;
/// Visual wrapping only; authored stack indices remain one ordered sequence.
pub const STACK_COLUMNS: usize = 12;
pub const STACK_LENGTH: f32 = STACK_COLUMNS as f32;
pub const STACK_DEPTH: f32 = 1.0;

/// World-space left edge of stack column zero.
pub fn stack_left_edge() -> f32 {
    -(STACK_LENGTH * SLOT_SIZE) * 0.5
}

pub fn stack_column_center(column: usize) -> f32 {
    stack_left_edge() + (column % STACK_COLUMNS) as f32 * SLOT_SIZE + SLOT_SIZE * 0.5
}

pub fn stack_row_center(index: usize) -> f32 {
    (index / STACK_COLUMNS) as f32 * SLOT_SIZE
}

pub fn stack_display_slot(index: usize) -> BoardSlot {
    BoardSlot::new(
        (index % STACK_COLUMNS) as i32,
        (index / STACK_COLUMNS) as i32,
    )
}

pub fn stack_index_at_display_slot(slot: BoardSlot) -> Option<usize> {
    if slot.x < 0 || slot.x >= STACK_COLUMNS as i32 || slot.y < 0 {
        return None;
    }
    (slot.y as usize)
        .checked_mul(STACK_COLUMNS)?
        .checked_add(slot.x as usize)
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
    let column = ((world.x - stack_left_edge()) / SLOT_SIZE).floor();
    let row = ((world.z + SLOT_SIZE * 0.5) / SLOT_SIZE).floor();
    if !column.is_finite()
        || !row.is_finite()
        || column < 0.0
        || column >= STACK_COLUMNS as f32
        || row < 0.0
        || row >= (i32::MAX as usize / STACK_COLUMNS) as f32
    {
        return None;
    }
    // Interaction uses a linear display address, matching VisibleBoardState.
    Some(BoardSlot::new(
        row as i32 * STACK_COLUMNS as i32 + column as i32,
        0,
    ))
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
    use proptest::prelude::*;

    #[test]
    fn stack_column_centers_round_trip_at_both_boundaries() {
        for column in [0, STACK_LENGTH as usize - 1, 8, 15, 63, 1023] {
            let slot = stack_slot_at_world_position(Vec3::new(
                stack_column_center(column),
                BOARD_PLANE_Y,
                stack_row_center(column),
            ));
            assert_eq!(slot, Some(BoardSlot::new(column as i32, 0)));
        }
    }

    proptest! {
        #[test]
        fn every_point_inside_a_wrapped_cell_picks_that_display_index(
            index in 0usize..10_000,
            dx in -0.45f32..0.45,
            dz in -0.45f32..0.45,
        ) {
            let world = Vec3::new(stack_column_center(index) + dx, 0.0, stack_row_center(index) + dz);
            prop_assert_eq!(stack_slot_at_world_position(world), Some(BoardSlot::new(index as i32, 0)));
            prop_assert_eq!(stack_index_at_display_slot(stack_display_slot(index)), Some(index));
        }
    }

    #[test]
    fn outside_container_cells_do_not_select_a_different_row() {
        for world in [
            Vec3::new(-6.01, 0.0, 0.0),
            Vec3::new(6.01, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -0.51),
            Vec3::new(f32::NAN, 0.0, 0.0),
        ] {
            assert_eq!(stack_slot_at_world_position(world), None);
        }
    }
}
