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

use crate::adapter::load_up::UiSpriteAssets;
use crate::application::editor::interaction::BoardPickTargetKind;
use crate::application::pipeline::scene_sync::{
    VisibleBoardConnection, VisibleBoardNode, VisibleBoardState, VisibleNodeKind,
};
use crate::application::session::MusaicProject;
use crate::domain::board::{BoardSurfaceId, STACK_DEPTH, SurfaceLayoutKind};
use crate::domain::document::{PlacementAddress, StackIndex};
use crate::infrastructure::ui::board_geometry::{
    placement_center_for_address, stack_flat_tile_center,
};
use crate::infrastructure::ui::musaic_tile::{
    TILE_HEIGHT, board_footprint_for_kind, stack_footprint_for_kind, tile_visual_for_visible,
};
use crate::infrastructure::ui::port_glyphs::spawn_board_port_compass;
use crate::infrastructure::ui::render_layers::{BOARD_VIEW, SCENE_NODE_VISIBILITY, SCENE_ROOT_VISIBILITY};
use crate::infrastructure::ui::tile_icons::{
    ICON_FOOTPRINT, ICON_THICKNESS, ICON_Y_LIFT, TileIconAssets, spawn_tile_icon,
};
use crate::infrastructure::ui::tile_mesh::TileMeshAssets;
use crate::infrastructure::ui::tile_visual::{
    BoardTileMode, BoardTileSpec, BoardTileVisualSource, resolve_board_tile,
};
use crate::infrastructure::ui::transform_tile::{
    TransformTileAssets, TransformTileBase, TransformTileIcon, frames, tile_sheet_for_atom,
};

use super::components::*;
use super::grid::spawn_board_grid_anchor;
use super::helpers::*;
use super::materials::{Board3dMaterials, Board3dRenderCache};
use super::picking::{on_board_base_clicked, on_tile_clicked};

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
    project: Res<'_, MusaicProject>,
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
        project.document.root_surface,
    );

    // Initial pose mirrors the rig's default; from here on the rig's
    // smoother is the only writer of this Transform/Projection.
    let rig_default = crate::infrastructure::ui::camera_rig::BoardCameraRig::default();
    commands.spawn((
        Board3dCamera,
        Camera3d::default(),
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
    transform_tiles: Res<TransformTileAssets>,
    tile_icons: Res<TileIconAssets>,
    tile_meshes: Option<Res<TileMeshAssets>>,
    atom_tiles: Option<Res<crate::adapter::load_up::AtomTileAssets>>,
    ui_sprites: Option<Res<UiSpriteAssets>>,
    images: Res<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    roots: Query<Entity, With<Board3dRoot>>,
) {
    let Some(surface) = visible.active_surface else {
        return;
    };

    let ortho_ready = transform_tiles.ready;
    let icons_ready = tile_icons.ready;
    let tile_meshes_ready = tile_meshes.as_ref().is_some_and(|assets| assets.ready);

    let structure_changed = cache.active_surface != Some(surface)
        || cache.layout != visible.layout
        || cache.ortho_tiles_ready != ortho_ready
        || cache.tile_icons_ready != icons_ready
        || cache.tile_meshes_ready != tile_meshes_ready
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
            ortho_tiles_ready: ortho_ready,
            tile_icons_ready: icons_ready,
            tile_meshes_ready,
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
            spawn_surface_base(parent, surface, visible.layout, &mut meshes, &materials);
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
        &mut standard_materials,
        &materials,
        &transform_tiles,
        &tile_icons,
        tile_meshes.as_deref(),
        atom_tiles.as_deref(),
        ui_sprites.as_deref(),
        &images,
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
    standard_materials: &mut Assets<StandardMaterial>,
    materials: &Board3dMaterials,
    transform_tiles: &TransformTileAssets,
    tile_icons: &TileIconAssets,
    tile_meshes: Option<&TileMeshAssets>,
    atom_tiles: Option<&crate::adapter::load_up::AtomTileAssets>,
    ui_sprites: Option<&UiSpriteAssets>,
    images: &Assets<Image>,
) {
    let desired: BTreeMap<NodeId, TileVisualKey> = visible
        .nodes
        .iter()
        .map(|n| (n.node.clone(), TileVisualKey::from_node(n)))
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

    for node in &visible.nodes {
        let key = TileVisualKey::from_node(node);
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
                meshes,
                standard_materials,
                materials,
                transform_tiles,
                tile_icons,
                tile_meshes,
                atom_tiles,
                ui_sprites,
                images,
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
            spawned = spawn_one_connection(parent, surface, connection, meshes, materials);
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
        if cache.stack_inserts.contains_key(index) {
            continue;
        }
        let mut spawned = None;
        commands.entity(root).with_children(|parent| {
            spawned = Some(spawn_one_stack_insert(
                parent, surface, *index, meshes, materials,
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
        if cache.stack_locked.contains_key(index) {
            continue;
        }
        let mut spawned = None;
        commands.entity(root).with_children(|parent| {
            spawned = Some(spawn_one_stack_locked(parent, *index, meshes, materials));
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
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
) -> Option<Entity> {
    let start = slot_position(connection.from_slot, CONNECTION_LIFT);
    let end = slot_position(connection.to_slot, CONNECTION_LIFT);
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return None;
    }

    let mid = start.lerp(end, 0.5);
    let yaw = delta.z.atan2(delta.x);
    use crate::domain::document::PortSlotState;
    let material = if connection.kind == PortSlotState::Input {
        materials.connection_scalar.clone()
    } else {
        materials.connection_control.clone()
    };
    Some(
        parent
            .spawn((
                ConnectionLineEntity,
                BoardPickTarget {
                    surface_id: surface,
                    kind: BoardPickTargetKind::Connection {
                        from: connection.from.clone(),
                        to: connection.to.clone(),
                    },
                },
                Mesh3d(meshes.add(Cuboid::new(length, CONNECTION_HEIGHT, CONNECTION_THICKNESS))),
                MeshMaterial3d(material),
                SCENE_NODE_VISIBILITY,
                Transform {
                    translation: mid,
                    rotation: Quat::from_rotation_y(-yaw),
                    ..default()
                },
            ))
            .observe(on_tile_clicked)
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
            Transform::from_translation(stack_insert_marker_center(index)),
        ))
        .observe(on_tile_clicked)
        .id()
}

fn spawn_surface_base(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    layout: SurfaceLayoutKind,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
) {
    match layout {
        // Root boards use the persistent BoardGridAnchor backdrop instead of a
        // per-rebuild slab; container interiors stay a finite stack track.
        SurfaceLayoutKind::Board => {}
        SurfaceLayoutKind::Stack => spawn_stack_base(parent, surface, meshes, materials),
    }
}

fn spawn_stack_base(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    meshes: &mut Assets<Mesh>,
    materials: &Board3dMaterials,
) {
    let width = stack_width();

    parent
        .spawn((
            BoardDragSurface { surface },
            Mesh3d(meshes.add(Cuboid::new(width, BOARD_THICKNESS, STACK_DEPTH))),
            MeshMaterial3d(materials.board.clone()),
            SCENE_NODE_VISIBILITY,
            Transform::from_xyz(0.0, -BOARD_THICKNESS * 0.5, 0.0),
            Pickable::default(),
        ))
        .observe(on_board_base_clicked);
}

fn spawn_atom_center_icon(
    parent: &mut ChildSpawnerCommands<'_>,
    tile_icons: &TileIconAssets,
    materials: &mut Assets<StandardMaterial>,
    texture: Handle<Image>,
) {
    if !tile_icons.ready {
        return;
    }
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(texture),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });
    parent.spawn((
        TransformTileIcon,
        Mesh3d(tile_icons.quad.clone()),
        MeshMaterial3d(material),
        SCENE_NODE_VISIBILITY,
        Transform::from_translation(Vec3::new(
            0.0,
            ICON_Y_LIFT + crate::infrastructure::ui::musaic_tile::TILE_WORLD_HEIGHT * 0.5,
            0.0,
        ))
        .with_scale(Vec3::new(ICON_FOOTPRINT, ICON_THICKNESS, ICON_FOOTPRINT)),
    ));
}

fn spawn_one_tile(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    node: &VisibleBoardNode,
    meshes: &mut Assets<Mesh>,
    standard_materials: &mut Assets<StandardMaterial>,
    materials: &Board3dMaterials,
    transform_tiles: &TransformTileAssets,
    tile_icons: &TileIconAssets,
    tile_meshes: Option<&TileMeshAssets>,
    atom_tiles: Option<&crate::adapter::load_up::AtomTileAssets>,
    ui_sprites: Option<&UiSpriteAssets>,
    images: &Assets<Image>,
) -> Option<Entity> {
    match node.address {
        PlacementAddress::StackIndex(index) => spawn_one_stack_tile(
            parent,
            surface,
            node,
            index,
            meshes,
            standard_materials,
            materials,
            transform_tiles,
            tile_icons,
            tile_meshes,
            atom_tiles,
        ),
        PlacementAddress::BoardSlot(_) => spawn_one_board_tile(
            parent,
            surface,
            node,
            meshes,
            standard_materials,
            materials,
            transform_tiles,
            tile_icons,
            tile_meshes,
            atom_tiles,
            ui_sprites,
            images,
        ),
    }
}

fn spawn_one_board_tile(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    node: &VisibleBoardNode,
    meshes: &mut Assets<Mesh>,
    standard_materials: &mut Assets<StandardMaterial>,
    materials: &Board3dMaterials,
    transform_tiles: &TransformTileAssets,
    tile_icons: &TileIconAssets,
    tile_meshes: Option<&TileMeshAssets>,
    atom_tiles: Option<&crate::adapter::load_up::AtomTileAssets>,
    ui_sprites: Option<&UiSpriteAssets>,
    images: &Assets<Image>,
) -> Option<Entity> {
    let footprint = board_footprint_for_kind(node.kind);
    let tessera_footprint = tessera_footprint_for_node(node);
    let world = placement_center_for_address(
        node.address,
        tessera_footprint,
        node.kind,
        footprint,
        SLOT_HEIGHT,
    );
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

    let ortho = board_tile_ortho_spec(node, footprint);
    let fallback = tile_material(node.kind, node.selected, node.focused, materials);
    let spec = BoardTileSpec {
        kind: node.kind,
        tessera_footprint,
        on_root_board: true,
        plane_anchor: Vec3::new(world.x, SLOT_HEIGHT, world.z),
        visual_footprint: footprint,
        ortho,
    };
    let visual = resolve_board_tile(
        tile_meshes,
        Some(transform_tiles),
        meshes,
        standard_materials,
        Some(&fallback),
        &spec,
        BoardTileMode::Placed,
    )?;
    let ortho_icon = match visual.source {
        BoardTileVisualSource::Ortho => spec.ortho.as_ref().and_then(|o| o.icon),
        _ => None,
    };

    let mut entity = parent.spawn((
        BoardDragSurface { surface },
        Board3dTile {
            surface,
            node: node.node.clone(),
            address: node.address,
        },
        pick_target,
        Mesh3d(visual.mesh),
        MeshMaterial3d(visual.material),
        SCENE_NODE_VISIBILITY,
        visual.transform,
        Pickable::default(),
    ));
    if visual.source == BoardTileVisualSource::Ortho {
        entity.insert(TransformTileBase);
    }
    Some(
        entity
            .observe(on_tile_clicked)
            .with_children(|tile| {
                if let Some(icon) = ortho_icon {
                    tile.spawn(TransformTileIcon);
                    spawn_tile_icon(tile, tile_icons, icon, ());
                } else if let (Some(atom_assets), Some(atom)) = (atom_tiles, node.atom.as_ref()) {
                    if let Some(texture) = atom_assets.image_for_atom(atom).cloned() {
                        spawn_atom_center_icon(tile, tile_icons, standard_materials, texture);
                    }
                }
                if let (Some(sprites), Some(view)) = (ui_sprites, node.ports.as_ref()) {
                    spawn_board_port_compass(
                        tile,
                        surface,
                        &node.node,
                        footprint,
                        view,
                        images,
                        sprites,
                        meshes,
                        standard_materials,
                    );
                }
            })
            .id(),
    )
}

fn spawn_one_stack_tile(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    node: &VisibleBoardNode,
    index: StackIndex,
    meshes: &mut Assets<Mesh>,
    standard_materials: &mut Assets<StandardMaterial>,
    materials: &Board3dMaterials,
    transform_tiles: &TransformTileAssets,
    tile_icons: &TileIconAssets,
    tile_meshes: Option<&TileMeshAssets>,
    atom_tiles: Option<&crate::adapter::load_up::AtomTileAssets>,
) -> Option<Entity> {
    let footprint = stack_footprint_for_kind(node.kind);
    let tessera_footprint = tessera_footprint_for_node(node);
    let center = stack_flat_tile_center(index, node.kind, SLOT_HEIGHT);
    let pick_target = BoardPickTarget {
        surface_id: surface,
        kind: BoardPickTargetKind::StackTile {
            tile_id: node.node.clone(),
        },
    };

    let ortho = board_tile_ortho_spec(node, footprint);
    let fallback = tile_material(node.kind, node.selected, node.focused, materials);
    let spec = BoardTileSpec {
        kind: node.kind,
        tessera_footprint,
        on_root_board: false,
        plane_anchor: Vec3::new(center.x, SLOT_HEIGHT, center.z),
        visual_footprint: footprint,
        ortho,
    };
    let visual = resolve_board_tile(
        tile_meshes,
        Some(transform_tiles),
        meshes,
        standard_materials,
        Some(&fallback),
        &spec,
        BoardTileMode::Placed,
    )?;
    let ortho_icon = match visual.source {
        BoardTileVisualSource::Ortho => spec.ortho.as_ref().and_then(|o| o.icon),
        _ => None,
    };

    let mut entity = parent.spawn((
        BoardDragSurface { surface },
        Board3dTile {
            surface,
            node: node.node.clone(),
            address: node.address,
        },
        pick_target,
        Mesh3d(visual.mesh),
        MeshMaterial3d(visual.material),
        SCENE_NODE_VISIBILITY,
        visual.transform,
        Pickable::default(),
    ));
    if visual.source == BoardTileVisualSource::Ortho {
        entity.insert(TransformTileBase);
    }
    Some(
        entity
            .observe(on_tile_clicked)
            .with_children(|tile| {
                if let Some(icon) = ortho_icon {
                    tile.spawn(TransformTileIcon);
                    spawn_tile_icon(tile, tile_icons, icon, ());
                } else if let (Some(atom_assets), Some(atom)) = (atom_tiles, node.atom.as_ref()) {
                    if let Some(texture) = atom_assets.image_for_atom(atom).cloned() {
                        spawn_atom_center_icon(tile, tile_icons, standard_materials, texture);
                    }
                }
            })
            .id(),
    )
}

fn board_tile_ortho_spec(
    node: &VisibleBoardNode,
    footprint: f32,
) -> Option<crate::infrastructure::ui::musaic_tile::TileVisualSpec> {
    let atom_sheet = node
        .atom
        .as_ref()
        .filter(|_| node.kind == VisibleNodeKind::Atom)
        .map(tile_sheet_for_atom);

    let mut spec = atom_sheet
        .map(|sheet| {
            let highlighted = node.selected || node.focused;
            crate::infrastructure::ui::musaic_tile::TileVisualSpec {
                sheet,
                frame: if highlighted {
                    frames::HIGHLIGHT
                } else {
                    frames::FRAMED
                },
                footprint,
                icon: None,
            }
        })
        .or_else(|| tile_visual_for_visible(node.kind, node.selected, node.focused, node.icon))?;
    spec.footprint = footprint;
    Some(spec)
}

fn tile_material(
    kind: VisibleNodeKind,
    selected: bool,
    focused: bool,
    materials: &Board3dMaterials,
) -> Handle<StandardMaterial> {
    if focused {
        return materials.focused.clone();
    }

    if selected {
        return materials.selected.clone();
    }

    match kind {
        VisibleNodeKind::Container => materials.container.clone(),
        VisibleNodeKind::Atom => materials.atom.clone(),
        VisibleNodeKind::Output => materials.output.clone(),
        VisibleNodeKind::TrickInstance => materials.trick.clone(),
        VisibleNodeKind::Tile => materials.generic.clone(),
    }
}

