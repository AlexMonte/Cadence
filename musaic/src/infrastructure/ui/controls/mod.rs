//! Musaic-specific Feathers-style UI templates (extensions not yet in bevy_feathers).

mod clickable;
mod tile_palette;

pub use clickable::{MusaicClickable, musaic_clickable};
pub use tile_palette::{
    TilePaletteViewport, UiTilePaletteCamera, UiTilePaletteDisplay, UiTilePalettePlugin,
    frame_ui_tile_palette_camera, sync_ui_tile_palette_scene, tile_palette_viewport,
};

use bevy::prelude::*;

pub struct MusaicUiControlsPlugin;

impl Plugin for MusaicUiControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiTilePalettePlugin)
            .add_plugins(clickable::MusaicClickablePlugin);
    }
}
