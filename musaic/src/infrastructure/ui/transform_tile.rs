//! Pixel-art orthographic tile sheets (32×32 frames in a 6-frame horizontal strip).
//!
//! Sheets: `transform_Ortho_32x32`, `container_Ortho_32x32`, `output_Ortho_32x32`.
//! Frame indices are shared across sheets (see [`frames`]).

use bevy::{
    math::{Affine2, Mat2, primitives::Cuboid},
    prelude::*,
};

use crate::{
    application::pipeline::scene_sync::VisibleNodeKind,
    domain::document::{AtomValue, TileSpawnKind},
};

pub const TILE_FRAME_PX: u32 = 32;
pub const TILE_FRAME_COUNT: u32 = 6;

pub const TRANSFORM_ORTHO_SHEET: &str = "tiles/transform_Ortho_32x32.png";
pub const CONTAINER_ORTHO_SHEET: &str = "tiles/container_Ortho_32x32.png";
pub const OUTPUT_ORTHO_SHEET: &str = "tiles/output_Ortho_32x32.png";
pub const ATOM_NOTE_ORTHO_SHEET: &str = "tiles/atom_note_Ortho_32x32.png";
pub const ATOM_SCALAR_ORTHO_SHEET: &str = "tiles/atom_scalar_Ortho_32x32.png";
pub const ATOM_ACCIDENTAL_ORTHO_SHEET: &str = "tiles/atom_accidental_Ortho_32x32.png";
pub const ATOM_OPERATOR_ORTHO_SHEET: &str = "tiles/atom_operator_Ortho_32x32.png";

/// Frame indices shared by all ortho tile sheets (left → right).
pub mod frames {
    /// Dark empty / slot base (frames 0–3 are visually similar).
    pub const PLAIN: u32 = 0;
    /// Default framed tile — resting state.
    pub const FRAMED: u32 = 4;
    /// Highlight — selected / focused / armed.
    pub const HIGHLIGHT: u32 = 5;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileSheetKind {
    Transform,
    Container,
    Output,
    AtomNote,
    AtomScalar,
    AtomAccidental,
    AtomOperator,
}

impl TileSheetKind {
    pub const fn asset_path(self) -> &'static str {
        match self {
            Self::Transform => TRANSFORM_ORTHO_SHEET,
            Self::Container => CONTAINER_ORTHO_SHEET,
            Self::Output => OUTPUT_ORTHO_SHEET,
            Self::AtomNote => ATOM_NOTE_ORTHO_SHEET,
            Self::AtomScalar => ATOM_SCALAR_ORTHO_SHEET,
            Self::AtomAccidental => ATOM_ACCIDENTAL_ORTHO_SHEET,
            Self::AtomOperator => ATOM_OPERATOR_ORTHO_SHEET,
        }
    }
}

pub fn tile_sheet_for_atom(atom: &AtomValue) -> TileSheetKind {
    match atom {
        AtomValue::NoteName(_) | AtomValue::Rest => TileSheetKind::AtomNote,
        AtomValue::Accidental(_) => TileSheetKind::AtomAccidental,
        AtomValue::Octave(_) | AtomValue::Number(_) => TileSheetKind::AtomScalar,
        AtomValue::Operator(_) => TileSheetKind::AtomOperator,
    }
}

pub fn tile_sheet_for_visible(kind: VisibleNodeKind) -> Option<TileSheetKind> {
    match kind {
        VisibleNodeKind::Container => Some(TileSheetKind::Container),
        VisibleNodeKind::Output => Some(TileSheetKind::Output),
        VisibleNodeKind::TrickInstance => Some(TileSheetKind::Transform),
        VisibleNodeKind::Tile | VisibleNodeKind::Atom => None,
    }
}

pub fn tile_sheet_for_spawn(spawn: &TileSpawnKind) -> Option<TileSheetKind> {
    match spawn {
        TileSpawnKind::Container { .. } => Some(TileSheetKind::Container),
        TileSpawnKind::Output { .. } => Some(TileSheetKind::Output),
        TileSpawnKind::TrickInstance { .. } => Some(TileSheetKind::Transform),
        TileSpawnKind::Tile { .. } | TileSpawnKind::Atom { .. } => None,
    }
}

#[derive(Component)]
pub struct TransformTileBase;

/// Child layer reserved for per-tile icons (populated later).
#[derive(Component)]
pub struct TransformTileIcon;

#[derive(Clone, Default)]
pub struct TileSheetMaterials {
    pub plain: Handle<StandardMaterial>,
    pub framed: Handle<StandardMaterial>,
    pub highlight: Handle<StandardMaterial>,
}

#[derive(Default, Clone)]
pub struct TransformTileMeshes {
    pub plain: Handle<Mesh>,
    pub framed: Handle<Mesh>,
    pub highlight: Handle<Mesh>,
}

impl TransformTileMeshes {
    pub fn for_frame(&self, frame_index: u32) -> Handle<Mesh> {
        match frame_index {
            frames::PLAIN => self.plain.clone(),
            frames::HIGHLIGHT => self.highlight.clone(),
            _ => self.framed.clone(),
        }
    }
}

#[derive(Resource, Default)]
pub struct TransformTileAssets {
    pub ready: bool,
    pub meshes: TransformTileMeshes,
    pub transform: TileSheetMaterials,
    pub container: TileSheetMaterials,
    pub output: TileSheetMaterials,
    pub atom_note: TileSheetMaterials,
    pub atom_scalar: TileSheetMaterials,
    pub atom_accidental: TileSheetMaterials,
    pub atom_operator: TileSheetMaterials,
    transform_texture: Handle<Image>,
    container_texture: Handle<Image>,
    output_texture: Handle<Image>,
    atom_note_texture: Handle<Image>,
    atom_scalar_texture: Handle<Image>,
    atom_accidental_texture: Handle<Image>,
    atom_operator_texture: Handle<Image>,
}

impl TransformTileAssets {
    pub fn mesh_and_material(
        &self,
        kind: TileSheetKind,
        selected: bool,
        focused: bool,
    ) -> Option<(Handle<Mesh>, Handle<StandardMaterial>)> {
        if !self.ready {
            return None;
        }

        let materials = self.materials(kind);
        if focused || selected {
            Some((self.meshes.highlight.clone(), materials.highlight.clone()))
        } else {
            Some((self.meshes.framed.clone(), materials.framed.clone()))
        }
    }

    pub fn materials(&self, kind: TileSheetKind) -> &TileSheetMaterials {
        match kind {
            TileSheetKind::Transform => &self.transform,
            TileSheetKind::Container => &self.container,
            TileSheetKind::Output => &self.output,
            TileSheetKind::AtomNote => &self.atom_note,
            TileSheetKind::AtomScalar => &self.atom_scalar,
            TileSheetKind::AtomAccidental => &self.atom_accidental,
            TileSheetKind::AtomOperator => &self.atom_operator,
        }
    }
}

pub fn finish_transform_tile_assets_from_handles(
    assets: &mut TransformTileAssets,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    transform_texture: Handle<Image>,
    container_texture: Handle<Image>,
    output_texture: Handle<Image>,
    atom_note_texture: Handle<Image>,
    atom_scalar_texture: Handle<Image>,
    atom_accidental_texture: Handle<Image>,
    atom_operator_texture: Handle<Image>,
) {
    if assets.ready {
        return;
    }

    let tile_mesh = Mesh::from(Cuboid::new(
        crate::infrastructure::ui::musaic_tile::TILE_WORLD_WIDTH,
        crate::infrastructure::ui::musaic_tile::TILE_WORLD_HEIGHT,
        crate::infrastructure::ui::musaic_tile::TILE_WORLD_DEPTH,
    ));
    assets.meshes.plain = meshes.add(tile_mesh.clone());
    assets.meshes.framed = meshes.add(tile_mesh.clone());
    assets.meshes.highlight = meshes.add(tile_mesh);

    assets.transform_texture = transform_texture.clone();
    assets.container_texture = container_texture.clone();
    assets.output_texture = output_texture.clone();
    assets.atom_note_texture = atom_note_texture.clone();
    assets.atom_scalar_texture = atom_scalar_texture.clone();
    assets.atom_accidental_texture = atom_accidental_texture.clone();
    assets.atom_operator_texture = atom_operator_texture.clone();

    assets.transform = build_sheet_materials(materials, transform_texture);
    assets.container = build_sheet_materials(materials, container_texture);
    assets.output = build_sheet_materials(materials, output_texture);
    assets.atom_note = build_sheet_materials(materials, atom_note_texture);
    assets.atom_scalar = build_sheet_materials(materials, atom_scalar_texture);
    assets.atom_accidental = build_sheet_materials(materials, atom_accidental_texture);
    assets.atom_operator = build_sheet_materials(materials, atom_operator_texture);

    assets.ready = true;
}

fn build_sheet_materials(
    materials: &mut Assets<StandardMaterial>,
    texture: Handle<Image>,
) -> TileSheetMaterials {
    let mut make = |frame_index: u32| {
        let (u0, u1) = atlas_uv_bounds(frame_index);
        let scale_u = u1 - u0;
        materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(texture.clone()),
            uv_transform: Affine2 {
                matrix2: Mat2::from_diagonal(Vec2::new(scale_u, -1.0)),
                translation: Vec2::new(u0, 1.0),
            },
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            cull_mode: None,
            ..default()
        })
    };

    TileSheetMaterials {
        plain: make(frames::PLAIN),
        framed: make(frames::FRAMED),
        highlight: make(frames::HIGHLIGHT),
    }
}

/// Pixel region of one frame in an ortho tile sheet.
pub fn frame_pixel_rect(frame_index: u32) -> Rect {
    let frame = frame_index.min(TILE_FRAME_COUNT - 1);
    let x = frame as f32 * TILE_FRAME_PX as f32;
    let size = TILE_FRAME_PX as f32;
    Rect {
        min: Vec2::new(x, 0.0),
        max: Vec2::new(x + size, size),
    }
}

pub fn atlas_uv_bounds(frame_index: u32) -> (f32, f32) {
    let cols = TILE_FRAME_COUNT as f32;
    let frame = frame_index.min(TILE_FRAME_COUNT - 1) as f32;
    (frame / cols, (frame + 1.0) / cols)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_uv_bounds_for_framed_frame() {
        let (u0, u1) = atlas_uv_bounds(frames::FRAMED);
        assert!((u0 - 4.0 / 6.0).abs() < f32::EPSILON);
        assert!((u1 - 5.0 / 6.0).abs() < f32::EPSILON);
    }

    #[test]
    fn tile_sheet_for_spawn_maps_macro_tiles() {
        assert_eq!(
            tile_sheet_for_spawn(&TileSpawnKind::Container {
                kind: crate::domain::document::ContainerKind::Sequence
            }),
            Some(TileSheetKind::Container)
        );
        assert_eq!(
            tile_sheet_for_spawn(&TileSpawnKind::Output {
                name: "main".into()
            }),
            Some(TileSheetKind::Output)
        );
        assert_eq!(
            tile_sheet_for_spawn(&TileSpawnKind::TrickInstance {
                prototype: crate::domain::document::GraphTilePrototypeId(0)
            }),
            Some(TileSheetKind::Transform)
        );
        assert!(
            tile_sheet_for_spawn(&TileSpawnKind::Atom {
                atom: crate::domain::document::AtomValue::Rest
            })
            .is_none()
        );
    }
}
