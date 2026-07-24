use bevy::{math::primitives::Cuboid, picking::prelude::*, prelude::*};

use crate::application::editor::{BoardPlacementPointer, EditorSession, PickHit};
use crate::application::pipeline::scene_sync::{VisibleBoardNode, VisibleBoardState, VisibleNodeKind};
use crate::domain::board::{SLOT_SIZE, SurfaceLayoutKind};
use crate::domain::document::PlacementAddress;
use crate::infrastructure::ui::board_geometry::tessera_slot_center;
use crate::infrastructure::ui::musaic_tile::{
    TILE_INSET, board_footprint_for_kind, stack_footprint_for_kind, tile_visual_for_visible,
};
use crate::infrastructure::ui::placement_preview::{
    resolve_placement_preview_center, tessera_footprint_for_preview,
};
use crate::infrastructure::ui::render_layers::{BOARD_VIEW, SCENE_NODE_VISIBILITY, SCENE_ROOT_VISIBILITY};
use crate::infrastructure::ui::tile_mesh::TileMeshAssets;
use crate::infrastructure::ui::tile_visual::{
    BoardTileMode, BoardTileSpec, board_tile_transform, spawn_board_tile, visible_kind_for_spawn,
};
use crate::infrastructure::ui::transform_tile::TransformTileAssets;

use super::components::*;
use super::helpers::*;
use super::materials::Board3dMaterials;

pub(super) fn sync_drag_preview(
    mut commands: Commands,
    session: Res<'_, EditorSession>,
    visible: Res<'_, VisibleBoardState>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut standard_materials: ResMut<'_, Assets<StandardMaterial>>,
    materials: Res<'_, Board3dMaterials>,
    tile_meshes: Option<Res<'_, TileMeshAssets>>,
    transform_tiles: Res<'_, TransformTileAssets>,
    previews: Query<'_, '_, Entity, With<BoardDragPreview>>,
) {
    if session.placement_tile().is_none() {
        for entity in previews.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let preview_kind = session
        .placement_tile()
        .map(visible_kind_for_spawn)
        .unwrap_or(VisibleNodeKind::Atom);

    let Some(center) = resolve_placement_preview_center(&session, &visible, preview_kind) else {
        for entity in previews.iter() {
            commands.entity(entity).despawn();
        }
        return;
    };

    let on_root_board = visible.layout == SurfaceLayoutKind::Board;
    let tessera_footprint = tessera_footprint_for_preview(preview_kind, on_root_board);
    let footprint = if visible.layout == SurfaceLayoutKind::Stack {
        stack_footprint_for_kind(preview_kind)
    } else {
        board_footprint_for_kind(preview_kind)
    };
    let ortho = tile_visual_for_visible(preview_kind, false, false, None).map(|mut spec| {
        spec.footprint = footprint;
        spec
    });
    let spec = BoardTileSpec {
        kind: preview_kind,
        tessera_footprint,
        on_root_board,
        plane_anchor: Vec3::new(center.x, SLOT_HEIGHT, center.z),
        visual_footprint: footprint,
        ortho,
    };

    if let Some(entity) = previews.iter().next() {
        let transform = board_tile_transform(
            tile_meshes.as_deref(),
            &spec,
            BoardTileMode::Preview,
        );
        commands.entity(entity).insert(transform);
        return;
    }

    spawn_board_tile(
        &mut commands,
        tile_meshes.as_deref(),
        Some(transform_tiles.as_ref()),
        &mut meshes,
        &mut standard_materials,
        Some(&materials.drag_preview),
        &spec,
        BoardTileMode::Preview,
        (BoardDragPreview, BOARD_VIEW, SCENE_ROOT_VISIBILITY),
    );
}

/// Renders the provisional root-board edge while the connection statechart is active.
pub(super) fn sync_connection_preview(
    mut commands: Commands,
    session: Res<'_, EditorSession>,
    pointer: Res<'_, BoardPlacementPointer>,
    visible: Res<'_, VisibleBoardState>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    materials: Res<'_, Board3dMaterials>,
    previews: Query<'_, '_, Entity, With<BoardConnectionPreview>>,
) {
    let Some(source) = session.connection_source() else {
        for entity in previews.iter() {
            commands.entity(entity).despawn();
        }
        return;
    };
    if visible.layout != SurfaceLayoutKind::Board {
        for entity in previews.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let Some(source_node) = visible.nodes.iter().find(|node| &node.node == source) else {
        for entity in previews.iter() {
            commands.entity(entity).despawn();
        }
        return;
    };
    let Some(world) = pointer.cursor_world else {
        for entity in previews.iter() {
            commands.entity(entity).despawn();
        }
        return;
    };
    let Some(target_slot) =
        crate::domain::board::geometry::slot_at_world_position(world, visible.layout)
    else {
        for entity in previews.iter() {
            commands.entity(entity).despawn();
        }
        return;
    };

    let source_center = visible_node_center(source_node, CONNECTION_LIFT);
    let target_center = visible
        .pick_at(target_slot)
        .and_then(|hit| match hit {
            PickHit::Tile { node } => visible
                .nodes
                .iter()
                .find(|candidate| candidate.node == node),
            _ => None,
        })
        .map(|node| visible_node_center(node, CONNECTION_LIFT))
        .unwrap_or_else(|| slot_position(target_slot, CONNECTION_LIFT));
    let delta = target_center - source_center;
    let length = delta.length();
    if length <= f32::EPSILON {
        for entity in previews.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let transform = Transform {
        translation: source_center.lerp(target_center, 0.5),
        rotation: Quat::from_rotation_y(-delta.z.atan2(delta.x)),
        scale: Vec3::new(length, 1.0, 1.0),
    };
    if let Some(entity) = previews.iter().next() {
        commands.entity(entity).insert(transform);
        return;
    }

    commands.spawn((
        BoardConnectionPreview,
        Mesh3d(meshes.add(Cuboid::new(1.0, CONNECTION_HEIGHT, CONNECTION_THICKNESS))),
        MeshMaterial3d(materials.connection_preview.clone()),
        SCENE_NODE_VISIBILITY,
        Pickable::IGNORE,
        transform,
    ));
}

fn visible_node_center(node: &VisibleBoardNode, y: f32) -> Vec3 {
    match node.address {
        PlacementAddress::BoardSlot(slot) => {
            tessera_slot_center(slot, tessera_footprint_for_node(node), y)
        }
        PlacementAddress::StackIndex(_) => unreachable!("root-board connection preview only"),
    }
}

pub(super) fn sync_placement_slot_highlight(
    mut commands: Commands,
    session: Res<'_, EditorSession>,
    visible: Res<'_, VisibleBoardState>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    materials: Res<'_, Board3dMaterials>,
    highlights: Query<'_, '_, Entity, With<BoardSlotHighlight>>,
) {
    for entity in highlights.iter() {
        commands.entity(entity).despawn();
    }

    if !session.is_placing_from_drawer() {
        return;
    }

    let Some(hover) = session.placement_hover() else {
        return;
    };

    let highlight_transform = if visible.layout == SurfaceLayoutKind::Stack {
        let PlacementAddress::StackIndex(index) = hover.address else {
            return;
        };
        Transform::from_translation(stack_insert_marker_center(index))
    } else {
        let slot = hovered_slot_to_visual_slot(hover.address);
        let footprint = SLOT_SIZE - TILE_INSET;
        Transform::from_translation(tile_world_position(
            visual_slot_to_address(visible.layout, slot),
            footprint,
        ))
    };

    let ring = meshes.add(Cuboid::new(
        SLOT_SIZE - TILE_INSET * 2.0,
        0.04,
        SLOT_SIZE - TILE_INSET * 2.0,
    ));
    commands.spawn((
        BoardSlotHighlight,
        BOARD_VIEW,
        Mesh3d(ring),
        MeshMaterial3d(materials.drag_preview.clone()),
        SCENE_NODE_VISIBILITY,
        highlight_transform,
        Pickable::IGNORE,
    ));
}

pub(super) fn animate_placement_pulse(
    time: Res<Time>,
    session: Res<'_, EditorSession>,
    mut commands: Commands,
    mut pulsing: Query<(Entity, &mut Transform, &mut TilePlacementPulse)>,
    tiles: Query<(Entity, &Board3dTile, &Transform), Without<TilePlacementPulse>>,
    mut last_pulse_address: Local<Option<PlacementAddress>>,
) {
    const PULSE_DURATION: f32 = 0.3;

    if let Some(address) = session.last_placement_address {
        if *last_pulse_address != Some(address) {
            *last_pulse_address = Some(address);
            for (entity, tile, transform) in tiles.iter() {
                if tile.address == address {
                    commands.entity(entity).insert(TilePlacementPulse {
                        elapsed: 0.0,
                        base_scale: transform.scale,
                    });
                }
            }
        }
    }

    for (entity, mut transform, mut pulse) in pulsing.iter_mut() {
        pulse.elapsed += time.delta_secs();
        let t = (pulse.elapsed / PULSE_DURATION).clamp(0.0, 1.0);
        transform.scale = pulse.base_scale * (1.0 + (1.0 - t) * 0.12);
        if pulse.elapsed >= PULSE_DURATION {
            transform.scale = pulse.base_scale;
            commands.entity(entity).remove::<TilePlacementPulse>();
        }
    }
}

