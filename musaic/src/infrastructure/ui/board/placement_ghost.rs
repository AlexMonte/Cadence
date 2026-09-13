use bevy::{math::primitives::Cuboid, picking::prelude::*, prelude::*};

use crate::application::editor::{BoardPlacementPointer, EditorSession, PickHit};
use crate::application::pipeline::scene_sync::{
    VisibleBoardNode, VisibleBoardState, VisibleNodeKind,
};
use crate::domain::board::{SLOT_SIZE, SurfaceLayoutKind};
use crate::domain::document::PlacementAddress;
use crate::infrastructure::ui::board_geometry::tessera_slot_center;
use crate::infrastructure::ui::musaic_tile::TILE_INSET;
use crate::infrastructure::ui::placement_preview::{
    resolve_placement_preview_center, tessera_footprint_for_preview,
};
use crate::infrastructure::ui::render_layers::{
    BOARD_VIEW, SCENE_NODE_VISIBILITY, SCENE_ROOT_VISIBILITY,
};
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::tile_surface::content_for_spawn;
use crate::infrastructure::ui::tile_visual::visible_kind_for_spawn;

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
    theme: Res<'_, MusaicUiTheme>,
    previews: Query<'_, '_, (Entity, &BoardDragPreview)>,
) {
    if session.placement_tile().is_none() {
        for (entity, _) in &previews {
            commands.entity(entity).despawn();
        }
        return;
    }

    let preview_kind = session
        .placement_tile()
        .map(visible_kind_for_spawn)
        .unwrap_or(VisibleNodeKind::Atom);

    let Some(mut center) = resolve_placement_preview_center(&session, &visible, preview_kind)
    else {
        for (entity, _) in &previews {
            commands.entity(entity).despawn();
        }
        return;
    };

    let on_root_board = visible.layout == SurfaceLayoutKind::Board;
    let mut tessera_footprint = tessera_footprint_for_preview(preview_kind, on_root_board);
    if !on_root_board {
        if let crate::application::editor::EditorMode::Placing(placement) = &session.mode {
            if let Some(source) = placement
                .source
                .as_ref()
                .and_then(|id| visible.nodes.iter().find(|n| &n.node == id))
            {
                let next = tessera_footprint_for_node(source);
                center.x += (next.width as f32 - tessera_footprint.width as f32) * 0.5;
                tessera_footprint = next;
            }
        }
    }
    let transform = Transform::from_xyz(center.x, SLOT_HEIGHT + 0.025, center.z);
    let tile = session.placement_tile().expect("placing tile");
    if let Some((entity, previous)) = previews.iter().next() {
        if previous.tile == *tile {
            commands.entity(entity).insert(transform);
            return;
        }
        commands.entity(entity).despawn();
    }
    let content = match &session.mode {
        crate::application::editor::EditorMode::Placing(placement) => placement
            .source
            .as_ref()
            .and_then(|source| visible.nodes.iter().find(|node| &node.node == source))
            .map(|node| node.surface_content.clone())
            .unwrap_or_else(|| content_for_spawn(tile)),
        _ => content_for_spawn(tile),
    };
    let (mesh, details) = super::detail::face(
        &mut meshes,
        preview_kind,
        &content,
        Vec2::new(
            tessera_footprint.width as f32,
            tessera_footprint.height as f32,
        ),
        false,
        true,
        &theme,
    );
    let mut material = standard_materials
        .get(&materials.surface)
        .cloned()
        .unwrap_or_default();
    material.base_color.set_alpha(0.60);
    material.alpha_mode = AlphaMode::Blend;
    commands.spawn((
        BoardDragPreview { tile: tile.clone() },
        BOARD_VIEW,
        SCENE_ROOT_VISIBILITY,
        mesh,
        details,
        MeshMaterial3d(standard_materials.add(material)),
        transform,
        Pickable::IGNORE,
    ));
}

/// Renders the provisional root-board edge while the connection statechart is active.
pub(super) fn sync_connection_preview(
    mut commands: Commands,
    session: Res<EditorSession>,
    pointer: Res<BoardPlacementPointer>,
    visible: Res<VisibleBoardState>,
    project: Res<crate::MusaicProject>,
    mut route: Local<
        Option<(
            tessera::prelude::NodeId,
            tessera::prelude::NodeId,
            u64,
            Option<tessera::prelude::SpatialSide>,
        )>,
    >,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Board3dMaterials>,
    previews: Query<(Entity, &BoardConnectionPreview)>,
) {
    let active = session.connection_source().and_then(|source| {
        let source_node = visible.nodes.iter().find(|n| &n.node == source)?;
        let world = pointer.cursor_world?;
        let slot = crate::domain::board::geometry::slot_at_world_position(world, visible.layout)?;
        (visible.layout == SurfaceLayoutKind::Board).then_some((source_node, slot))
    });
    let Some((source_node, target_slot)) = active else {
        for (entity, _) in &previews {
            commands.entity(entity).despawn();
        }
        *route = None;
        return;
    };
    let target_node = visible.pick_at(target_slot).and_then(|hit| match hit {
        PickHit::Tile { node } => visible
            .nodes
            .iter()
            .find(|candidate| candidate.node == node),
        _ => None,
    });
    let source = visible_node_center(source_node, CONNECTION_LIFT);
    let target = target_node
        .map(|node| visible_node_center(node, CONNECTION_LIFT))
        .unwrap_or_else(|| slot_position(target_slot, CONNECTION_LIFT));
    let half = |node: &VisibleBoardNode| {
        let f = tessera_footprint_for_node(node);
        crate::infrastructure::ui::tile_surface::face_size(node.kind)
            * Vec2::new(f.width as f32, f.height as f32)
            * 0.5
    };
    use tessera::prelude::SpatialSide;
    let delta = target - source;
    let mut side = if delta.x.abs() >= delta.z.abs() {
        if delta.x >= 0.0 {
            SpatialSide::East
        } else {
            SpatialSide::West
        }
    } else if delta.z >= 0.0 {
        SpatialSide::South
    } else {
        SpatialSide::North
    };
    if let Some(target_node) = target_node {
        let revision = project.document.revision.0;
        if route.as_ref().is_none_or(|(from, to, r, _)| {
            from != &source_node.node || to != &target_node.node || *r != revision
        }) {
            let planned = crate::domain::document::export_document_program(&project.document)
                .ok()
                .and_then(|program| {
                    crate::domain::document::connection_policy::authorize_manual_connection(
                        &program,
                        &source_node.node,
                        &target_node.node,
                    )
                    .ok()
                })
                .map(|edge| edge.side);
            *route = Some((
                source_node.node.clone(),
                target_node.node.clone(),
                revision,
                planned,
            ));
        }
        if let Some((_, _, _, Some(planned))) = &*route {
            side = *planned;
        }
    }
    let points = crate::infrastructure::ui::board_geometry::connection_path(
        source,
        half(source_node),
        target,
        target_node.map(half).unwrap_or(Vec2::splat(0.48)),
        side,
    );
    let transform = |index: usize| {
        points.get(index).zip(points.get(index + 1)).map(|(a, b)| {
            let delta = *b - *a;
            Transform {
                translation: a.lerp(*b, 0.5),
                rotation: Quat::from_rotation_y(-delta.z.atan2(delta.x)),
                scale: Vec3::new(delta.length(), 1.0, 1.0),
            }
        })
    };
    if previews.is_empty() {
        let mesh = meshes.add(Cuboid::new(1.0, CONNECTION_HEIGHT, CONNECTION_THICKNESS));
        for index in 0..5 {
            commands.spawn((
                BoardConnectionPreview(index),
                Mesh3d(mesh.clone()),
                MeshMaterial3d(materials.connection_preview.clone()),
                Pickable::IGNORE,
                transform(index).unwrap_or_default(),
                if transform(index).is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ));
        }
    } else {
        for (entity, segment) in &previews {
            commands.entity(entity).insert((
                transform(segment.0).unwrap_or_default(),
                if transform(segment.0).is_some() {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                },
            ));
        }
    }
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
        Transform::from_translation(stack_insert_marker_center(
            visible.stack_display.display_index(index),
        ))
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
    preferences: Option<Res<crate::application::editor::preferences::EditorPreferences>>,
    session: Res<'_, EditorSession>,
    mut commands: Commands,
    mut pulsing: Query<(Entity, &mut Transform, &mut TilePlacementPulse)>,
    tiles: Query<(Entity, &Board3dTile, &Transform), Without<TilePlacementPulse>>,
    mut last_pulse_address: Local<Option<PlacementAddress>>,
) {
    const PULSE_DURATION: f32 = 0.3;

    if preferences
        .as_ref()
        .is_some_and(|preferences| preferences.reduced_motion)
    {
        *last_pulse_address = session.last_placement_address;
        for (entity, mut transform, pulse) in &mut pulsing {
            transform.scale = pulse.base_scale;
            commands.entity(entity).remove::<TilePlacementPulse>();
        }
        return;
    }

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
