//! Inspector tile drawer: ortho tiles as 3D entities in a dedicated viewport (not UI buttons).

use crate::infrastructure::app::AppState;
use bevy::{
    asset::RenderAssetUsages,
    camera::{OrthographicProjection, RenderTarget, ScalingMode},
    picking::prelude::*,
    prelude::*,
    render::render_resource::{TextureDimension, TextureFormat, TextureUsages},
    ui::widget::ViewportNode,
};

use crate::{
    adapter::load_up::{
        AtomTileAssets, BoardTileIconAssets, GLYPH_ATLAS_COLS, GLYPH_ATLAS_ROWS,
        GLYPH_CELL_SUBDIVISION,
    },
    application::editor::TileLibraryContextKind,
    domain::document::{ContainerKind, TileSpawnKind, root_board_tile_footprint},
    infrastructure::ui::{
        DrawerTileSource,
        musaic_tile::tile_visual_for_spawn,
        on_drawer_tile_press, on_drawer_tile_release,
        render_layers::{
            PALETTE_VIEW, PALETTE_WORLD_ORIGIN, SCENE_NODE_VISIBILITY, SCENE_ROOT_VISIBILITY,
        },
        tile_icons::TileIconAssets,
        tile_mesh::TileMeshAssets,
        tile_visual::{BoardTileMode, BoardTileSpec, spawn_board_tile_child, visible_kind_for_spawn},
        transform_tile::TransformTileAssets,
    },
};

#[derive(Component)]
pub struct UiTilePaletteCamera;

#[derive(Component)]
pub struct UiTilePaletteRoot;

#[derive(Component)]
pub struct TilePaletteViewport;

#[derive(Component)]
pub struct UiTilePaletteSpec {
    pub context: TileLibraryContextKind,
}

#[derive(Resource, Debug, Clone, Copy)]
pub struct UiTilePaletteDisplay {
    pub context: TileLibraryContextKind,
}

impl Default for UiTilePaletteDisplay {
    fn default() -> Self {
        Self {
            context: TileLibraryContextKind::RootBoard,
        }
    }
}

pub struct UiTilePalettePlugin;

impl Plugin for UiTilePalettePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiTilePaletteDisplay>()
            .add_systems(OnEnter(AppState::Editor), setup_ui_tile_palette_scene)
            .add_systems(OnExit(AppState::Editor), teardown_ui_tile_palette_scene);
        // sync_ui_tile_palette_scene + frame_ui_tile_palette_camera are in MusaicUiPlugin RenderUi.
    }
}

const PALETTE_COLUMNS: usize = 2;
const PALETTE_VIEWPORT_HEIGHT_MIN: f32 = 4.0;
const PALETTE_TILE_SPACING: f32 = 1.55;
const PALETTE_FRAMING_PAD: f32 = 1.1;
/// Label glyph floating above each palette tile.
const PALETTE_ICON_SIZE: f32 = 0.62;
const PALETTE_ICON_LIFT: f32 = 0.55;

fn palette_local_position(index: usize) -> Vec3 {
    let col = (index % PALETTE_COLUMNS) as f32;
    let row = (index / PALETTE_COLUMNS) as f32;
    Vec3::new(col * PALETTE_TILE_SPACING, 0.0, row * PALETTE_TILE_SPACING)
}

fn palette_row_count(item_count: usize) -> usize {
    item_count.div_ceil(PALETTE_COLUMNS).max(1)
}

/// Ortho vertical extent so every catalog tile fits in the drawer viewport.
fn palette_viewport_height_for_count(item_count: usize) -> f32 {
    let rows = palette_row_count(item_count) as f32;
    let extent = (rows - 1.0).max(0.0) * PALETTE_TILE_SPACING + PALETTE_FRAMING_PAD * 2.0;
    extent.max(PALETTE_VIEWPORT_HEIGHT_MIN)
}

fn palette_scene_focus_for_count(item_count: usize) -> Vec3 {
    let rows = palette_row_count(item_count) as f32;
    let center_z = (rows - 1.0).max(0.0) * PALETTE_TILE_SPACING * 0.5;
    PALETTE_WORLD_ORIGIN + Vec3::new(0.75, 0.0, center_z)
}

fn palette_scene_focus() -> Vec3 {
    palette_scene_focus_for_count(0)
}

fn palette_isometric_camera_transform(focus: Vec3, distance: f32) -> Transform {
    let pitch = std::f32::consts::FRAC_PI_4;
    let offset = Vec3::new(0.0, distance * pitch.sin(), distance * pitch.cos());
    Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y)
}

fn teardown_ui_tile_palette_scene(
    mut commands: Commands,
    cameras: Query<Entity, With<UiTilePaletteCamera>>,
    roots: Query<Entity, With<UiTilePaletteRoot>>,
    lights: Query<Entity, With<PaletteSceneLight>>,
) {
    for entity in cameras.iter().chain(roots.iter()).chain(lights.iter()) {
        commands.entity(entity).despawn();
    }
}

#[derive(Component)]
struct PaletteSceneLight;

fn setup_ui_tile_palette_scene(
    mut commands: Commands,
    theme: Res<crate::infrastructure::ui::theme::MusaicUiTheme>,
    mut images: ResMut<Assets<Image>>,
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

    let focus = palette_scene_focus();

    commands.spawn((
        UiTilePaletteCamera,
        Camera3d::default(),
        MeshPickingCamera,
        PALETTE_VIEW,
        Camera {
            order: -2,
            clear_color: ClearColorConfig::Custom(theme.chrome.palette_clear),
            ..default()
        },
        RenderTarget::Image(image_handle.into()),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: PALETTE_VIEWPORT_HEIGHT_MIN,
            },
            near: -100.0,
            far: 100.0,
            ..OrthographicProjection::default_3d()
        }),
        palette_isometric_camera_transform(focus, 7.5),
        SCENE_ROOT_VISIBILITY,
    ));

    commands.spawn((
        PaletteSceneLight,
        DirectionalLight {
            illuminance: 14_000.0,
            shadows_enabled: false,
            ..default()
        },
        PALETTE_VIEW,
        SCENE_ROOT_VISIBILITY,
        Transform::from_translation(focus + Vec3::new(0.0, 8.0, 2.0)).looking_at(focus, Vec3::Y),
    ));

    commands.spawn((
        UiTilePaletteRoot,
        PALETTE_VIEW,
        UiTilePaletteSpec {
            context: TileLibraryContextKind::RootBoard,
        },
        Transform::from_translation(PALETTE_WORLD_ORIGIN),
        GlobalTransform::default(),
        SCENE_ROOT_VISIBILITY,
    ));
}

pub fn tile_palette_viewport(camera: Entity) -> impl Bundle {
    (
        TilePaletteViewport,
        Node {
            width: percent(100.0),
            flex_grow: 1.0,
            min_height: px(0.0),
            margin: UiRect::vertical(px(6.0)),
            border_radius: BorderRadius::all(px(8.0)),
            overflow: Overflow::clip(),
            ..default()
        },
        ViewportNode::new(camera),
    )
}

pub fn sync_ui_tile_palette_scene(
    mut commands: Commands,
    theme: Res<crate::infrastructure::ui::theme::MusaicUiTheme>,
    assets: Res<TransformTileAssets>,
    tile_meshes: Option<Res<TileMeshAssets>>,
    tile_icons: Res<TileIconAssets>,
    board_icons: Option<Res<BoardTileIconAssets>>,
    atom_tiles: Option<Res<AtomTileAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    dirty: Res<crate::application::pipeline::ui_projection::UiDirty>,
    projection: Res<crate::application::pipeline::ui_projection::EditorUiProjection>,
    mut display: ResMut<UiTilePaletteDisplay>,
    root: Query<Entity, With<UiTilePaletteRoot>>,
    children: Query<&Children>,
    mut last_ready: Local<(bool, bool, bool)>,
) {
    display.context = projection.palette.context;

    let assets_ready = assets.ready;
    let icons_ready = tile_icons.ready;
    let meshes_ready = tile_meshes.as_ref().is_some_and(|m| m.ready);
    let ready_changed =
        last_ready.0 != assets_ready || last_ready.1 != icons_ready || last_ready.2 != meshes_ready;
    *last_ready = (assets_ready, icons_ready, meshes_ready);

    if !dirty.palette && !ready_changed {
        return;
    }

    let Ok(root) = root.single() else {
        return;
    };

    let items = &projection.palette.options;
    let armed = projection.palette.armed.as_ref();
    let on_root_board = projection.palette.context == TileLibraryContextKind::RootBoard;

    if let Ok(kids) = children.get(root) {
        for child in kids.iter() {
            commands.entity(child).despawn();
        }
    }

    commands.entity(root).with_children(|parent| {
        for (index, item) in items.iter().enumerate() {
            let highlighted = armed == Some(&item.spawn);
            let Some(visual) = tile_visual_for_spawn(&item.spawn, highlighted) else {
                continue;
            };

            let source = DrawerTileSource {
                tile: item.spawn.clone(),
            };
            let slot = palette_local_position(index);

            parent
                .spawn((
                    source,
                    PALETTE_VIEW,
                    SCENE_NODE_VISIBILITY,
                    Transform::from_translation(slot),
                    Pickable::default(),
                ))
                .observe(on_drawer_tile_press)
                .observe(on_drawer_tile_release)
                .with_children(|tile_root| {
                    spawn_palette_label_icon(
                        tile_root,
                        &item.spawn,
                        &tile_icons,
                        board_icons.as_deref(),
                        atom_tiles.as_deref(),
                        &mut materials,
                    );

                    let kind = visible_kind_for_spawn(&item.spawn);
                    let tessera_footprint = if on_root_board {
                        root_board_tile_footprint(&item.spawn)
                    } else {
                        tessera::prelude::TileFootprint::unit()
                    };
                    let needs_fallback = !(tile_meshes.as_ref().is_some_and(|m| m.ready)
                        || assets.ready);
                    let fallback = needs_fallback.then(|| {
                        materials.add(StandardMaterial {
                            base_color: theme.chrome.palette_fallback_tile,
                            ..default()
                        })
                    });
                    spawn_board_tile_child(
                        tile_root,
                        tile_meshes.as_deref(),
                        Some(&assets),
                        &mut meshes,
                        &mut materials,
                        fallback.as_ref(),
                        &BoardTileSpec {
                            kind,
                            tessera_footprint,
                            on_root_board,
                            plane_anchor: Vec3::ZERO,
                            visual_footprint: visual.footprint,
                            ortho: Some(visual),
                        },
                        BoardTileMode::Palette,
                        PALETTE_VIEW,
                    );
                });
        }
    });
}

/// Frames the palette ortho camera so every catalog tile is visible.
pub fn frame_ui_tile_palette_camera(
    projection: Res<crate::application::pipeline::ui_projection::EditorUiProjection>,
    mut cameras: Query<(&mut Transform, &mut Projection), With<UiTilePaletteCamera>>,
) {
    let item_count = projection.palette.options.len();
    let Ok((mut transform, mut camera_projection)) = cameras.single_mut() else {
        return;
    };
    let focus = palette_scene_focus_for_count(item_count);
    *transform = palette_isometric_camera_transform(focus, 7.5);
    if let Projection::Orthographic(ortho) = camera_projection.as_mut() {
        ortho.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: palette_viewport_height_for_count(item_count),
        };
    }
}

/// The label texture for a palette entry, reusing the 2D tile sprites until
/// the dedicated 3D tile textures are authored. Returns the texture plus an
/// optional UV cell into the glyph atlas.
fn palette_label_texture(
    spawn: &TileSpawnKind,
    board_icons: Option<&BoardTileIconAssets>,
    atom_tiles: Option<&AtomTileAssets>,
) -> Option<(Handle<Image>, Option<(u32, u32)>)> {
    match spawn {
        TileSpawnKind::Container { kind } => {
            let icons = board_icons?;
            match kind {
                ContainerKind::Sequence => Some((icons.container_sequence.clone(), None)),
                ContainerKind::Alternating => Some((icons.container_alternate.clone(), None)),
                ContainerKind::Parallel => Some((icons.container_parallel.clone(), None)),
                ContainerKind::Subdivision => {
                    Some((icons.glyph_atlas.clone(), Some(GLYPH_CELL_SUBDIVISION)))
                }
            }
        }
        TileSpawnKind::Output { .. } => Some((board_icons?.output_tile.clone(), None)),
        TileSpawnKind::TrickInstance { prototype } => {
            let icons = board_icons?;
            let texture = match prototype.0 {
                0 => icons.transform_fast.clone(),
                1 => icons.transform_slow.clone(),
                2 => icons.transform_legato.clone(),
                _ => icons.transform_gain.clone(),
            };
            Some((texture, None))
        }
        TileSpawnKind::Atom { atom } => Some((atom_tiles?.image_for_atom(atom)?.clone(), None)),
        TileSpawnKind::Tile { .. } => None,
    }
}

/// Unlit glyph quad floating above a palette tile so entries stay readable
/// while the authored tile textures are unfinished.
fn spawn_palette_label_icon(
    tile_root: &mut ChildSpawnerCommands<'_>,
    spawn: &TileSpawnKind,
    tile_icons: &TileIconAssets,
    board_icons: Option<&BoardTileIconAssets>,
    atom_tiles: Option<&AtomTileAssets>,
    materials: &mut Assets<StandardMaterial>,
) {
    if !tile_icons.ready {
        return;
    }
    let Some((texture, atlas_cell)) = palette_label_texture(spawn, board_icons, atom_tiles) else {
        return;
    };

    let uv_transform = atlas_cell
        .map(|(col, row)| bevy::math::Affine2 {
            matrix2: bevy::math::Mat2::from_diagonal(Vec2::new(
                1.0 / GLYPH_ATLAS_COLS as f32,
                1.0 / GLYPH_ATLAS_ROWS as f32,
            )),
            translation: Vec2::new(
                col as f32 / GLYPH_ATLAS_COLS as f32,
                row as f32 / GLYPH_ATLAS_ROWS as f32,
            ),
        })
        .unwrap_or_default();

    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(texture),
        uv_transform,
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        cull_mode: None,
        ..default()
    });

    tile_root.spawn((
        Mesh3d(tile_icons.quad.clone()),
        MeshMaterial3d(material),
        PALETTE_VIEW,
        SCENE_NODE_VISIBILITY,
        Transform::from_xyz(0.0, PALETTE_ICON_LIFT, 0.0).with_scale(Vec3::splat(PALETTE_ICON_SIZE)),
    ));
}
