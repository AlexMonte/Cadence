//! GLTF tile prototypes and socket map for 3D board rendering.

use std::collections::HashMap;

use bevy::{
    gltf::{Gltf, GltfMesh, GltfNode},
    prelude::*,
};

pub const TILE_MODEL_PATH: &str = "tiles/models/musaic_tiles_model.gltf";

const PROTOTYPE_MESH_NAMES: [(&str, TilePrototypeKind); 5] = [
    ("MESH_container_tile_1x1", TilePrototypeKind::Container1x1),
    ("MESH_container_tile_2x2", TilePrototypeKind::Container2x2),
    ("MESH_normal_tile_1x1", TilePrototypeKind::Normal1x1),
    ("MESH_normal_tile_2x2", TilePrototypeKind::Normal2x2),
    ("MESH_atom_tile", TilePrototypeKind::Atom),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TilePrototypeKind {
    Container1x1,
    Container2x2,
    Normal1x1,
    Normal2x2,
    Atom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileSocketKind {
    Container1x1 { row: u32, col: u32 },
    Container2x2 { row: u32, col: u32 },
    Normal1x1 { index: u32 },
    Normal2x2 { index: u32 },
    AtomStack,
}

#[derive(Debug, Clone)]
pub struct TileMeshPrimitive {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    /// Uniform scale mapping the raw mesh footprint onto board slots
    /// (`footprint_slots × SLOT_SIZE` across the mesh's larger XZ extent).
    pub fit_scale: f32,
    /// World-space lift so the scaled mesh's lowest point sits on the board plane.
    pub ground_lift: f32,
}

impl TileMeshPrimitive {
    /// Transform placing this tile at `center` (XZ) resting on the plane at `center.y`.
    ///
    /// Single source of truth for board tiles, palette entries, and drag ghosts —
    /// see `docs/BOARD_PLACEMENT.md` (ghost must match `try_spawn_gltf_tile`).
    pub fn board_transform(&self, center: Vec3) -> Transform {
        Transform::from_translation(Vec3::new(center.x, center.y + self.ground_lift, center.z))
            .with_scale(Vec3::splat(self.fit_scale))
    }
}

#[derive(Debug, Clone, Default)]
pub struct TileMeshPrototype {
    pub root: Option<TileMeshPrimitive>,
    pub sockets: HashMap<TileSocketKind, Transform>,
}

#[derive(Resource, Default, Clone)]
pub struct TileMeshAssets {
    pub ready: bool,
    pub container_1x1: TileMeshPrototype,
    pub container_2x2: TileMeshPrototype,
    pub normal_1x1: TileMeshPrototype,
    pub normal_2x2: TileMeshPrototype,
    pub atom: TileMeshPrototype,
}

impl TileMeshAssets {
    pub fn prototype(&self, kind: TilePrototypeKind) -> &TileMeshPrototype {
        match kind {
            TilePrototypeKind::Container1x1 => &self.container_1x1,
            TilePrototypeKind::Container2x2 => &self.container_2x2,
            TilePrototypeKind::Normal1x1 => &self.normal_1x1,
            TilePrototypeKind::Normal2x2 => &self.normal_2x2,
            TilePrototypeKind::Atom => &self.atom,
        }
    }

    pub fn prototype_for_visible(
        &self,
        kind: crate::application::pipeline::scene_sync::VisibleNodeKind,
        footprint: tessera::prelude::TileFootprint,
        on_root_board: bool,
    ) -> Option<&TileMeshPrototype> {
        if !self.ready {
            return None;
        }
        let prototype = match kind {
            crate::application::pipeline::scene_sync::VisibleNodeKind::Container
                if on_root_board =>
            {
                if footprint.width >= 2 || footprint.height >= 2 {
                    TilePrototypeKind::Container2x2
                } else {
                    TilePrototypeKind::Container1x1
                }
            }
            crate::application::pipeline::scene_sync::VisibleNodeKind::Atom => {
                TilePrototypeKind::Atom
            }
            _ => {
                if footprint.width >= 2 || footprint.height >= 2 {
                    TilePrototypeKind::Normal2x2
                } else {
                    TilePrototypeKind::Normal1x1
                }
            }
        };
        Some(self.prototype(prototype))
    }
}

pub fn parse_socket_name(name: &str) -> Option<TileSocketKind> {
    let rest = name.strip_prefix("SOCKET_")?;
    if rest == "atom_stack" {
        return Some(TileSocketKind::AtomStack);
    }

    let (family, coords) = rest.split_once("_slot_")?;
    match family {
        "container_1x1" => {
            let (row, col) = parse_socket_coords(coords)?;
            Some(TileSocketKind::Container1x1 { row, col })
        }
        "container_2x2" => {
            let (row, col) = parse_socket_coords(coords)?;
            Some(TileSocketKind::Container2x2 { row, col })
        }
        "normal_1x1" => {
            let index = coords.parse().ok()?;
            Some(TileSocketKind::Normal1x1 { index })
        }
        "normal_2x2" => {
            let index = coords.parse().ok()?;
            Some(TileSocketKind::Normal2x2 { index })
        }
        _ => None,
    }
}

fn parse_socket_coords(coords: &str) -> Option<(u32, u32)> {
    let (row, col) = coords.split_once('_')?;
    Some((row.parse().ok()?, col.parse().ok()?))
}

/// Board slots each prototype's mesh must span across its larger XZ extent.
fn footprint_slots(kind: TilePrototypeKind) -> f32 {
    match kind {
        TilePrototypeKind::Container2x2 | TilePrototypeKind::Normal2x2 => 2.0,
        TilePrototypeKind::Container1x1
        | TilePrototypeKind::Normal1x1
        | TilePrototypeKind::Atom => 1.0,
    }
}

/// Fit scale + ground lift from the raw mesh AABB.
///
/// The authored meshes carry arbitrary Blender units and scene-layout node
/// transforms; the only stable contract is "this mesh covers N board slots".
fn fit_primitive(
    mesh_handle: &Handle<Mesh>,
    material: Handle<StandardMaterial>,
    kind: TilePrototypeKind,
    meshes: &Assets<Mesh>,
) -> Option<TileMeshPrimitive> {
    use bevy::camera::primitives::MeshAabb;

    use crate::domain::board::SLOT_SIZE;

    let aabb = meshes.get(mesh_handle)?.compute_aabb()?;
    let extent = aabb.half_extents * 2.0;
    let footprint = extent.x.max(extent.z).max(f32::EPSILON);
    let fit_scale = footprint_slots(kind) * SLOT_SIZE / footprint;
    let min_y = aabb.center.y - aabb.half_extents.y;
    Some(TileMeshPrimitive {
        mesh: mesh_handle.clone(),
        material,
        fit_scale,
        ground_lift: -min_y * fit_scale,
    })
}

pub fn finish_tile_mesh_assets_from_gltf(
    assets: &mut TileMeshAssets,
    gltf: &Gltf,
    gltf_nodes: &Assets<GltfNode>,
    gltf_meshes: &Assets<GltfMesh>,
    meshes: &Assets<Mesh>,
) {
    if assets.ready {
        return;
    }

    let mut prototypes = HashMap::<TilePrototypeKind, TileMeshPrototype>::new();

    for (mesh_name, kind) in PROTOTYPE_MESH_NAMES {
        let Some(node_handle) = gltf.named_nodes.get(mesh_name) else {
            continue;
        };
        let Some(node) = gltf_nodes.get(node_handle) else {
            continue;
        };

        let root = node
            .mesh
            .as_ref()
            .and_then(|mesh_handle| gltf_meshes.get(mesh_handle))
            .and_then(|mesh| mesh.primitives.first())
            .and_then(|primitive| {
                primitive
                    .material
                    .clone()
                    .and_then(|material| fit_primitive(&primitive.mesh, material, kind, meshes))
            });

        let mut prototype = TileMeshPrototype {
            root,
            sockets: HashMap::new(),
        };
        collect_socket_transforms(
            node,
            Transform::IDENTITY,
            gltf_nodes,
            &mut prototype.sockets,
        );
        prototypes.insert(kind, prototype);
    }

    assets.container_1x1 = prototypes
        .remove(&TilePrototypeKind::Container1x1)
        .unwrap_or_default();
    assets.container_2x2 = prototypes
        .remove(&TilePrototypeKind::Container2x2)
        .unwrap_or_default();
    assets.normal_1x1 = prototypes
        .remove(&TilePrototypeKind::Normal1x1)
        .unwrap_or_default();
    assets.normal_2x2 = prototypes
        .remove(&TilePrototypeKind::Normal2x2)
        .unwrap_or_default();
    assets.atom = prototypes
        .remove(&TilePrototypeKind::Atom)
        .unwrap_or_default();
    assets.ready = assets.container_2x2.root.is_some() || assets.normal_1x1.root.is_some();
}

fn collect_socket_transforms(
    node: &GltfNode,
    parent: Transform,
    gltf_nodes: &Assets<GltfNode>,
    sockets: &mut HashMap<TileSocketKind, Transform>,
) {
    let local = node.transform;
    let world = parent.mul_transform(local);

    if let Some(kind) = parse_socket_name(&node.name) {
        sockets.insert(kind, world);
    }

    for child in &node.children {
        let Some(child_node) = gltf_nodes.get(child) else {
            continue;
        };
        collect_socket_transforms(child_node, world, gltf_nodes, sockets);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_container_2x2_socket_names() {
        assert_eq!(
            parse_socket_name("SOCKET_container_2x2_slot_002_003"),
            Some(TileSocketKind::Container2x2 { row: 2, col: 3 })
        );
    }

    #[test]
    fn parses_container_1x1_socket_names() {
        assert_eq!(
            parse_socket_name("SOCKET_container_1x1_slot_001_002"),
            Some(TileSocketKind::Container1x1 { row: 1, col: 2 })
        );
    }

    #[test]
    fn parses_normal_socket_names() {
        assert_eq!(
            parse_socket_name("SOCKET_normal_1x1_slot_000"),
            Some(TileSocketKind::Normal1x1 { index: 0 })
        );
        assert_eq!(
            parse_socket_name("SOCKET_normal_2x2_slot_000"),
            Some(TileSocketKind::Normal2x2 { index: 0 })
        );
    }

    #[test]
    fn parses_atom_stack_socket() {
        assert_eq!(
            parse_socket_name("SOCKET_atom_stack"),
            Some(TileSocketKind::AtomStack)
        );
    }

    #[test]
    fn rejects_unknown_socket_names() {
        assert_eq!(parse_socket_name("SOCKET_unknown_slot_000"), None);
        assert_eq!(parse_socket_name("NOT_A_SOCKET"), None);
    }
}
