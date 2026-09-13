//! Musaic-specific UI controls, including the native categorized tile library.
//!
//! Clickable buttons live in [`crate::infrastructure::ui::widgets`].

pub(crate) mod tile_palette;

pub use crate::infrastructure::ui::widgets::{MusaicClickable, musaic_button};
pub(crate) use tile_palette::sync_ui_tile_palette_scene;
pub use tile_palette::{UiTilePaletteDisplay, UiTilePalettePlugin};

use bevy::prelude::*;

pub struct MusaicUiControlsPlugin;

impl Plugin for MusaicUiControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiTilePalettePlugin)
            .add_plugins(crate::infrastructure::ui::widgets::MusaicSliderPlugin)
            .add_plugins(crate::infrastructure::ui::widgets::MusaicClickablePlugin);
    }
}

pub(crate) mod library_collections;
pub(crate) mod library_search;
