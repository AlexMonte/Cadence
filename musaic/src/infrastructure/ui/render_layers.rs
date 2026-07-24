//! Isolate board and palette 3D scenes so they only appear in their viewports.

use bevy::{camera::visibility::RenderLayers, prelude::*};

/// Main compose board (`Board3dCamera` → center viewport).
pub const BOARD_VIEW: RenderLayers = RenderLayers::layer(0);

/// Inspector tile drawer (`UiTilePaletteCamera` → palette viewport).
pub const PALETTE_VIEW: RenderLayers = RenderLayers::layer(1);

/// World-space offset for the palette scene (keeps it away from the board origin).
pub const PALETTE_WORLD_ORIGIN: Vec3 = Vec3::new(256.0, 0.0, 0.0);

/// Root entity for an isolated 3D scene (board or palette).
pub const SCENE_ROOT_VISIBILITY: Visibility = Visibility::Visible;

/// Child entities in an isolated 3D scene — keeps `InheritedVisibility` hierarchy valid (B0004).
pub const SCENE_NODE_VISIBILITY: Visibility = Visibility::Inherited;
