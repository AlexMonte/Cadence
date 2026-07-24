//! Shared tile visual specs and footprint constants for board and palette.
//!
//! Mesh spawning lives in [`super::tile_visual`] (`spawn_board_tile`).

use bevy::prelude::*;

use crate::{application::pipeline::scene_sync::VisibleNodeKind, domain::document::TileSpawnKind};

use crate::adapter::tile_icons::TileIconId;

use super::transform_tile::{
    TileSheetKind, frames, tile_sheet_for_spawn, tile_sheet_for_visible,
};

/// Tile mesh size in world units (32×4×32 pixel proportions).
pub const TILE_WORLD_WIDTH: f32 = 0.32;
pub const TILE_WORLD_HEIGHT: f32 = 0.04;
pub const TILE_WORLD_DEPTH: f32 = 0.32;

/// Inset from slot edge for highlight rings and legacy footprint math.
pub const TILE_INSET: f32 = 0.18;

/// Solid-marker height for stack insert / locked chrome (not the GLTF tile body).
pub const TILE_HEIGHT: f32 = 0.22;

/// Palette drawer tiles (larger than board slot tiles).
pub const PALETTE_TILE_FOOTPRINT: f32 = 2.15;

/// Smaller footprint for atom tiles in the container palette.
pub const ATOM_TILE_FOOTPRINT: f32 = 1.72;

/// Root board macro tiles (containers, outputs).
pub const BOARD_MACRO_TILE_FOOTPRINT: f32 = 0.68;
/// Root board transform / trick tiles.
pub const BOARD_TRICK_TILE_FOOTPRINT: f32 = 0.58;
/// Atoms placed on a container stack track.
pub const STACK_ATOM_FOOTPRINT: f32 = 0.46;
/// Macro pieces on a stack track (rare; same slot width as board).
pub const STACK_MACRO_FOOTPRINT: f32 = 0.62;

pub fn board_footprint_for_kind(kind: VisibleNodeKind) -> f32 {
    match kind {
        VisibleNodeKind::Container | VisibleNodeKind::Output => BOARD_MACRO_TILE_FOOTPRINT,
        VisibleNodeKind::TrickInstance | VisibleNodeKind::Tile => BOARD_TRICK_TILE_FOOTPRINT,
        VisibleNodeKind::Atom => STACK_ATOM_FOOTPRINT,
    }
}

pub fn stack_footprint_for_kind(kind: VisibleNodeKind) -> f32 {
    match kind {
        VisibleNodeKind::Atom => STACK_ATOM_FOOTPRINT,
        VisibleNodeKind::Container | VisibleNodeKind::Output => STACK_MACRO_FOOTPRINT,
        VisibleNodeKind::TrickInstance | VisibleNodeKind::Tile => BOARD_TRICK_TILE_FOOTPRINT,
    }
}

/// Uniform scale so footprint matches `TILE_WORLD_WIDTH` on X/Z.
pub fn tile_scale_for_footprint(footprint: f32) -> Vec3 {
    let s = footprint / TILE_WORLD_WIDTH;
    Vec3::splat(s)
}

pub fn tile_center_y_for_footprint(footprint: f32, ground_y: f32) -> f32 {
    let scale = footprint / TILE_WORLD_WIDTH;
    ground_y + TILE_WORLD_HEIGHT * scale * 0.5
}

#[derive(Debug, Clone, Copy)]
pub struct TileVisualSpec {
    pub sheet: TileSheetKind,
    pub frame: u32,
    pub footprint: f32,
    pub icon: Option<TileIconId>,
}

impl TileVisualSpec {
    pub fn scale(self) -> Vec3 {
        tile_scale_for_footprint(self.footprint)
    }
}

pub fn tile_visual_for_spawn(spawn: &TileSpawnKind, highlighted: bool) -> Option<TileVisualSpec> {
    if let TileSpawnKind::Atom { atom } = spawn {
        return Some(TileVisualSpec {
            sheet: super::transform_tile::tile_sheet_for_atom(atom),
            frame: if highlighted {
                frames::HIGHLIGHT
            } else {
                frames::FRAMED
            },
            footprint: ATOM_TILE_FOOTPRINT,
            icon: None,
        });
    }

    let sheet = tile_sheet_for_spawn(spawn)?;
    Some(TileVisualSpec {
        sheet,
        frame: if highlighted {
            frames::HIGHLIGHT
        } else {
            frames::FRAMED
        },
        footprint: PALETTE_TILE_FOOTPRINT,
        icon: crate::adapter::tile_icons::icon_for_spawn(spawn),
    })
}

pub fn tile_visual_for_visible(
    kind: VisibleNodeKind,
    selected: bool,
    focused: bool,
    icon: Option<TileIconId>,
) -> Option<TileVisualSpec> {
    let sheet = tile_sheet_for_visible(kind)?;
    let highlighted = selected || focused;
    Some(TileVisualSpec {
        sheet,
        frame: if highlighted {
            frames::HIGHLIGHT
        } else {
            frames::FRAMED
        },
        footprint: board_footprint_for_kind(kind),
        icon,
    })
}
