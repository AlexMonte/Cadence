//! Single board-tile spawn API for placed tiles, placement ghosts, and palette entries.
//!
//! One resolve path owns GLTF fit/ground-lift, ortho fallback, and Cuboid placeholder.
//! [`BoardTileMode::Preview`] differs only by [`translucent_preview_material`].

use bevy::{math::primitives::Cuboid, prelude::*};
use tessera::prelude::TileFootprint;

use crate::{
    application::pipeline::scene_sync::VisibleNodeKind,
    domain::document::TileSpawnKind,
    infrastructure::ui::{
        musaic_tile::{
            TILE_WORLD_DEPTH, TILE_WORLD_HEIGHT, TILE_WORLD_WIDTH, TileVisualSpec,
            tile_center_y_for_footprint, tile_scale_for_footprint,
        },
        render_layers::SCENE_NODE_VISIBILITY,
        tile_mesh::TileMeshAssets,
        transform_tile::{TransformTileAssets, TransformTileBase, frames},
    },
};

pub const PLACEMENT_PREVIEW_ALPHA: f32 = 0.58;

/// Presentation mode for [`spawn_board_tile`] / [`resolve_board_tile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardTileMode {
    /// Opaque tile on the board or stack.
    Placed,
    /// Placement ghost: identical transform, translucent material only.
    Preview,
    /// Catalog entry in the inspector tile palette.
    Palette,
}

/// Which mesh family produced a resolved visual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardTileVisualSource {
    Gltf,
    Ortho,
    Fallback,
}

/// Inputs shared by board, ghost, and palette spawn sites.
#[derive(Debug, Clone)]
pub struct BoardTileSpec {
    pub kind: VisibleNodeKind,
    pub tessera_footprint: TileFootprint,
    pub on_root_board: bool,
    /// Slot anchor on the board/palette plane (`Y` = ground). GLTF applies ground lift.
    pub plane_anchor: Vec3,
    /// Ortho / Cuboid footprint in world units.
    pub visual_footprint: f32,
    /// Ortho sheet selection when GLTF is unavailable.
    pub ortho: Option<TileVisualSpec>,
}

/// Mesh + material + transform from the single spawn policy.
#[derive(Clone)]
pub struct ResolvedBoardTile {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    pub transform: Transform,
    pub source: BoardTileVisualSource,
}

pub fn visible_kind_for_spawn(tile: &TileSpawnKind) -> VisibleNodeKind {
    match tile {
        TileSpawnKind::Atom { .. } => VisibleNodeKind::Atom,
        TileSpawnKind::Container { .. } => VisibleNodeKind::Container,
        TileSpawnKind::Output { .. } => VisibleNodeKind::Output,
        TileSpawnKind::TrickInstance { .. } => VisibleNodeKind::TrickInstance,
        TileSpawnKind::Tile { .. } => VisibleNodeKind::Tile,
    }
}

/// Semi-transparent clone of a tile material for placement ghosts.
pub fn translucent_preview_material(
    materials: &mut Assets<StandardMaterial>,
    source: &Handle<StandardMaterial>,
) -> Handle<StandardMaterial> {
    let mut preview_material = materials.get(source).cloned().unwrap_or_default();
    preview_material
        .base_color
        .set_alpha(PLACEMENT_PREVIEW_ALPHA);
    preview_material.alpha_mode = AlphaMode::Blend;
    materials.add(preview_material)
}

/// World transform for a board tile at `spec.plane_anchor` (GLTF fit or ortho/fallback scale).
///
/// [`BoardTileMode::Preview`] uses the same transform as [`BoardTileMode::Placed`].
pub fn board_tile_transform(
    tile_meshes: Option<&TileMeshAssets>,
    spec: &BoardTileSpec,
    mode: BoardTileMode,
) -> Transform {
    if let Some(primitive) = gltf_tile_primitive(tile_meshes, spec) {
        return primitive.board_transform(spec.plane_anchor);
    }
    fallback_tile_transform(spec, mode)
}

/// Resolve mesh, material, and transform. Preview only changes the material.
pub fn resolve_board_tile(
    tile_meshes: Option<&TileMeshAssets>,
    transform_tiles: Option<&TransformTileAssets>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    fallback_material: Option<&Handle<StandardMaterial>>,
    spec: &BoardTileSpec,
    mode: BoardTileMode,
) -> Option<ResolvedBoardTile> {
    if let Some(primitive) = gltf_tile_primitive(tile_meshes, spec) {
        let base = primitive.material.clone();
        let material = material_for_mode(materials, mode, &base);
        return Some(ResolvedBoardTile {
            mesh: primitive.mesh.clone(),
            material,
            transform: primitive.board_transform(spec.plane_anchor),
            source: BoardTileVisualSource::Gltf,
        });
    }

    if let Some(ortho) = ortho_tile_parts(transform_tiles, spec) {
        let material = material_for_mode(materials, mode, &ortho.material);
        return Some(ResolvedBoardTile {
            mesh: ortho.mesh,
            material,
            transform: fallback_tile_transform(spec, mode),
            source: BoardTileVisualSource::Ortho,
        });
    }

    let base = fallback_material?.clone();
    let material = material_for_mode(materials, mode, &base);
    Some(ResolvedBoardTile {
        mesh: meshes.add(Cuboid::new(
            TILE_WORLD_WIDTH,
            TILE_WORLD_HEIGHT,
            TILE_WORLD_DEPTH,
        )),
        material,
        transform: fallback_tile_transform(spec, mode),
        source: BoardTileVisualSource::Fallback,
    })
}

/// Spawns a board tile at the scene root (placement ghost).
pub fn spawn_board_tile(
    commands: &mut Commands,
    tile_meshes: Option<&TileMeshAssets>,
    transform_tiles: Option<&TransformTileAssets>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    fallback_material: Option<&Handle<StandardMaterial>>,
    spec: &BoardTileSpec,
    mode: BoardTileMode,
    extra: impl Bundle,
) -> Option<Entity> {
    let visual = resolve_board_tile(
        tile_meshes,
        transform_tiles,
        meshes,
        materials,
        fallback_material,
        spec,
        mode,
    )?;
    let mut entity = commands.spawn((
        extra,
        Mesh3d(visual.mesh),
        MeshMaterial3d(visual.material),
        visual.transform,
    ));
    if visual.source == BoardTileVisualSource::Ortho {
        entity.insert(TransformTileBase);
    }
    Some(entity.id())
}

/// Spawns a board tile under an existing parent (placed board tile or palette entry).
pub fn spawn_board_tile_child(
    parent: &mut ChildSpawnerCommands<'_>,
    tile_meshes: Option<&TileMeshAssets>,
    transform_tiles: Option<&TransformTileAssets>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    fallback_material: Option<&Handle<StandardMaterial>>,
    spec: &BoardTileSpec,
    mode: BoardTileMode,
    extra: impl Bundle,
) -> Option<Entity> {
    let visual = resolve_board_tile(
        tile_meshes,
        transform_tiles,
        meshes,
        materials,
        fallback_material,
        spec,
        mode,
    )?;
    let mut entity = parent.spawn((
        extra,
        Mesh3d(visual.mesh),
        MeshMaterial3d(visual.material),
        SCENE_NODE_VISIBILITY,
        visual.transform,
    ));
    if visual.source == BoardTileVisualSource::Ortho {
        entity.insert(TransformTileBase);
    }
    Some(entity.id())
}

fn material_for_mode(
    materials: &mut Assets<StandardMaterial>,
    mode: BoardTileMode,
    base: &Handle<StandardMaterial>,
) -> Handle<StandardMaterial> {
    match mode {
        BoardTileMode::Preview => translucent_preview_material(materials, base),
        BoardTileMode::Placed | BoardTileMode::Palette => base.clone(),
    }
}

fn gltf_tile_primitive<'a>(
    tile_meshes: Option<&'a TileMeshAssets>,
    spec: &BoardTileSpec,
) -> Option<&'a crate::infrastructure::ui::tile_mesh::TileMeshPrimitive> {
    let tile_meshes = tile_meshes.filter(|assets| assets.ready)?;
    let prototype =
        tile_meshes.prototype_for_visible(spec.kind, spec.tessera_footprint, spec.on_root_board)?;
    prototype.root.as_ref()
}

struct OrthoParts {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

fn ortho_tile_parts(
    transform_tiles: Option<&TransformTileAssets>,
    spec: &BoardTileSpec,
) -> Option<OrthoParts> {
    let assets = transform_tiles.filter(|assets| assets.ready)?;
    let ortho = spec.ortho.as_ref()?;
    let sheet_materials = assets.materials(ortho.sheet);
    let material = match ortho.frame {
        frames::PLAIN => sheet_materials.plain.clone(),
        frames::HIGHLIGHT => sheet_materials.highlight.clone(),
        _ => sheet_materials.framed.clone(),
    };
    Some(OrthoParts {
        mesh: assets.meshes.for_frame(ortho.frame),
        material,
    })
}

fn fallback_tile_transform(spec: &BoardTileSpec, mode: BoardTileMode) -> Transform {
    // Preview must match Placed. Palette stays on the local plane (no board lift).
    let y = match mode {
        BoardTileMode::Palette => spec.plane_anchor.y,
        BoardTileMode::Placed | BoardTileMode::Preview => {
            tile_center_y_for_footprint(spec.visual_footprint, spec.plane_anchor.y)
        }
    };
    Transform::from_translation(Vec3::new(spec.plane_anchor.x, y, spec.plane_anchor.z))
        .with_scale(tile_scale_for_footprint(spec.visual_footprint))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::board::geometry::BOARD_PLANE_Y;
    use crate::infrastructure::ui::musaic_tile::BOARD_MACRO_TILE_FOOTPRINT;

    fn sample_spec(plane_anchor: Vec3) -> BoardTileSpec {
        BoardTileSpec {
            kind: VisibleNodeKind::Container,
            tessera_footprint: TileFootprint::unit(),
            on_root_board: true,
            plane_anchor,
            visual_footprint: BOARD_MACRO_TILE_FOOTPRINT,
            ortho: None,
        }
    }

    #[test]
    fn placed_and_preview_share_fallback_transform() {
        let plane = Vec3::new(2.0, BOARD_PLANE_Y, -1.0);
        let spec = sample_spec(plane);
        let placed = board_tile_transform(None, &spec, BoardTileMode::Placed);
        let preview = board_tile_transform(None, &spec, BoardTileMode::Preview);
        assert_eq!(placed.translation, preview.translation);
        assert_eq!(placed.scale, preview.scale);
        assert!(placed.translation.y > BOARD_PLANE_Y);
    }

    #[test]
    fn preview_material_only_changes_alpha_mode_path() {
        let mut materials = Assets::<StandardMaterial>::default();
        let base = materials.add(StandardMaterial {
            base_color: Color::srgb(0.5, 0.4, 0.3),
            ..default()
        });
        let preview = translucent_preview_material(&mut materials, &base);
        let preview_mat = materials.get(&preview).expect("preview material");
        assert_eq!(preview_mat.alpha_mode, AlphaMode::Blend);
        assert!((preview_mat.base_color.alpha() - PLACEMENT_PREVIEW_ALPHA).abs() < 1e-4);
    }
}
