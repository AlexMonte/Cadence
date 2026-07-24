//! Loads and spawns center tile icon meshes (board + palette).

use bevy::prelude::*;

use super::render_layers::SCENE_NODE_VISIBILITY;

pub use crate::adapter::tile_icons::{
    TileIconId, icon_for_container, icon_for_spawn, icon_for_trick_prototype,
};

pub const ICON_FOOTPRINT: f32 = 0.52;
pub const ICON_THICKNESS: f32 = 0.02;
pub const ICON_Y_LIFT: f32 = 0.06;

#[derive(Resource, Default)]
pub struct TileIconAssets {
    pub ready: bool,
    pub quad: Handle<Mesh>,
    fast: Handle<Image>,
    slow: Handle<Image>,
    legato: Handle<Image>,
    gain: Handle<Image>,
    materials: [Handle<StandardMaterial>; 4],
}

impl TileIconAssets {
    pub fn material(&self, icon: TileIconId) -> Option<Handle<StandardMaterial>> {
        if !self.ready {
            return None;
        }
        let index = match icon {
            TileIconId::TransformFast => 0,
            TileIconId::TransformSlow => 1,
            TileIconId::TransformLegato => 2,
            TileIconId::TransformGain => 3,
        };
        Some(self.materials[index].clone())
    }
}

fn icon_quad_mesh(size: f32) -> Mesh {
    use bevy::mesh::Indices;
    use bevy::render::render_resource::PrimitiveTopology;

    let half = size * 0.5;
    let positions = vec![
        [-half, 0.0, -half],
        [half, 0.0, -half],
        [half, 0.0, half],
        [-half, 0.0, half],
    ];
    let normals = vec![[0.0, 1.0, 0.0]; 4];
    let uvs = vec![[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    let indices = Indices::U32(vec![0, 2, 1, 0, 3, 2]);

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(indices);
    mesh
}

pub fn finish_tile_icon_assets_from_handles(
    assets: &mut TileIconAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    textures: [Handle<Image>; 4],
) {
    if assets.ready {
        return;
    }

    assets.fast = textures[0].clone();
    assets.slow = textures[1].clone();
    assets.legato = textures[2].clone();
    assets.gain = textures[3].clone();

    assets.quad = meshes.add(icon_quad_mesh(1.0));

    let mut make = |texture: Handle<Image>| {
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(texture),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        })
    };

    assets.materials = [
        make(assets.fast.clone()),
        make(assets.slow.clone()),
        make(assets.legato.clone()),
        make(assets.gain.clone()),
    ];
    assets.ready = true;
}

pub fn spawn_tile_icon(
    parent: &mut ChildSpawnerCommands<'_>,
    icons: &TileIconAssets,
    icon: TileIconId,
    extra: impl Bundle,
) {
    let Some(material) = icons.material(icon) else {
        return;
    };

    parent.spawn((
        extra,
        Mesh3d(icons.quad.clone()),
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
