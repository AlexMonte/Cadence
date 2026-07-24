//! load_up asset packs (per-image sprites until atlases land).

pub mod atom_lookup;
pub mod sampler;
pub mod tile_assets;
pub mod ui_assets;

mod plugin;

pub use plugin::LoadUpMusaicPlugin;
pub use sampler::{
    NearestSamplerApplied, PIXEL_TILE_ICON_SCALE, PIXEL_UI_SCALE, ensure_nearest_sampler,
    ensure_nearest_samplers, pixel_ui_display_size,
};
pub use tile_assets::{
    AtomTileAssets, BoardTileIconAssets, GLYPH_ATLAS_COLS, GLYPH_ATLAS_ROWS,
    GLYPH_CELL_SUBDIVISION, OrthoTileAssets, TileMeshSourceAssets,
};
pub use ui_assets::UiSpriteAssets;
