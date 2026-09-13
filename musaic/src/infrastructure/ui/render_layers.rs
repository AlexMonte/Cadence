//! Isolate the board scene so it only appears in its viewport.

use bevy::{camera::visibility::RenderLayers, prelude::*};

/// Main compose board (`Board3dCamera` → center viewport).
pub const BOARD_VIEW: RenderLayers = RenderLayers::layer(0);

/// Root entity for the isolated 3D board scene.
pub const SCENE_ROOT_VISIBILITY: Visibility = Visibility::Visible;

/// Child entities in an isolated 3D scene — keeps `InheritedVisibility` hierarchy valid (B0004).
pub const SCENE_NODE_VISIBILITY: Visibility = Visibility::Inherited;
