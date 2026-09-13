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
    Vec3::new(
        stack_column_center(index),
        y,
        crate::domain::board::geometry::stack_row_center(index.0),
    )
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
        PlacementAddress::StackIndex(index) => {
            stack_flat_tile_center(index, kind, ground_y)
                + Vec3::X * (tessera_footprint.width.saturating_sub(1) as f32 * SLOT_SIZE * 0.5)
        }
    }
}

/// Orthogonal cable between authored port sides, with an outside lane for a backward route.
pub(crate) fn connection_path(
    from: Vec3,
    from_half: Vec2,
    to: Vec3,
    to_half: Vec2,
    side: tessera::prelude::SpatialSide,
) -> Vec<Vec3> {
    use tessera::prelude::SpatialSide;
    let direction = match side {
        SpatialSide::West => -Vec3::X,
        SpatialSide::North => -Vec3::Z,
        SpatialSide::South => Vec3::Z,
        _ => Vec3::X,
    };
    let horizontal = direction.x != 0.0;
    let start = from + direction * if horizontal { from_half.x } else { from_half.y };
    let end = to - direction * if horizontal { to_half.x } else { to_half.y };
    if start.distance_squared(end) < 0.0001 {
        return vec![start - direction * 0.12, end + direction * 0.08];
    }
    let gap = (end - start).dot(direction);
    let points = if gap >= 0.0 {
        if horizontal {
            let middle = (start.x + end.x) * 0.5;
            vec![
                start,
                Vec3::new(middle, start.y, start.z),
                Vec3::new(middle, end.y, end.z),
                end,
            ]
        } else {
            let middle = (start.z + end.z) * 0.5;
            vec![
                start,
                Vec3::new(start.x, start.y, middle),
                Vec3::new(end.x, end.y, middle),
                end,
            ]
        }
    } else {
        let exit = start + direction * 0.3;
        let entry = end - direction * 0.3;
        if horizontal {
            let lane = (from.z - from_half.y).min(to.z - to_half.y) - 0.5;
            vec![
                start,
                exit,
                Vec3::new(exit.x, exit.y, lane),
                Vec3::new(entry.x, entry.y, lane),
                entry,
                end,
            ]
        } else {
            let lane = (from.x - from_half.x).min(to.x - to_half.x) - 0.5;
            vec![
                start,
                exit,
                Vec3::new(lane, exit.y, exit.z),
                Vec3::new(lane, entry.y, entry.z),
                entry,
                end,
            ]
        }
    };
    let mut result = Vec::new();
    for point in points {
        if result
            .last()
            .is_some_and(|previous: &Vec3| previous.distance_squared(point) <= 0.000001)
        {
            continue;
        }
        if result.len() >= 2 {
            let a = result[result.len() - 1] - result[result.len() - 2];
            let b = point - result[result.len() - 1];
            if a.cross(b).length_squared() < 0.000001 && a.dot(b) > 0.0 {
                result.pop();
            }
        }
        result.push(point);
    }
    result
}

#[cfg(test)]
mod cable_geometry_tests {
    use super::*;
    use tessera::prelude::SpatialSide;
    #[test]
    fn touching_faces_have_one_short_connector_without_a_loop() {
        let points = connection_path(
            Vec3::ZERO,
            Vec2::splat(0.5),
            Vec3::X,
            Vec2::splat(0.5),
            SpatialSide::East,
        );
        assert_eq!(points.len(), 2);
        assert!(points[0].distance(points[1]) < 0.3);
        assert_eq!(points[0].z, 0.0);
        assert_eq!(points[1].z, 0.0);
    }
    #[test]
    fn cables_use_authored_sides_and_orthogonal_segments_even_after_moving_behind_source() {
        for (side, direction) in [
            (SpatialSide::East, Vec3::X),
            (SpatialSide::West, -Vec3::X),
            (SpatialSide::South, Vec3::Z),
            (SpatialSide::North, -Vec3::Z),
        ] {
            for target in [Vec3::new(8.0, 0.1, 3.0), Vec3::new(-8.0, 0.1, -3.0)] {
                let start = Vec3::new(0.0, 0.1, 0.0);
                let points =
                    connection_path(start, Vec2::splat(0.5), target, Vec2::splat(0.5), side);
                assert_eq!(points[0], start + direction * 0.5);
                assert_eq!(*points.last().unwrap(), target - direction * 0.5);
                assert!((points[1] - points[0]).dot(direction) > 0.0);
                assert!((points[points.len() - 1] - points[points.len() - 2]).dot(direction) > 0.0);
                for pair in points.windows(2) {
                    let delta = pair[1] - pair[0];
                    assert!(delta.is_finite() && delta.length() > 0.0);
                    assert!(delta.x == 0.0 || delta.z == 0.0);
                }
            }
        }
    }
}
