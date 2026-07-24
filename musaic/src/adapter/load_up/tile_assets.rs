//! Board / palette tile images under `assets/tiles/`.

use bevy::gltf::Gltf;
use bevy::prelude::*;
use load_up::LoadAssetResource;

/// Horizontal ortho strips (32×32 frames) for container / output / transform shells.
#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
pub struct OrthoTileAssets {
    #[dependency(path = "tiles/transform_Ortho_32x32.png")]
    pub transform_ortho: Handle<Image>,
    #[dependency(path = "tiles/container_Ortho_32x32.png")]
    pub container_ortho: Handle<Image>,
    #[dependency(path = "tiles/output_Ortho_32x32.png")]
    pub output_ortho: Handle<Image>,
    #[dependency(path = "tiles/atom_note_Ortho_32x32.png")]
    pub atom_note_ortho: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_Ortho_32x32.png")]
    pub atom_scalar_ortho: Handle<Image>,
    #[dependency(path = "tiles/atom_accidental_Ortho_32x32.png")]
    pub atom_accidental_ortho: Handle<Image>,
    #[dependency(path = "tiles/atom_operator_Ortho_32x32.png")]
    pub atom_operator_ortho: Handle<Image>,
}

/// Center icons on transform / trick tiles and drawer previews.
#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
pub struct BoardTileIconAssets {
    #[dependency(path = "tiles/container_sequence.png")]
    pub container_sequence: Handle<Image>,
    #[dependency(path = "tiles/container_parallel.png")]
    pub container_parallel: Handle<Image>,
    #[dependency(path = "tiles/container_alternate.png")]
    pub container_alternate: Handle<Image>,
    #[dependency(path = "tiles/resources/output_tile_base_top.png")]
    pub output_tile: Handle<Image>,
    #[dependency(path = "tiles/transform_fast.png")]
    pub transform_fast: Handle<Image>,
    #[dependency(path = "tiles/transform_slow.png")]
    pub transform_slow: Handle<Image>,
    #[dependency(path = "tiles/transform_legato.png")]
    pub transform_legato: Handle<Image>,
    #[dependency(path = "tiles/transform_gain.png")]
    pub transform_gain: Handle<Image>,
    /// 7x5 grid of 32px glyphs; cells addressed via UV transform (e.g. the
    /// Subdivision "?" glyph, which has no standalone PNG).
    #[dependency(path = "tiles/drawer_tiles.png")]
    pub glyph_atlas: Handle<Image>,
}

/// Grid size of `glyph_atlas` (`tiles/drawer_tiles.png`).
pub const GLYPH_ATLAS_COLS: u32 = 7;
pub const GLYPH_ATLAS_ROWS: u32 = 5;
/// `glyph_atlas` cell (column, row) of the Subdivision "?" glyph.
pub const GLYPH_CELL_SUBDIVISION: (u32, u32) = (5, 3);

/// Atom center icons on the 3D board.
#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
pub struct AtomTileAssets {
    #[dependency(path = "tiles/atom_note_a.png")]
    pub note_a: Handle<Image>,
    #[dependency(path = "tiles/atom_note_b.png")]
    pub note_b: Handle<Image>,
    #[dependency(path = "tiles/atom_note_c.png")]
    pub note_c: Handle<Image>,
    #[dependency(path = "tiles/atom_note_d.png")]
    pub note_d: Handle<Image>,
    #[dependency(path = "tiles/atom_note_e.png")]
    pub note_e: Handle<Image>,
    #[dependency(path = "tiles/atom_note_f.png")]
    pub note_f: Handle<Image>,
    #[dependency(path = "tiles/atom_note_g.png")]
    pub note_g: Handle<Image>,
    #[dependency(path = "tiles/atom_note_silence.png")]
    pub note_silence: Handle<Image>,
    #[dependency(path = "tiles/atom_accidental_sharp.png")]
    pub accidental_sharp: Handle<Image>,
    #[dependency(path = "tiles/atom_accidental_flat.png")]
    pub accidental_flat: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_0.png")]
    pub scalar_0: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_1.png")]
    pub scalar_1: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_2.png")]
    pub scalar_2: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_3.png")]
    pub scalar_3: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_4.png")]
    pub scalar_4: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_5.png")]
    pub scalar_5: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_6.png")]
    pub scalar_6: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_7.png")]
    pub scalar_7: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_8.png")]
    pub scalar_8: Handle<Image>,
    #[dependency(path = "tiles/atom_scalar_9.png")]
    pub scalar_9: Handle<Image>,
    #[dependency(path = "tiles/atom_operator_elongation.png")]
    pub operator_elongation: Handle<Image>,
    #[dependency(path = "tiles/atom_operator_randomness.png")]
    pub operator_randomness: Handle<Image>,
    #[dependency(path = "tiles/atom_operator_replication.png")]
    pub operator_replication: Handle<Image>,
    #[dependency(path = "tiles/atom_operator_multiply.png")]
    pub operator_multiply: Handle<Image>,
    #[dependency(path = "tiles/atom_operator_divide.png")]
    pub operator_divide: Handle<Image>,
}

/// GLTF tile model loaded once via load_up.
#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
pub struct TileMeshSourceAssets {
    #[dependency(path = "tiles/models/musaic_tiles_model.gltf")]
    pub model: Handle<Gltf>,
}

impl AtomTileAssets {
    pub fn all_handles(&self) -> Vec<Handle<Image>> {
        vec![
            self.note_a.clone(),
            self.note_b.clone(),
            self.note_c.clone(),
            self.note_d.clone(),
            self.note_e.clone(),
            self.note_f.clone(),
            self.note_g.clone(),
            self.note_silence.clone(),
            self.accidental_sharp.clone(),
            self.accidental_flat.clone(),
            self.scalar_0.clone(),
            self.scalar_1.clone(),
            self.scalar_2.clone(),
            self.scalar_3.clone(),
            self.scalar_4.clone(),
            self.scalar_5.clone(),
            self.scalar_6.clone(),
            self.scalar_7.clone(),
            self.scalar_8.clone(),
            self.scalar_9.clone(),
            self.operator_elongation.clone(),
            self.operator_randomness.clone(),
            self.operator_replication.clone(),
        ]
    }
}
