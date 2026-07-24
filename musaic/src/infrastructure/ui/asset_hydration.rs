//! Build runtime mesh/material caches from load_up packs (no scattered `AssetServer::load`).

use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::state::condition::in_state;

use super::tile_icons::TileIconAssets;
use super::tile_mesh::{TileMeshAssets, finish_tile_mesh_assets_from_gltf};
use super::transform_tile::TransformTileAssets;
use crate::adapter::load_up::{
    BoardTileIconAssets, NearestSamplerApplied, OrthoTileAssets, TileMeshSourceAssets,
    ensure_nearest_samplers,
};
use crate::infrastructure::app::AppState;

pub struct EditorSpriteHydrationPlugin;

impl Plugin for EditorSpriteHydrationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TransformTileAssets>()
            .init_resource::<TileIconAssets>()
            .init_resource::<TileMeshAssets>()
            .add_systems(
                Update,
                (
                    hydrate_transform_tile_assets,
                    hydrate_tile_icon_assets,
                    hydrate_tile_mesh_assets,
                )
                    .chain()
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

fn hydrate_transform_tile_assets(
    ortho: Option<Res<OrthoTileAssets>>,
    mut assets: ResMut<TransformTileAssets>,
    mut images: ResMut<Assets<Image>>,
    mut applied: ResMut<NearestSamplerApplied>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if assets.ready {
        return;
    }
    let Some(ortho) = ortho else {
        return;
    };
    ensure_nearest_samplers(
        &mut images,
        &mut applied,
        [
            ortho.transform_ortho.clone(),
            ortho.container_ortho.clone(),
            ortho.output_ortho.clone(),
            ortho.atom_note_ortho.clone(),
            ortho.atom_scalar_ortho.clone(),
            ortho.atom_accidental_ortho.clone(),
            ortho.atom_operator_ortho.clone(),
        ],
    );
    super::transform_tile::finish_transform_tile_assets_from_handles(
        &mut assets,
        &mut meshes,
        &mut materials,
        ortho.transform_ortho.clone(),
        ortho.container_ortho.clone(),
        ortho.output_ortho.clone(),
        ortho.atom_note_ortho.clone(),
        ortho.atom_scalar_ortho.clone(),
        ortho.atom_accidental_ortho.clone(),
        ortho.atom_operator_ortho.clone(),
    );
}

fn hydrate_tile_icon_assets(
    pack: Option<Res<BoardTileIconAssets>>,
    mut assets: ResMut<TileIconAssets>,
    mut images: ResMut<Assets<Image>>,
    mut applied: ResMut<NearestSamplerApplied>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if assets.ready {
        return;
    }
    let Some(pack) = pack else {
        return;
    };
    ensure_nearest_samplers(
        &mut images,
        &mut applied,
        [
            pack.transform_fast.clone(),
            pack.transform_slow.clone(),
            pack.transform_legato.clone(),
            pack.transform_gain.clone(),
        ],
    );
    super::tile_icons::finish_tile_icon_assets_from_handles(
        &mut assets,
        &mut meshes,
        &mut materials,
        [
            pack.transform_fast.clone(),
            pack.transform_slow.clone(),
            pack.transform_legato.clone(),
            pack.transform_gain.clone(),
        ],
    );
}

fn hydrate_tile_mesh_assets(
    source: Option<Res<TileMeshSourceAssets>>,
    gltf_assets: Res<Assets<Gltf>>,
    gltf_nodes: Res<Assets<bevy::gltf::GltfNode>>,
    gltf_meshes: Res<Assets<bevy::gltf::GltfMesh>>,
    meshes: Res<Assets<Mesh>>,
    mut assets: ResMut<TileMeshAssets>,
) {
    if assets.ready {
        return;
    }
    let Some(source) = source else {
        return;
    };
    let Some(gltf) = gltf_assets.get(&source.model) else {
        return;
    };
    finish_tile_mesh_assets_from_gltf(&mut assets, gltf, &gltf_nodes, &gltf_meshes, &meshes);
}
