use bevy::{math::primitives::Cuboid, picking::prelude::*, prelude::*};

use crate::application::pipeline::scene_sync::VisibleBoardState;
use crate::domain::board::{BoardSurfaceId, SLOT_SIZE, SurfaceLayoutKind};
use crate::infrastructure::ui::render_layers::{
    BOARD_VIEW, SCENE_NODE_VISIBILITY, SCENE_ROOT_VISIBILITY,
};

use super::components::*;
use super::helpers::*;
use super::materials::Board3dMaterials;
use super::picking::on_board_base_clicked;

pub(super) const GRID_LINE_WIDTH: f32 = 0.020;
pub(super) const GRID_LINE_LIFT: f32 = 0.005;
/// Fixed mesh count; the grid strides over multiple cells at very wide zoom.
const GRID_HALF_SLOTS: i32 = 48;

pub(super) fn spawn_board_grid_anchor(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
    surface: BoardSurfaceId,
) {
    let extent = GRID_HALF_SLOTS as f32 * 2.0 * SLOT_SIZE;
    let column_line = meshes.add(Cuboid::new(GRID_LINE_WIDTH, GRID_LINE_LIFT, extent));
    let row_line = meshes.add(Cuboid::new(extent, GRID_LINE_LIFT, GRID_LINE_WIDTH));
    let slab = meshes.add(Cuboid::new(extent, BOARD_THICKNESS, extent));

    commands
        .spawn((
            BoardGridAnchor,
            BOARD_VIEW,
            Transform::default(),
            GlobalTransform::default(),
            SCENE_ROOT_VISIBILITY,
        ))
        .with_children(|anchor| {
            anchor
                .spawn((
                    BoardGridPickSlab,
                    BoardDragSurface { surface },
                    Mesh3d(slab),
                    MeshMaterial3d(materials.board.clone()),
                    SCENE_NODE_VISIBILITY,
                    Transform::from_xyz(0.0, -BOARD_THICKNESS * 0.5, 0.0),
                    Pickable::default(),
                ))
                .observe(on_board_base_clicked);

            for line in -GRID_HALF_SLOTS..=GRID_HALF_SLOTS {
                let offset = line as f32 * SLOT_SIZE;
                anchor.spawn((
                    Mesh3d(column_line.clone()),
                    MeshMaterial3d(materials.grid_line.clone()),
                    SCENE_NODE_VISIBILITY,
                    Transform::from_xyz(offset, GRID_LINE_LIFT * 0.5, 0.0),
                    Pickable::IGNORE,
                ));
                anchor.spawn((
                    Mesh3d(row_line.clone()),
                    MeshMaterial3d(materials.grid_line.clone()),
                    SCENE_NODE_VISIBILITY,
                    Transform::from_xyz(0.0, GRID_LINE_LIFT * 0.5, offset),
                    Pickable::IGNORE,
                ));
            }
        });
}

/// Keep the grid backdrop under the camera (snapped to slot pitch so lines
/// stay aligned with slot boundaries) and bound to the active root surface.
pub(super) fn sync_board_grid_anchor(
    visible: Res<'_, VisibleBoardState>,
    camera: Query<
        '_,
        '_,
        (&Transform, &Projection),
        (With<Board3dCamera>, Without<BoardGridAnchor>),
    >,
    viewports: Query<
        &ComputedNode,
        With<crate::infrastructure::ui::board_camera_nav::UiBoardViewport>,
    >,
    mut anchors: Query<'_, '_, (&mut Transform, &mut Visibility), With<BoardGridAnchor>>,
    mut slabs: Query<'_, '_, &mut BoardDragSurface, With<BoardGridPickSlab>>,
) {
    let Ok((mut anchor_transform, mut anchor_visibility)) = anchors.single_mut() else {
        return;
    };

    let on_root_board =
        visible.layout == SurfaceLayoutKind::Board && visible.active_surface.is_some();
    *anchor_visibility = if on_root_board {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !on_root_board {
        return;
    }

    if let (Some(surface), Ok(mut slab)) = (visible.active_surface, slabs.single_mut()) {
        if slab.surface != surface {
            slab.surface = surface;
        }
    }

    let Ok((camera_transform, projection)) = camera.single() else {
        return;
    };
    let Some(focus) = camera_ground_focus(camera_transform) else {
        return;
    };
    let aspect = viewports
        .single()
        .ok()
        .filter(|n| n.size.y > 0.0)
        .map(|n| n.size.x / n.size.y)
        .unwrap_or(1.5);
    let height = match projection {
        Projection::Orthographic(p) => match p.scaling_mode {
            bevy::camera::ScalingMode::FixedVertical { viewport_height } => {
                viewport_height * p.scale
            }
            _ => p.area.height() * p.scale,
        },
        _ => return,
    };
    let stride = grid_stride(height, aspect);
    let pitch = stride * SLOT_SIZE;
    let origin = Vec2::new(
        -crate::domain::board::board_width() * 0.5,
        -crate::domain::board::board_height() * 0.5,
    );
    let snap = |value: f32, origin: f32| origin + ((value - origin) / pitch).round() * pitch;
    let translation = Vec3::new(snap(focus.x, origin.x), 0.0, snap(focus.z, origin.y));
    let scale = Vec3::new(stride, 1.0, stride);
    if anchor_transform.translation != translation || anchor_transform.scale != scale {
        anchor_transform.translation = translation;
        anchor_transform.scale = scale;
    }
}

/// Intersection of the camera's view axis with the board plane (Y = 0).
fn camera_ground_focus(camera: &Transform) -> Option<Vec3> {
    let forward = camera.forward();
    if forward.y.abs() < 1e-5 {
        return None;
    }
    let t = -camera.translation.y / forward.y;
    if t < 0.0 {
        return None;
    }
    Some(camera.translation + forward * t)
}

/// The view diagonal bounds every yaw. Two cells of margin cover snapping and
/// the partially visible outer cells without adding meshes as the view grows.
fn grid_stride(height: f32, aspect: f32) -> f32 {
    let diagonal = height * aspect.hypot(1.0);
    let needed = diagonal / ((GRID_HALF_SLOTS - 2) as f32 * 2.0 * SLOT_SIZE);
    if !needed.is_finite() {
        return 1.0;
    }
    2.0_f32.powf(needed.max(1.0).log2().ceil())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_grid_covers_wide_and_rotated_views_at_integer_cell_strides() {
        for height in [3.0, 9.0, 24.0, 200.0, 10_000.0] {
            for aspect in [0.25, 1.5, 4.0] {
                let stride = grid_stride(height, aspect);
                assert_eq!(stride.fract(), 0.0);
                assert!(stride >= 1.0);
                let usable = (GRID_HALF_SLOTS - 2) as f32 * 2.0 * stride;
                assert!(usable + 0.01 >= height * aspect.hypot(1.0));
            }
        }
    }
}
