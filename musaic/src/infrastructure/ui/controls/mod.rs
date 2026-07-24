//! Musaic-specific UI controls (tile palette viewport).
//!
//! Clickable buttons live in [`crate::infrastructure::ui::widgets`].

mod tile_palette;

pub use crate::infrastructure::ui::widgets::{MusaicClickable, musaic_button, musaic_clickable};
pub use tile_palette::{
    TilePaletteViewport, UiTilePaletteCamera, UiTilePaletteDisplay, UiTilePalettePlugin,
    frame_ui_tile_palette_camera, sync_ui_tile_palette_scene, tile_palette_viewport,
};

use bevy::prelude::*;

pub struct MusaicUiControlsPlugin;

impl Plugin for MusaicUiControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiTilePalettePlugin)
            .add_plugins(crate::infrastructure::ui::widgets::MusaicClickablePlugin);
    }
}
