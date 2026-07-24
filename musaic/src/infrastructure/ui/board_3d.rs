use std::collections::{BTreeMap, BTreeSet};

use crate::infrastructure::app::{AppState, MusaicSet};
use bevy::{
    asset::RenderAssetUsages,
    camera::ScalingMode,
    math::primitives::Cuboid,
    picking::prelude::*,
    prelude::*,
    render::render_resource::{TextureDimension, TextureFormat, TextureUsages},
    state::condition::in_state,
};
use tessera::prelude::NodeId;

use crate::{
    adapter::load_up::UiSpriteAssets,
    application::editor::interaction::{BoardPickEvent, BoardPickHit, BoardPickTargetKind},
    application::editor::{
        BoardCursorHover, BoardPlacementPointer, CursorInteraction, EditorSession, MusaicEditorSet,
        PickHit, blocks_board_picks_with_cursor, connection_endpoint_view,
    },
    application::pipeline::scene_sync::{
        VisibleBoardConnection, VisibleBoardNode, VisibleBoardState, VisibleNodeKind,
    },
    application::session::MusaicProject,
    domain::board::{
        BoardSlot, BoardSurfaceId, SLOT_SIZE, STACK_DEPTH, STACK_LENGTH, SurfaceLayoutKind,
        geometry::BOARD_PLANE_Y,
    },
    domain::document::{DocumentQueries, PlacementAddress, StackIndex},
    infrastructure::ui::{
        board_geometry::{
            placement_center_for_address, stack_column_center, stack_flat_tile_center,
            tessera_slot_center,
        },
        musaic_tile::{
            board_footprint_for_kind, stack_footprint_for_kind, tile_center_y_for_footprint,
            tile_scale_for_footprint, tile_visual_for_visible,
        },
        placement_preview::{resolve_placement_preview_center, tessera_footprint_for_preview},
        port_glyphs::spawn_board_port_compass,
        render_layers::{BOARD_VIEW, SCENE_NODE_VISIBILITY, SCENE_ROOT_VISIBILITY},
        tile_icons::{
            ICON_FOOTPRINT, ICON_THICKNESS, ICON_Y_LIFT, TileIconAssets, spawn_tile_icon,
        },
        tile_mesh::TileMeshAssets,
        tile_visual::{spawn_gltf_tile_mesh, translucent_preview_material, visible_kind_for_spawn},
        transform_tile::{
            TransformTileAssets, TransformTileBase, TransformTileIcon, frames, tile_sheet_for_atom,
        },
    },
};

pub struct Board3dPlugin;

impl Plugin for Board3dPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Board3dMaterials>()
            .init_resource::<Board3dRenderCache>()
            .add_systems(OnEnter(AppState::Editor), setup_board_3d_scene)
            .add_systems(OnExit(AppState::Editor), teardown_board_3d_scene)
            .add_systems(
                Update,
                sync_board_3d_scene
                    .after(MusaicSet::SceneSync)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                sample_board_placement_pointer
                    .before(MusaicEditorSet::MutateState)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                (
                    sync_board_cursor_hover,
                    // Grid backdrop follows the smoothed camera (read-only) so
                    // the slab always covers the view; runs after the rig's
                    // single transform writer.
                    sync_board_grid_anchor.after(super::camera_rig::CameraRigSet::Smooth),
                    sync_drag_preview,
                    sync_connection_preview,
                    sync_placement_slot_highlight,
                    animate_placement_pulse,
                )
                    .after(MusaicEditorSet::MutateState)
                    .after(MusaicSet::SceneSync)
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

const SLOT_HEIGHT: f32 = BOARD_PLANE_Y;
const STACK_CELL_WIDTH: f32 = SLOT_SIZE - 0.18;
const STACK_CELL_DEPTH: f32 = STACK_DEPTH - 0.36;
const STACK_INSERT_WIDTH: f32 = 0.10;

const TILE_HEIGHT: f32 = 0.22;
const TILE_INSET: f32 = 0.18;
const CONNECTION_THICKNESS: f32 = 0.05;
const CONNECTION_HEIGHT: f32 = 0.12;
const CONNECTION_LIFT: f32 = SLOT_HEIGHT + 0.06;

const BOARD_THICKNESS: f32 = 0.02;
const STACK_MARGIN: f32 = 0.18;

#[derive(Component)]
pub struct Board3dRoot;

/// Persistent infinite-board backdrop: pick slab + grid lines that follow the
/// camera in `SLOT_SIZE` snaps so the root board reads as an endless surface.
#[derive(Component)]
pub struct BoardGridAnchor;

/// The large pickable plane under the grid (click → slot resolution).
#[derive(Component)]
pub struct BoardGridPickSlab;

#[derive(Component)]
struct Board3dLight;

#[derive(Component, Clone, Copy)]
struct BoardDragSurface {
    surface: BoardSurfaceId,
}

#[derive(Component)]
pub struct Board3dCamera;

pub type BoardTileId = NodeId;

#[derive(Component, Clone, Debug)]
pub struct BoardPickTarget {
    pub surface_id: BoardSurfaceId,
    pub kind: BoardPickTargetKind,
}

#[derive(Component)]
pub struct Board3dTile {
    pub surface: BoardSurfaceId,
    pub node: NodeId,
    pub address: PlacementAddress,
}

#[derive(Component)]
struct BoardDragPreview;

#[derive(Component)]
struct BoardConnectionPreview;

#[derive(Component)]
struct BoardSlotHighlight;

#[derive(Component)]
struct TilePlacementPulse {
    elapsed: f32,
    base_scale: Vec3,
}

#[derive(Component)]
struct StackInsertEntity;

#[derive(Component)]
struct StackLockedEntity;

#[derive(Component)]
struct ConnectionLineEntity;

/// Per-tile visual identity for keyed board reconcile.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TileVisualKey {
    address: PlacementAddress,
    kind: VisibleNodeKind,
    selected: bool,
    focused: bool,
    footprint: Option<(u32, u32)>,
    surface_display: Option<String>,
    icon: Option<crate::adapter::tile_icons::TileIconId>,
}

impl TileVisualKey {
    fn from_node(node: &VisibleBoardNode) -> Self {
        Self {
            address: node.address,
            kind: node.kind,
            selected: node.selected,
            focused: node.focused,
            footprint: node.tessera_footprint.map(|f| (f.width, f.height)),
            surface_display: node.surface_content.display(),
            icon: node.icon,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConnectionVisualKey {
    from_slot: BoardSlot,
    to_slot: BoardSlot,
    kind: crate::domain::document::PortSlotState,
}

#[derive(Resource, Default)]
struct Board3dRenderCache {
    root: Option<Entity>,
    active_surface: Option<BoardSurfaceId>,
    layout: SurfaceLayoutKind,
    ortho_tiles_ready: bool,
    tile_icons_ready: bool,
    tile_meshes_ready: bool,
    tiles: BTreeMap<NodeId, (Entity, TileVisualKey)>,
    connections: BTreeMap<(NodeId, NodeId), (Entity, ConnectionVisualKey)>,
    stack_inserts: BTreeMap<StackIndex, Entity>,
    stack_locked: BTreeMap<StackIndex, Entity>,
}

#[derive(Resource)]
struct Board3dMaterials {
    board: Handle<StandardMaterial>,
    grid_line: Handle<StandardMaterial>,
    container: Handle<StandardMaterial>,
    atom: Handle<StandardMaterial>,
    output: Handle<StandardMaterial>,
    trick: Handle<StandardMaterial>,
    generic: Handle<StandardMaterial>,
    selected: Handle<StandardMaterial>,
    focused: Handle<StandardMaterial>,
    locked_slot: Handle<StandardMaterial>,
    connection_scalar: Handle<StandardMaterial>,
    connection_control: Handle<StandardMaterial>,
    connection_preview: Handle<StandardMaterial>,
    drag_preview: Handle<StandardMaterial>,
}

impl FromWorld for Board3dMaterials {
    fn from_world(world: &mut World) -> Self {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();

        Self {
            board: materials.add(StandardMaterial {
                base_color: Color::srgb(0.10, 0.105, 0.13),
                perceptual_roughness: 1.0,
                ..default()
            }),
            grid_line: materials.add(StandardMaterial {
                base_color: Color::srgb(0.28, 0.31, 0.38),
                emissive: LinearRgba::rgb(0.005, 0.006, 0.01),
                perceptual_roughness: 0.95,
                ..default()
            }),
            container: materials.add(StandardMaterial {
                base_color: Color::srgb(0.29, 0.36, 0.52),
                perceptual_roughness: 0.85,
                ..default()
            }),
            atom: materials.add(StandardMaterial {
                base_color: Color::srgb(0.22, 0.48, 0.34),
                perceptual_roughness: 0.85,
                ..default()
            }),
            output: materials.add(StandardMaterial {
                base_color: Color::srgb(0.61, 0.42, 0.18),
                perceptual_roughness: 0.85,
                ..default()
            }),
            trick: materials.add(StandardMaterial {
                base_color: Color::srgb(0.45, 0.32, 0.60),
                perceptual_roughness: 0.85,
                ..default()
            }),
            generic: materials.add(StandardMaterial {
                base_color: Color::srgb(0.38, 0.40, 0.46),
                perceptual_roughness: 0.85,
                ..default()
            }),
            selected: materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.78, 0.22),
                emissive: LinearRgba::rgb(0.18, 0.12, 0.02),
                perceptual_roughness: 0.8,
                ..default()
            }),
            focused: materials.add(StandardMaterial {
                base_color: Color::srgb(0.18, 0.75, 0.85),
                emissive: LinearRgba::rgb(0.02, 0.12, 0.16),
                perceptual_roughness: 0.8,
                ..default()
            }),
            locked_slot: materials.add(StandardMaterial {
                base_color: Color::srgb(0.05, 0.055, 0.07),
                perceptual_roughness: 1.0,
                ..default()
            }),
            connection_scalar: materials.add(StandardMaterial {
                base_color: Color::srgb(0.20, 0.78, 0.92),
                emissive: LinearRgba::rgb(0.04, 0.14, 0.18),
                perceptual_roughness: 0.85,
                ..default()
            }),
            connection_control: materials.add(StandardMaterial {
                base_color: Color::srgb(0.95, 0.55, 0.18),
                emissive: LinearRgba::rgb(0.16, 0.08, 0.02),
                perceptual_roughness: 0.85,
                ..default()
            }),
            connection_preview: materials.add(StandardMaterial {
                base_color: Color::srgba(0.55, 0.85, 1.0, 0.45),
                alpha_mode: AlphaMode::Blend,
                emissive: LinearRgba::rgb(0.05, 0.13, 0.2),
                perceptual_roughness: 0.7,
                ..default()
            }),
            drag_preview: materials.add(StandardMaterial {
                base_color: Color::srgba(0.86, 0.90, 0.98, 0.55),
                alpha_mode: AlphaMode::Blend,
                emissive: LinearRgba::rgb(0.04, 0.05, 0.08),
                perceptual_roughness: 0.75,
                ..default()
            }),
        }
    }
}

fn teardown_board_3d_scene(
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

fn setup_board_3d_scene(
    mut commands: Commands,
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
    let rig_default = super::camera_rig::BoardCameraRig::default();
    commands.spawn((
        Board3dCamera,
        Camera3d::default(),
        MeshPickingCamera,
        BOARD_VIEW,
        SCENE_ROOT_VISIBILITY,
        Camera {
            order: -1,
            clear_color: ClearColorConfig::Custom(Color::srgb(0.11, 0.12, 0.15)),
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

fn sync_board_3d_scene(
    mut commands: Commands,
    project: Res<MusaicProject>,
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

    let queries = DocumentQueries::new(&project.document);
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
        &queries,
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
    queries: &DocumentQueries<'_>,
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
                queries,
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

const GRID_LINE_WIDTH: f32 = 0.03;
const GRID_LINE_LIFT: f32 = 0.005;
/// Half extent (in slots) of the camera-following grid backdrop. Must exceed
/// the widest camera framing so the board plane never shows an edge.
const GRID_HALF_SLOTS: i32 = 48;

fn spawn_board_grid_anchor(
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
fn sync_board_grid_anchor(
    visible: Res<'_, VisibleBoardState>,
    camera: Query<'_, '_, &Transform, (With<Board3dCamera>, Without<BoardGridAnchor>)>,
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

    let Ok(camera_transform) = camera.single() else {
        return;
    };
    let Some(focus) = camera_ground_focus(camera_transform) else {
        return;
    };
    // Slot boundaries sit at integer multiples of SLOT_SIZE offset from the
    // board origin; snapping the anchor to that lattice keeps the (locally
    // centered) grid lines world-aligned while following the camera.
    let origin = -crate::domain::board::board_width() * 0.5;
    let snap = |value: f32| origin + ((value - origin) / SLOT_SIZE).round() * SLOT_SIZE;
    anchor_transform.translation.x = snap(focus.x);
    anchor_transform.translation.z = snap(focus.z);
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
    queries: &DocumentQueries<'_>,
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
            queries,
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
            queries,
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
    queries: &DocumentQueries<'_>,
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

    if let Some(entity) = try_spawn_gltf_tile(
        parent,
        surface,
        node,
        pick_target.clone(),
        world,
        tessera_footprint,
        true,
        tile_meshes,
        queries,
        tile_icons,
        standard_materials,
        atom_tiles,
        ui_sprites,
        Some(images),
        meshes,
    ) {
        return Some(entity);
    }

    let atom_sheet = (node.kind == VisibleNodeKind::Atom)
        .then(|| {
            queries.node_kind(&node.node).and_then(|kind| match kind {
                crate::domain::document::DocumentNodeKind::Atom(atom) => {
                    Some(tile_sheet_for_atom(&atom.atom))
                }
                _ => None,
            })
        })
        .flatten();

    if transform_tiles.ready
        && let Some(mut spec) = atom_sheet
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
            .or_else(|| tile_visual_for_visible(node.kind, node.selected, node.focused, node.icon))
    {
        spec.footprint = footprint;
        let center = placement_center_for_address(
            node.address,
            tessera_footprint,
            node.kind,
            footprint,
            SLOT_HEIGHT,
        );
        let mesh = transform_tiles.meshes.framed.clone();
        let sheet_materials = transform_tiles.materials(spec.sheet);
        let material = match spec.frame {
            frames::PLAIN => sheet_materials.plain.clone(),
            frames::HIGHLIGHT => sheet_materials.highlight.clone(),
            _ => sheet_materials.framed.clone(),
        };
        return Some(
            parent
                .spawn((
                    BoardDragSurface { surface },
                    Board3dTile {
                        surface,
                        node: node.node.clone(),
                        address: node.address,
                    },
                    pick_target,
                    TransformTileBase,
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    SCENE_NODE_VISIBILITY,
                    Transform::from_translation(center)
                        .with_scale(tile_scale_for_footprint(footprint)),
                    Pickable::default(),
                ))
                .observe(on_tile_clicked)
                .with_children(|tile| {
                    if let Some(icon) = spec.icon {
                        tile.spawn(TransformTileIcon);
                        spawn_tile_icon(tile, tile_icons, icon, ());
                    } else if let Some(atom_assets) = atom_tiles {
                        if let Some(crate::domain::document::DocumentNodeKind::Atom(atom)) =
                            queries.node_kind(&node.node)
                        {
                            if let Some(texture) = atom_assets.image_for_atom(&atom.atom).cloned() {
                                spawn_atom_center_icon(
                                    tile,
                                    tile_icons,
                                    standard_materials,
                                    texture,
                                );
                            }
                        }
                    }
                    if let (PlacementAddress::BoardSlot(slot), Some(sprites)) =
                        (node.address, ui_sprites)
                        && let Some(view) =
                            connection_endpoint_view(queries, &node.node, Some(slot), None)
                    {
                        spawn_board_port_compass(
                            tile,
                            surface,
                            &node.node,
                            footprint,
                            &view,
                            images,
                            sprites,
                            meshes,
                            standard_materials,
                        );
                    }
                })
                .id(),
        );
    }

    let material = tile_material(node.kind, node.selected, node.focused, materials);
    Some(
        parent
            .spawn((
                BoardDragSurface { surface },
                pick_target,
                Board3dTile {
                    surface,
                    node: node.node.clone(),
                    address: node.address,
                },
                Mesh3d(meshes.add(Cuboid::new(footprint, TILE_HEIGHT, footprint))),
                MeshMaterial3d(material),
                SCENE_NODE_VISIBILITY,
                Transform::from_translation(world),
                Pickable::default(),
            ))
            .observe(on_tile_clicked)
            .with_children(|tile| {
                if let (PlacementAddress::BoardSlot(slot), Some(sprites)) =
                    (node.address, ui_sprites)
                    && let Some(view) =
                        connection_endpoint_view(queries, &node.node, Some(slot), None)
                {
                    spawn_board_port_compass(
                        tile,
                        surface,
                        &node.node,
                        footprint,
                        &view,
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
    queries: &DocumentQueries<'_>,
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

    if let Some(entity) = try_spawn_gltf_tile(
        parent,
        surface,
        node,
        pick_target.clone(),
        center,
        tessera_footprint,
        false,
        tile_meshes,
        queries,
        tile_icons,
        standard_materials,
        atom_tiles,
        None,
        None,
        meshes,
    ) {
        return Some(entity);
    }

    let atom_sheet = (node.kind == VisibleNodeKind::Atom)
        .then(|| {
            queries.node_kind(&node.node).and_then(|kind| match kind {
                crate::domain::document::DocumentNodeKind::Atom(atom) => {
                    Some(tile_sheet_for_atom(&atom.atom))
                }
                _ => None,
            })
        })
        .flatten();

    if transform_tiles.ready
        && let Some(mut spec) = atom_sheet
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
            .or_else(|| tile_visual_for_visible(node.kind, node.selected, node.focused, node.icon))
    {
        spec.footprint = footprint;
        let sheet_materials = transform_tiles.materials(spec.sheet);
        let material = match spec.frame {
            frames::PLAIN => sheet_materials.plain.clone(),
            frames::HIGHLIGHT => sheet_materials.highlight.clone(),
            _ => sheet_materials.framed.clone(),
        };
        return Some(
            parent
                .spawn((
                    BoardDragSurface { surface },
                    Board3dTile {
                        surface,
                        node: node.node.clone(),
                        address: node.address,
                    },
                    pick_target,
                    TransformTileBase,
                    Mesh3d(transform_tiles.meshes.framed.clone()),
                    MeshMaterial3d(material),
                    SCENE_NODE_VISIBILITY,
                    Transform::from_translation(center)
                        .with_scale(tile_scale_for_footprint(footprint)),
                    Pickable::default(),
                ))
                .observe(on_tile_clicked)
                .with_children(|tile| {
                    if let Some(icon) = spec.icon {
                        tile.spawn(TransformTileIcon);
                        spawn_tile_icon(tile, tile_icons, icon, ());
                    } else if let Some(atom_assets) = atom_tiles {
                        if let Some(crate::domain::document::DocumentNodeKind::Atom(atom)) =
                            queries.node_kind(&node.node)
                        {
                            if let Some(texture) = atom_assets.image_for_atom(&atom.atom).cloned() {
                                spawn_atom_center_icon(
                                    tile,
                                    tile_icons,
                                    standard_materials,
                                    texture,
                                );
                            }
                        }
                    }
                })
                .id(),
        );
    }

    let material = tile_material(node.kind, node.selected, node.focused, materials);
    Some(
        parent
            .spawn((
                BoardDragSurface { surface },
                pick_target,
                Board3dTile {
                    surface,
                    node: node.node.clone(),
                    address: node.address,
                },
                Mesh3d(meshes.add(Cuboid::new(footprint, TILE_HEIGHT, footprint * 0.75))),
                MeshMaterial3d(material),
                SCENE_NODE_VISIBILITY,
                Transform::from_translation(center),
                Pickable::default(),
            ))
            .observe(on_tile_clicked)
            .id(),
    )
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

fn sync_drag_preview(
    mut commands: Commands,
    session: Res<'_, EditorSession>,
    pointer: Res<'_, BoardPlacementPointer>,
    visible: Res<'_, VisibleBoardState>,
    mut meshes: ResMut<'_, Assets<Mesh>>,
    mut standard_materials: ResMut<'_, Assets<StandardMaterial>>,
    materials: Res<'_, Board3dMaterials>,
    tile_meshes: Option<Res<'_, TileMeshAssets>>,
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

    let Some(center) = resolve_placement_preview_center(&session, &pointer, &visible, preview_kind)
    else {
        return;
    };

    let on_root_board = visible.layout == SurfaceLayoutKind::Board;
    let tessera_footprint = tessera_footprint_for_preview(preview_kind, on_root_board);

    let gltf_primitive = tile_meshes
        .as_ref()
        .filter(|assets| assets.ready)
        .and_then(|assets| {
            assets.prototype_for_visible(preview_kind, tessera_footprint, on_root_board)
        })
        .and_then(|prototype| prototype.root.as_ref());

    if let Some(entity) = previews.iter().next() {
        let transform = gltf_primitive
            .map(|primitive| primitive.board_transform(Vec3::new(center.x, SLOT_HEIGHT, center.z)))
            .unwrap_or_else(|| {
                let footprint = if visible.layout == SurfaceLayoutKind::Stack {
                    stack_footprint_for_kind(preview_kind)
                } else {
                    board_footprint_for_kind(preview_kind)
                };
                Transform::from_translation(center).with_scale(tile_scale_for_footprint(footprint))
            });
        commands.entity(entity).insert(transform);
        return;
    }

    if let Some(tile_meshes) = tile_meshes.as_ref().filter(|assets| assets.ready) {
        if let Some(primitive) = gltf_primitive {
            let preview_material =
                translucent_preview_material(&mut standard_materials, &primitive.material);
            spawn_gltf_tile_mesh(
                &mut commands,
                tile_meshes,
                preview_kind,
                tessera_footprint,
                on_root_board,
                Some(preview_material),
                Vec3::new(center.x, SLOT_HEIGHT, center.z),
                (BoardDragPreview, BOARD_VIEW, SCENE_ROOT_VISIBILITY),
            );
            return;
        }
    }

    let footprint = if visible.layout == SurfaceLayoutKind::Stack {
        stack_footprint_for_kind(preview_kind)
    } else {
        board_footprint_for_kind(preview_kind)
    };
    let mesh = if visible.layout == SurfaceLayoutKind::Stack {
        meshes.add(Cuboid::new(footprint, TILE_HEIGHT, footprint * 0.75))
    } else {
        meshes.add(Cuboid::new(
            crate::infrastructure::ui::musaic_tile::TILE_WORLD_WIDTH,
            crate::infrastructure::ui::musaic_tile::TILE_WORLD_HEIGHT,
            crate::infrastructure::ui::musaic_tile::TILE_WORLD_DEPTH,
        ))
    };
    commands.spawn((
        BoardDragPreview,
        BOARD_VIEW,
        Mesh3d(mesh),
        MeshMaterial3d(materials.drag_preview.clone()),
        SCENE_ROOT_VISIBILITY,
        Transform::from_translation(center).with_scale(tile_scale_for_footprint(footprint)),
    ));
}

/// Renders the provisional root-board edge while the connection statechart is active.
fn sync_connection_preview(
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

fn sync_placement_slot_highlight(
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

fn animate_placement_pulse(
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

fn on_tile_clicked(
    mut event: On<'_, '_, Pointer<Click>>,
    session: Res<'_, EditorSession>,
    cursor: Res<'_, CursorInteraction>,
    targets: Query<'_, '_, &BoardPickTarget>,
    mut pick_events: MessageWriter<'_, BoardPickEvent>,
) {
    if blocks_board_picks_with_cursor(&session, cursor.phase()) {
        event.propagate(false);
        return;
    }
    let entity = event.original_event_target();
    let Ok(target) = targets.get(entity) else {
        return;
    };
    let Some(position) = event.hit.position else {
        return;
    };

    pick_events.write(BoardPickEvent::Hit(BoardPickHit {
        surface_id: target.surface_id,
        kind: target.kind.clone(),
        world_position: position,
        distance: 0.0,
    }));
    event.propagate(false);
}

/// Board base clicks route through the board projection (`VisibleBoardState`):
/// empty slots and stack inserts become pick hits; anything else is a miss
/// that clears editor focus. Drag hover/commit are owned by the placement
/// pointer + editor session, never by picking drag events.
fn on_board_base_clicked(
    mut event: On<'_, '_, Pointer<Click>>,
    session: Res<'_, EditorSession>,
    cursor: Res<'_, CursorInteraction>,
    visible: Res<'_, VisibleBoardState>,
    surfaces: Query<'_, '_, &BoardDragSurface>,
    mut pick_events: MessageWriter<'_, BoardPickEvent>,
) {
    if blocks_board_picks_with_cursor(&session, cursor.phase()) {
        event.propagate(false);
        return;
    }
    let entity = event.original_event_target();
    let Ok(base) = surfaces.get(entity) else {
        return;
    };
    let Some(position) = event.hit.position else {
        return;
    };

    let pick = crate::domain::board::geometry::slot_at_world_position(position, visible.layout)
        .and_then(|slot| visible.pick_at(slot))
        .and_then(|hit| match hit {
            PickHit::EmptySlot { slot, .. } => Some(BoardPickTargetKind::Slot { slot }),
            PickHit::StackInsert { index, .. } => Some(BoardPickTargetKind::StackInsert { index }),
            _ => None,
        });

    match pick {
        Some(kind) => {
            pick_events.write(BoardPickEvent::Hit(BoardPickHit {
                surface_id: base.surface,
                kind,
                world_position: position,
                distance: 0.0,
            }));
        }
        None => {
            pick_events.write(BoardPickEvent::Miss);
        }
    }
    event.propagate(false);
}

fn sync_board_cursor_hover(
    pointer: Res<BoardPlacementPointer>,
    visible: Res<VisibleBoardState>,
    session: Res<EditorSession>,
    mut hover: ResMut<BoardCursorHover>,
) {
    hover.pickable = false;
    if session.is_placing_from_drawer() {
        return;
    }
    let Some(world) = pointer.cursor_world else {
        return;
    };
    let Some(slot) = crate::domain::board::geometry::slot_at_world_position(world, visible.layout)
    else {
        return;
    };
    if visible.pick_at(slot).is_some() {
        hover.pickable = true;
    }
}

/// Samples pointer position against the board viewport (no application session mutation).
pub(crate) fn sample_board_placement_pointer(
    windows: Query<&Window>,
    board_camera: Query<(&Camera, &GlobalTransform), With<Board3dCamera>>,
    viewports: Query<
        (&ComputedNode, &UiGlobalTransform),
        With<super::board_camera_nav::UiBoardViewport>,
    >,
    mut pointer: ResMut<BoardPlacementPointer>,
) {
    pointer.window_cursor = windows.iter().next().and_then(|w| w.cursor_position());
    pointer.viewport_cursor = None;
    pointer.cursor_world = None;

    let Some(window_cursor) = pointer.window_cursor else {
        return;
    };
    let Ok((camera, camera_transform)) = board_camera.single() else {
        return;
    };
    let Ok((node, global)) = viewports.single() else {
        return;
    };
    let Some(viewport_cursor) =
        super::board_camera_nav::board_render_viewport_cursor(window_cursor, node, global, camera)
    else {
        return;
    };
    pointer.viewport_cursor = Some(viewport_cursor);

    let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_cursor) else {
        return;
    };
    let direction = ray.direction.normalize();
    if direction.y.abs() < 1e-5 {
        return;
    }
    let t = (SLOT_HEIGHT - ray.origin.y) / direction.y;
    if t < 0.0 {
        return;
    }
    pointer.cursor_world = Some(ray.origin + direction * t);
}

fn tessera_footprint_for_node(node: &VisibleBoardNode) -> tessera::prelude::TileFootprint {
    node.tessera_footprint
        .unwrap_or(tessera::prelude::TileFootprint::unit())
}

fn slot_position(slot: BoardSlot, y: f32) -> Vec3 {
    tessera_slot_center(slot, tessera::prelude::TileFootprint::unit(), y)
}

fn tile_world_position(address: PlacementAddress, footprint: f32) -> Vec3 {
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

fn try_spawn_gltf_tile(
    parent: &mut ChildSpawnerCommands<'_>,
    surface: BoardSurfaceId,
    node: &VisibleBoardNode,
    pick_target: BoardPickTarget,
    center: Vec3,
    tessera_footprint: tessera::prelude::TileFootprint,
    on_root_board: bool,
    tile_meshes: Option<&TileMeshAssets>,
    queries: &DocumentQueries<'_>,
    tile_icons: &TileIconAssets,
    standard_materials: &mut Assets<StandardMaterial>,
    atom_tiles: Option<&crate::adapter::load_up::AtomTileAssets>,
    ui_sprites: Option<&UiSpriteAssets>,
    images: Option<&Assets<Image>>,
    meshes: &mut Assets<Mesh>,
) -> Option<Entity> {
    let tile_meshes = tile_meshes.filter(|assets| assets.ready)?;
    let prototype =
        tile_meshes.prototype_for_visible(node.kind, tessera_footprint, on_root_board)?;
    let primitive = prototype.root.as_ref()?;

    Some(
        parent
            .spawn((
                BoardDragSurface { surface },
                Board3dTile {
                    surface,
                    node: node.node.clone(),
                    address: node.address,
                },
                pick_target,
                Mesh3d(primitive.mesh.clone()),
                MeshMaterial3d(primitive.material.clone()),
                SCENE_NODE_VISIBILITY,
                primitive.board_transform(Vec3::new(center.x, SLOT_HEIGHT, center.z)),
                Pickable::default(),
            ))
            .observe(on_tile_clicked)
            .with_children(|tile| {
                if let Some(atom_assets) = atom_tiles {
                    if let Some(crate::domain::document::DocumentNodeKind::Atom(atom)) =
                        queries.node_kind(&node.node)
                    {
                        if let Some(texture) = atom_assets.image_for_atom(&atom.atom).cloned() {
                            spawn_atom_center_icon(tile, tile_icons, standard_materials, texture);
                        }
                    }
                }
                if let (PlacementAddress::BoardSlot(slot), Some(sprites), Some(images)) =
                    (node.address, ui_sprites, images)
                    && let Some(view) =
                        connection_endpoint_view(queries, &node.node, Some(slot), None)
                {
                    spawn_board_port_compass(
                        tile,
                        surface,
                        &node.node,
                        board_footprint_for_kind(node.kind),
                        &view,
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

fn stack_insert_marker_center(index: StackIndex) -> Vec3 {
    let y = tile_center_y_for_footprint(STACK_CELL_WIDTH * 0.5, SLOT_HEIGHT);
    Vec3::new(stack_column_center(index), y, 0.0)
}

fn hovered_slot_to_visual_slot(address: PlacementAddress) -> BoardSlot {
    match address {
        PlacementAddress::BoardSlot(slot) => slot,
        PlacementAddress::StackIndex(index) => BoardSlot::new(index.0 as i32, 0),
    }
}

fn visual_slot_to_address(layout: SurfaceLayoutKind, slot: BoardSlot) -> PlacementAddress {
    match layout {
        SurfaceLayoutKind::Board => PlacementAddress::BoardSlot(slot),
        SurfaceLayoutKind::Stack => {
            PlacementAddress::StackIndex(StackIndex(slot.x.max(0) as usize))
        }
    }
}

fn stack_inner_width() -> f32 {
    STACK_LENGTH * SLOT_SIZE
}

fn stack_width() -> f32 {
    stack_inner_width() + STACK_MARGIN * 2.0
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::*;
    use crate::application::command::{EditorCommand, EditorCommandBus};
    use crate::application::editor::{SelectionMode, SelectionState, transaction::PlacementTarget};
    use crate::application::pipeline::PlaybackPlugin;
    use crate::domain::document::{ContainerKind, TileSpawnKind};
    use crate::infrastructure::app::{AppState, TransportMode};
    use tessera::bevy::TesseraPlugin;

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_state::<TransportMode>()
            .insert_resource(Assets::<Image>::default())
            .insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .init_resource::<TransformTileAssets>()
            .init_resource::<TileIconAssets>()
            .init_resource::<super::super::board_camera_nav::BoardCameraPointerState>()
            .init_resource::<super::super::camera_rig::BoardCameraRig>()
            .add_plugins((
                TesseraPlugin,
                crate::application::editor::EditorPlugin,
                PlaybackPlugin,
            ))
            .add_plugins(Board3dPlugin);
        app.insert_state(AppState::Editor);
        app
    }

    fn place_container(app: &mut App, surface: BoardSurfaceId, slot: BoardSlot) {
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot { surface, slot },
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            }));
        app.update();
        app.update();
    }

    fn tile_entity(world: &mut World, node: &NodeId) -> Entity {
        world
            .query::<(Entity, &Board3dTile)>()
            .iter(world)
            .find(|(_, tile)| &tile.node == node)
            .map(|(entity, _)| entity)
            .expect("board tile entity for node")
    }

    fn board_root(world: &mut World) -> Entity {
        world
            .query_filtered::<Entity, With<Board3dRoot>>()
            .iter(world)
            .next()
            .expect("Board3dRoot")
    }

    #[test]
    fn entering_editor_with_empty_project_spawns_board_grid_backdrop() {
        let mut app = test_app();
        app.update();
        app.update();

        let visible = app.world().resource::<VisibleBoardState>();
        assert!(
            visible.active_surface.is_some(),
            "scene sync should activate the root surface for an empty project"
        );
        let active_surface = visible.active_surface.unwrap();

        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<Board3dRoot>>()
            .iter(world)
            .next()
            .expect("entering Editor should spawn a Board3dRoot");
        world
            .query_filtered::<Entity, With<BoardGridAnchor>>()
            .iter(world)
            .next()
            .expect("entering Editor should spawn the persistent grid backdrop");
        let slab = world
            .query_filtered::<(&Mesh3d, &BoardDragSurface, &Pickable), With<BoardGridPickSlab>>()
            .iter(world)
            .next()
            .expect("grid backdrop should carry a pickable slab");
        assert_eq!(
            slab.1.surface, active_surface,
            "pick slab should be bound to the active root surface"
        );
    }

    #[test]
    fn selection_toggle_keeps_unrelated_tile_entity_stable() {
        let mut app = test_app();
        app.update();
        app.update();

        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;

        place_container(&mut app, root_surface, BoardSlot::new(0, 0));
        let first = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();

        place_container(&mut app, root_surface, BoardSlot::new(2, 0));
        let second = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();
        assert_ne!(first, second);

        let first_entity_before = tile_entity(app.world_mut(), &first);
        let second_entity_before = tile_entity(app.world_mut(), &second);
        let root_before = board_root(app.world_mut());

        // Select the second tile; first tile's visual key is unchanged.
        app.world_mut()
            .resource_mut::<SelectionState>()
            .select(second.clone(), SelectionMode::Replace);
        app.update();
        app.update();

        assert_eq!(board_root(app.world_mut()), root_before);
        assert_eq!(tile_entity(app.world_mut(), &first), first_entity_before);
        // Selected tile may respawn for highlight materials — that is fine.
        let _ = second_entity_before;
        assert!(
            app.world()
                .resource::<VisibleBoardState>()
                .nodes
                .iter()
                .any(|n| n.node == second && n.selected),
            "selection should project onto visible board"
        );
    }

    #[test]
    fn tile_address_change_keeps_board_root_stable() {
        let mut app = test_app();
        app.update();
        app.update();

        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;
        place_container(&mut app, root_surface, BoardSlot::new(0, 0));
        let node = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();
        let root_before = board_root(app.world_mut());

        {
            let mut visible = app.world_mut().resource_mut::<VisibleBoardState>();
            if let Some(entry) = visible.nodes.iter_mut().find(|n| n.node == node) {
                entry.address = PlacementAddress::BoardSlot(BoardSlot::new(2, 1));
            }
        }
        app.update();
        app.update();

        assert_eq!(board_root(app.world_mut()), root_before);
        let tile = app
            .world_mut()
            .query::<&Board3dTile>()
            .iter(app.world())
            .find(|t| t.node == node)
            .expect("moved tile still present");
        assert_eq!(
            tile.address,
            PlacementAddress::BoardSlot(BoardSlot::new(2, 1))
        );
    }
}
