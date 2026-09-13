use std::collections::{BTreeMap, BTreeSet};

use bevy::{
    asset::RenderAssetUsages,
    camera::ScalingMode,
    math::primitives::Cuboid,
    picking::prelude::*,
    prelude::*,
    render::render_resource::{TextureDimension, TextureFormat, TextureUsages},
};
use tessera::prelude::NodeId;

use crate::application::editor::EditorAttention;
use crate::application::editor::interaction::BoardPickTargetKind;
use crate::application::pipeline::scene_sync::{
    VisibleBoardConnection, VisibleBoardNode, VisibleBoardState, VisibleNodeKind,
};
use crate::domain::board::{BoardSurfaceId, SurfaceLayoutKind};
use crate::domain::document::{PlacementAddress, PortSlotState, StackIndex};
use crate::infrastructure::ui::board_geometry::placement_center_for_address;
use crate::infrastructure::ui::musaic_tile::TILE_HEIGHT;
use crate::infrastructure::ui::render_layers::{
    BOARD_VIEW, SCENE_NODE_VISIBILITY, SCENE_ROOT_VISIBILITY,
};
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::ui::tile_surface::compact_port_mesh;

use super::components::*;
use super::grid::spawn_board_grid_anchor;
use super::helpers::*;
use super::materials::{Board3dMaterials, Board3dRenderCache};
use super::picking::{on_board_base_clicked, on_tile_clicked, on_tile_pressed};

pub(super) fn teardown_board_3d_scene(
    mut commands: Commands,
    mut cache: ResMut<Board3dRenderCache>,
    cameras: Query<Entity, With<Board3dCamera>>,
    roots: Query<Entity, With<Board3dRoot>>,
    anchors: Query<Entity, With<BoardGridAnchor>>,
    lights: Query<Entity, With<Board3dLight>>,
) {
    *cache = Board3dRenderCache::default();
    for entity in cameras
        .iter()
        .chain(roots.iter())
        .chain(anchors.iter())
        .chain(lights.iter())
    {
        commands.entity(entity).despawn();
    }
}

pub(super) fn setup_board_3d_scene(
    mut commands: Commands,
    theme: Res<'_, crate::infrastructure::ui::theme::MusaicUiTheme>,
    mut images: ResMut<'_, Assets<Image>>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    materials: Res<'_, Board3dMaterials>,
    attention: Res<'_, EditorAttention>,
) {
    let mut image = Image::new_uninit(
        default(),
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let image_handle = images.add(image);

    spawn_board_grid_anchor(
        &mut commands,
        &mut meshes,
        &materials,
        attention.active_board(),
    );

    // Initial pose mirrors the rig's default; from here on the rig's
    // smoother is the only writer of this Transform/Projection.
    let rig_default = crate::infrastructure::ui::camera_rig::BoardCameraRig::default();
    commands.spawn((
        Board3dCamera,
        Camera3d::default(),
        bevy::core_pipeline::tonemapping::Tonemapping::None,
        MeshPickingCamera,
        BOARD_VIEW,
        SCENE_ROOT_VISIBILITY,
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(theme.chrome.board_clear),
            ..default()
        },
        bevy::camera::RenderTarget::Image(image_handle.into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: rig_default.viewport_height,
            },
            scale: 1.0,
            near: -1000.0,
            far: 1000.0,
            ..OrthographicProjection::default_3d()
        }),
        rig_default.desired_transform(),
    ));

    commands.spawn((
        Board3dLight,
        DirectionalLight {
            illuminance: 12000.0,
            shadows_enabled: false,
            ..default()
        },
        BOARD_VIEW,
        SCENE_ROOT_VISIBILITY,
        Transform::from_xyz(0.0, 12.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

pub(super) fn sync_board_3d_scene(
    mut commands: Commands,
    visible: Res<VisibleBoardState>,
    mut cache: ResMut<Board3dRenderCache>,
    materials: Res<Board3dMaterials>,
    theme: Res<MusaicUiTheme>,
    mut meshes: ResMut<Assets<Mesh>>,
    roots: Query<Entity, With<Board3dRoot>>,
) {
    let Some(surface) = visible.active_surface else {
        return;
    };

    let stack_columns = stack_display_columns(&visible);
    let structure_changed = cache.active_surface != Some(surface)
        || cache.layout != visible.layout
        || cache.stack_columns != stack_columns
        || cache.root.is_none()
        || !cache
            .root
            .is_some_and(|root| roots.iter().any(|e| e == root));

    let root = if structure_changed {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        *cache = Board3dRenderCache {
            active_surface: Some(surface),
            layout: visible.layout,
            stack_columns,
            ..Default::default()
        };
        let root = commands
            .spawn((
                Board3dRoot,
                BOARD_VIEW,
                Transform::default(),
                GlobalTransform::default(),
                SCENE_ROOT_VISIBILITY,
            ))
            .id();
        cache.root = Some(root);
        commands.entity(root).with_children(|parent| {
            spawn_surface_base(
                parent,
                surface,
                visible.layout,
                stack_columns,
                &mut meshes,
                &materials,
            );
        });
        root
    } else {
        cache.root.expect("root present when structure unchanged")
    };

    reconcile_board_tiles(
        &mut commands,
        root,
        surface,
        &visible,
        &mut cache,
        &mut meshes,
        &materials,
        &theme,
    );
    reconcile_board_connections(
        &mut commands,
        root,
        surface,
        &visible,
        &mut cache,
        &mut meshes,
        &materials,
    );
    reconcile_stack_markers(
        &mut commands,
        root,
        surface,
        &visible,
        &mut cache,
        &mut meshes,
        &materials,
    );
}

fn reconcile_board_tiles(
    commands: &mut Commands,
    root: Entity,
    surface: BoardSurfaceId,
    visible: &VisibleBoardState,
    cache: &mut Board3dRenderCache,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
    theme: &MusaicUiTheme,
) {
    let desired: BTreeMap<NodeId, TileVisualKey> = visible
        .nodes
        .iter()
        .filter(|node| visible_tile_face(node))
        .map(|n| {
            (
                n.node.clone(),
                TileVisualKey::from_node(n, visible.display_address(n.address)),
            )
        })
        .collect();

    let stale: Vec<NodeId> = cache
        .tiles
        .keys()
        .filter(|id| !desired.contains_key(*id))
        .cloned()
        .collect();
    for id in stale {
        if let Some((entity, _)) = cache.tiles.remove(&id) {
            commands.entity(entity).despawn();
        }
    }

    for node in visible.nodes.iter().filter(|node| visible_tile_face(node)) {
        let key = TileVisualKey::from_node(node, visible.display_address(node.address));
        let needs_spawn = match cache.tiles.get(&node.node) {
            Some((entity, old_key)) if old_key == &key => {
                let _ = entity;
                false
            }
            Some((entity, _)) => {
                commands.entity(*entity).despawn();
                true
            }
            None => true,
        };
        if !needs_spawn {
            continue;
        }
        let mut spawned = None;
        commands.entity(root).with_children(|parent| {
            spawned = spawn_one_tile(
                parent,
                surface,
                node,
                visible.display_address(node.address),
                meshes,
                materials,
                theme,
            );
        });
        if let Some(entity) = spawned {
            cache.tiles.insert(node.node.clone(), (entity, key));
        }
    }
}

fn reconcile_board_connections(
    commands: &mut Commands,
    root: Entity,
    surface: BoardSurfaceId,
    visible: &VisibleBoardState,
    cache: &mut Board3dRenderCache,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
) {
    if visible.layout != SurfaceLayoutKind::Board {
        for (_, (entity, _)) in std::mem::take(&mut cache.connections) {
            commands.entity(entity).despawn();
        }
        return;
    }

    let desired: BTreeMap<(NodeId, NodeId), ConnectionVisualKey> = visible
        .connections
        .iter()
        .map(|c| {
            (
                (c.from.clone(), c.to.clone()),
                ConnectionVisualKey {
                    side: c.side,
                    from_slot: c.from_slot,
                    to_slot: c.to_slot,
                    kind: c.kind,
                },
            )
        })
        .collect();

    let stale: Vec<(NodeId, NodeId)> = cache
        .connections
        .keys()
        .filter(|k| !desired.contains_key(*k))
        .cloned()
        .collect();
    for key in stale {
        if let Some((entity, _)) = cache.connections.remove(&key) {
            commands.entity(entity).despawn();
        }
    }

    for connection in &visible.connections {
        let key = (connection.from.clone(), connection.to.clone());
        let visual = ConnectionVisualKey {
            side: connection.side,
            from_slot: connection.from_slot,
            to_slot: connection.to_slot,
            kind: connection.kind,
        };
        let needs_spawn = match cache.connections.get(&key) {
            Some((_, old)) if old == &visual => false,
            Some((entity, _)) => {
                commands.entity(*entity).despawn();
                true
            }
            None => true,
        };
        if !needs_spawn {
            continue;
        }
        let mut spawned = None;
        commands.entity(root).with_children(|parent| {
            spawned = spawn_one_connection(parent, surface, connection, visible, meshes, materials);
        });
        if let Some(entity) = spawned {
            cache.connections.insert(key, (entity, visual));
        }
    }
}

fn reconcile_stack_markers(
    commands: &mut Commands,
    root: Entity,
    surface: BoardSurfaceId,
    visible: &VisibleBoardState,
    cache: &mut Board3dRenderCache,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
) {
    if visible.layout != SurfaceLayoutKind::Stack {
        for (_, entity) in std::mem::take(&mut cache.stack_inserts) {
            commands.entity(entity).despawn();
        }
        for (_, entity) in std::mem::take(&mut cache.stack_locked) {
            commands.entity(entity).despawn();
        }
        return;
    }

    let desired_inserts: BTreeSet<_> = visible.stack_inserts.iter().copied().collect();
    let stale_inserts: Vec<_> = cache
        .stack_inserts
        .keys()
        .filter(|k| !desired_inserts.contains(k))
        .copied()
        .collect();
    for key in stale_inserts {
        if let Some(entity) = cache.stack_inserts.remove(&key) {
            commands.entity(entity).despawn();
        }
    }
    for index in &visible.stack_inserts {
        if let Some(entity) = cache.stack_inserts.get(index) {
            commands.entity(*entity).insert(Transform::from_translation(
                stack_insert_marker_center(visible.stack_display.display_index(*index)),
            ));
            continue;
        }
        let mut spawned = None;
        commands.entity(root).with_children(|parent| {
            spawned = Some(spawn_one_stack_insert(
                parent,
                surface,
                *index,
                visible.stack_display.display_index(*index),
                meshes,
                materials,
            ));
        });
        if let Some(entity) = spawned {
            cache.stack_inserts.insert(*index, entity);
        }
    }

    let desired_locked: BTreeSet<_> = visible.stack_locked_slots.iter().copied().collect();
    let stale_locked: Vec<_> = cache
        .stack_locked
        .keys()
        .filter(|k| !desired_locked.contains(k))
        .copied()
        .collect();
    for key in stale_locked {
        if let Some(entity) = cache.stack_locked.remove(&key) {
            commands.entity(entity).despawn();
        }
    }
    for index in &visible.stack_locked_slots {
        if let Some(entity) = cache.stack_locked.get(index) {
            commands.entity(*entity).insert(Transform::from_translation(
                stack_insert_marker_center(visible.stack_display.display_index(*index)),
            ));
            continue;
        }
        let mut spawned = None;
        commands.entity(root).with_children(|parent| {
            spawned = Some(spawn_one_stack_locked(
                parent,
                visible.stack_display.display_index(*index),
                meshes,
                materials,
            ));
        });
        if let Some(entity) = spawned {
            cache.stack_locked.insert(*index, entity);
        }
    }
}

fn spawn_one_connection(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    connection: &VisibleBoardConnection,
    visible: &VisibleBoardState,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
) -> Option<Entity> {
    let geometry = |node_id: &NodeId, slot| {
        let node = visible.nodes.iter().find(|node| &node.node == node_id);
        let footprint = node
            .map(tessera_footprint_for_node)
            .unwrap_or(tessera::prelude::TileFootprint::unit());
        let center = crate::infrastructure::ui::board_geometry::tessera_slot_center(
            slot,
            footprint,
            CONNECTION_LIFT,
        );
        let size = crate::infrastructure::ui::tile_surface::face_size(
            node.map(|n| n.kind).unwrap_or(VisibleNodeKind::Tile),
        ) * Vec2::new(footprint.width as f32, footprint.height as f32);
        (center, size * 0.5)
    };
    let (from, from_half) = geometry(&connection.from, connection.from_slot);
    let (to, to_half) = geometry(&connection.to, connection.to_slot);
    let points = crate::infrastructure::ui::board_geometry::connection_path(
        from,
        from_half,
        to,
        to_half,
        connection.side,
    );
    let total: f32 = points.windows(2).map(|p| p[0].distance(p[1])).sum();
    if total <= f32::EPSILON {
        return None;
    }
    let material = if connection.kind == PortSlotState::Input {
        materials.connection_scalar.clone()
    } else {
        materials.connection_control.clone()
    };
    Some(
        parent
            .spawn((
                ConnectionLineEntity,
                Transform::default(),
                SCENE_NODE_VISIBILITY,
            ))
            .with_children(|wire| {
                let mut offset = 0.0;
                for (index, pair) in points.windows(2).enumerate() {
                    let delta = pair[1] - pair[0];
                    let length = delta.length();
                    if length <= f32::EPSILON {
                        continue;
                    }
                    wire.spawn((
                        super::playback_activity::ConnectionPlaybackPath {
                            from: connection.from.clone(),
                            to: connection.to.clone(),
                            length,
                            offset,
                            total,
                        },
                        BoardPickTarget {
                            surface_id: surface,
                            kind: BoardPickTargetKind::Connection {
                                from: connection.from.clone(),
                                to: connection.to.clone(),
                            },
                        },
                        Mesh3d(meshes.add(Cuboid::new(
                            length,
                            CONNECTION_HEIGHT,
                            CONNECTION_THICKNESS,
                        ))),
                        MeshMaterial3d(material.clone()),
                        SCENE_NODE_VISIBILITY,
                        Transform {
                            translation: pair[0].lerp(pair[1], 0.5),
                            rotation: Quat::from_rotation_y(-delta.z.atan2(delta.x)),
                            ..default()
                        },
                    ))
                    .observe(on_tile_clicked)
                    .with_children(|segment| {
                        if index == points.len() - 2 {
                            segment.spawn((
                                Mesh3d(meshes.add(
                                    crate::infrastructure::ui::tile_surface::connection_arrow_mesh(
                                    ),
                                )),
                                MeshMaterial3d(material.clone()),
                                SCENE_NODE_VISIBILITY,
                                Transform::from_xyz(length * 0.5, 0.0, 0.0),
                                Pickable::IGNORE,
                            ));
                        }
                    });
                    offset += length;
                }
            })
            .id(),
    )
}

fn spawn_one_stack_locked(
    parent: &mut ChildSpawnerCommands<'_>,
    index: StackIndex,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
) -> Entity {
    parent
        .spawn((
            StackLockedEntity,
            Mesh3d(meshes.add(Cuboid::new(
                STACK_CELL_WIDTH * 0.85,
                TILE_HEIGHT,
                STACK_CELL_DEPTH * 0.6,
            ))),
            MeshMaterial3d(materials.locked_slot.clone()),
            SCENE_NODE_VISIBILITY,
            Transform::from_translation(stack_insert_marker_center(index)),
            Pickable::IGNORE,
        ))
        .id()
}

fn spawn_one_stack_insert(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    index: StackIndex,
    display_index: StackIndex,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
) -> Entity {
    parent
        .spawn((
            StackInsertEntity,
            BoardPickTarget {
                surface_id: surface,
                kind: BoardPickTargetKind::StackInsert { index },
            },
            Mesh3d(meshes.add(Cuboid::new(
                STACK_INSERT_WIDTH,
                TILE_HEIGHT * 1.4,
                STACK_CELL_DEPTH * 0.55,
            ))),
            MeshMaterial3d(materials.focused.clone()),
            SCENE_NODE_VISIBILITY,
            Transform::from_translation(stack_insert_marker_center(display_index)),
        ))
        .observe(on_tile_clicked)
        .id()
}

fn stack_display_columns(visible: &VisibleBoardState) -> usize {
    if visible.layout != SurfaceLayoutKind::Stack {
        return 0;
    }
    crate::infrastructure::ui::camera_rig::stack_canvas_rows(visible)
        * crate::domain::board::geometry::STACK_COLUMNS
}

fn spawn_surface_base(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    layout: SurfaceLayoutKind,
    cells: usize,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
) {
    if layout != SurfaceLayoutKind::Stack {
        return;
    }
    use crate::domain::board::geometry::{SLOT_SIZE, STACK_COLUMNS, stack_left_edge};
    let columns = STACK_COLUMNS;
    let rows = cells / columns;
    let width = columns as f32 * SLOT_SIZE;
    let depth = rows as f32 * SLOT_SIZE;
    let center_z = (rows as f32 - 1.0) * SLOT_SIZE * 0.5;
    parent
        .spawn((
            BoardDragSurface { surface },
            Mesh3d(meshes.add(Cuboid::new(width, BOARD_THICKNESS, depth))),
            MeshMaterial3d(materials.board.clone()),
            SCENE_NODE_VISIBILITY,
            Transform::from_xyz(0.0, -BOARD_THICKNESS * 0.5, center_z),
            Pickable::default(),
        ))
        .observe(on_board_base_clicked);
    let column_line = meshes.add(Cuboid::new(0.015, 0.008, depth));
    for column in 0..=columns {
        parent.spawn((
            Mesh3d(column_line.clone()),
            MeshMaterial3d(materials.grid_line.clone()),
            SCENE_NODE_VISIBILITY,
            Transform::from_xyz(
                stack_left_edge() + column as f32 * SLOT_SIZE,
                0.007,
                center_z,
            ),
            Pickable::IGNORE,
        ));
    }
    let row_line = meshes.add(Cuboid::new(width, 0.008, 0.015));
    for row in 0..=rows {
        parent.spawn((
            Mesh3d(row_line.clone()),
            MeshMaterial3d(materials.grid_line.clone()),
            SCENE_NODE_VISIBILITY,
            Transform::from_xyz(0.0, 0.007, (row as f32 - 0.5) * SLOT_SIZE),
            Pickable::IGNORE,
        ));
    }
}

fn spawn_one_tile(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    node: &VisibleBoardNode,
    display_address: PlacementAddress,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
    theme: &MusaicUiTheme,
) -> Option<Entity> {
    // Empty surface content on an atom denotes a layer already summarized by
    // its compound anchor. Its authored identity remains in scene_sync.
    if node.kind == VisibleNodeKind::Atom
        && matches!(
            node.surface_content,
            crate::application::pipeline::scene_sync::TileSurfaceContent::Empty
        )
    {
        return None;
    }
    let footprint = tessera_footprint_for_node(node);
    let world =
        placement_center_for_address(display_address, footprint, node.kind, 0.92, SLOT_HEIGHT);
    let pick_target = BoardPickTarget {
        surface_id: surface,
        kind: match node.address {
            PlacementAddress::BoardSlot(_) => BoardPickTargetKind::BoardTile {
                tile_id: node.node.clone(),
            },
            PlacementAddress::StackIndex(_) => BoardPickTargetKind::StackTile {
                tile_id: node.node.clone(),
            },
        },
    };
    let label = node
        .surface_content
        .display()
        .unwrap_or_else(|| match node.kind {
            VisibleNodeKind::Output => "Output".into(),
            _ => "Tile".into(),
        });
    let (mesh, details) = super::detail::face(
        meshes,
        node.kind,
        &node.surface_content,
        Vec2::new(footprint.width as f32, footprint.height as f32),
        node.selected,
        node.focused,
        theme,
    );
    Some(
        parent
            .spawn((
                Name::new(label),
                BoardDragSurface { surface },
                Board3dTile {
                    surface,
                    node: node.node.clone(),
                    address: node.address,
                },
                pick_target,
                mesh,
                details,
                MeshMaterial3d(materials.surface.clone()),
                SCENE_NODE_VISIBILITY,
                Transform::from_xyz(world.x, SLOT_HEIGHT + 0.012, world.z),
                Pickable::default(),
            ))
            .observe(on_tile_clicked)
            .observe(on_tile_pressed)
            .with_children(|tile| {
                if let Some(view) = &node.ports {
                    use tessera::prelude::SpatialSide;
                    let half = Vec2::new(footprint.width as f32, footprint.height as f32) * 0.5;
                    let sides = [
                        (
                            SpatialSide::North,
                            Vec3::new(0.0, 0.105, -half.y),
                            0.0,
                            view.north,
                        ),
                        (
                            SpatialSide::East,
                            Vec3::new(half.x, 0.105, 0.0),
                            -std::f32::consts::FRAC_PI_2,
                            view.east,
                        ),
                        (
                            SpatialSide::South,
                            Vec3::new(0.0, 0.105, half.y),
                            std::f32::consts::PI,
                            view.south,
                        ),
                        (
                            SpatialSide::West,
                            Vec3::new(-half.x, 0.105, 0.0),
                            std::f32::consts::FRAC_PI_2,
                            view.west,
                        ),
                    ];
                    for (side, mut offset, mut yaw, state) in sides {
                        if state == PortSlotState::None && !node.selected && !node.focused {
                            continue;
                        }
                        let port_mesh = if node.kind == VisibleNodeKind::Container
                            && side == SpatialSide::East
                        {
                            offset.x -= 0.19;
                            offset.y = 0.15;
                            yaw = 0.0;
                            crate::infrastructure::ui::tile_surface::compact_container_port_mesh(
                                state,
                                &node.surface_content,
                                theme,
                            )
                        } else {
                            compact_port_mesh(state, theme)
                        };
                        let full = meshes.add(port_mesh);
                        let coarse = if node.kind == VisibleNodeKind::Container
                            && side == SpatialSide::East
                        {
                            meshes.add(crate::infrastructure::ui::tile_surface::simplified_container_port_mesh(state, theme))
                        } else {
                            full.clone()
                        };
                        tile.spawn((
                            super::detail::DetailMeshes::port(full.clone(), coarse),
                            Name::new(format!("{side:?} {} port", state.label())),
                            BoardPickTarget {
                                surface_id: surface,
                                kind: BoardPickTargetKind::PortSide {
                                    tile_id: node.node.clone(),
                                    side,
                                },
                            },
                            Mesh3d(full),
                            MeshMaterial3d(materials.surface.clone()),
                            SCENE_NODE_VISIBILITY,
                            Transform::from_translation(offset)
                                .with_rotation(Quat::from_rotation_y(yaw)),
                            Pickable::default(),
                        ))
                        .observe(crate::infrastructure::ui::port_glyphs::on_port_side_clicked);
                    }
                }
            })
            .id(),
    )
}

fn visible_tile_face(node: &VisibleBoardNode) -> bool {
    node.kind != VisibleNodeKind::Atom
        || !matches!(
            node.surface_content,
            crate::application::pipeline::scene_sync::TileSurfaceContent::Empty
        )
}
