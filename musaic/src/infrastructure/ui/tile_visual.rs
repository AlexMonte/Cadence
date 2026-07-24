//! Shared GLTF tile mesh spawning for board, palette, and drag preview.

use bevy::prelude::*;
use tessera::prelude::TileFootprint;

use crate::{
    application::pipeline::scene_sync::VisibleNodeKind,
    domain::document::TileSpawnKind,
    infrastructure::ui::{render_layers::SCENE_NODE_VISIBILITY, tile_mesh::TileMeshAssets},
};

pub const PLACEMENT_PREVIEW_ALPHA: f32 = 0.58;

pub fn visible_kind_for_spawn(tile: &TileSpawnKind) -> VisibleNodeKind {
    match tile {
        TileSpawnKind::Atom { .. } => VisibleNodeKind::Atom,
        TileSpawnKind::Container { .. } => VisibleNodeKind::Container,
        TileSpawnKind::Output { .. } => VisibleNodeKind::Output,
        TileSpawnKind::TrickInstance { .. } => VisibleNodeKind::TrickInstance,
        TileSpawnKind::Tile { .. } => VisibleNodeKind::Tile,
    }
}

/// Semi-transparent clone of a GLTF tile material for placement ghosts.
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

fn gltf_tile_prototype<'a>(
    tile_meshes: &'a TileMeshAssets,
    kind: VisibleNodeKind,
    footprint: TileFootprint,
    on_root_board: bool,
) -> Option<&'a crate::infrastructure::ui::tile_mesh::TileMeshPrimitive> {
    if !tile_meshes.ready {
        return None;
    }
    let prototype = tile_meshes.prototype_for_visible(kind, footprint, on_root_board)?;
    prototype.root.as_ref()
}

/// Spawns a GLTF tile prototype mesh at the scene root when assets are ready.
///
/// `center` is the slot center on the board plane; the primitive's fit scale
/// and ground lift place the mesh footprint on that slot. `material` overrides
/// the prototype's authored material (e.g. translucent placement ghosts).
pub fn spawn_gltf_tile_mesh(
    commands: &mut Commands,
    tile_meshes: &TileMeshAssets,
    kind: VisibleNodeKind,
    footprint: TileFootprint,
    on_root_board: bool,
    material: Option<Handle<StandardMaterial>>,
    center: Vec3,
    extra: impl Bundle,
) -> bool {
    let Some(primitive) = gltf_tile_prototype(tile_meshes, kind, footprint, on_root_board) else {
        return false;
    };

    commands.spawn((
        extra,
        Mesh3d(primitive.mesh.clone()),
        MeshMaterial3d(material.unwrap_or_else(|| primitive.material.clone())),
        primitive.board_transform(center),
    ));
    true
}

/// Spawns a GLTF tile prototype mesh under an existing parent entity.
pub fn spawn_gltf_tile_mesh_child(
    parent: &mut ChildSpawnerCommands<'_>,
    tile_meshes: &TileMeshAssets,
    kind: VisibleNodeKind,
    footprint: TileFootprint,
    on_root_board: bool,
    material: Option<Handle<StandardMaterial>>,
    center: Vec3,
    extra: impl Bundle,
) -> bool {
    let Some(primitive) = gltf_tile_prototype(tile_meshes, kind, footprint, on_root_board) else {
        return false;
    };

    parent.spawn((
        extra,
        Mesh3d(primitive.mesh.clone()),
        MeshMaterial3d(material.unwrap_or_else(|| primitive.material.clone())),
        SCENE_NODE_VISIBILITY,
        primitive.board_transform(center),
    ));
    true
}
