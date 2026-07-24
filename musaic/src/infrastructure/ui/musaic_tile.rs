//! Shared ortho-tile visuals and picking for the board and inspector palette.
//!
//! Tiles are 3D mesh entities with pointer observers — not UI buttons.

use bevy::{math::primitives::Cuboid, prelude::*};

use crate::{application::pipeline::scene_sync::VisibleNodeKind, domain::document::TileSpawnKind};

use crate::adapter::tile_icons::TileIconId;

use super::{
    render_layers::SCENE_NODE_VISIBILITY,
    tile_icons::{TileIconAssets, spawn_tile_icon},
    transform_tile::{
        TileSheetKind, TransformTileAssets, TransformTileBase, TransformTileIcon, frames,
        tile_sheet_for_spawn, tile_sheet_for_visible,
    },
};

/// Tile mesh size in world units (32×4×32 pixel proportions).
pub const TILE_WORLD_WIDTH: f32 = 0.32;
pub const TILE_WORLD_HEIGHT: f32 = 0.04;
pub const TILE_WORLD_DEPTH: f32 = 0.32;

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

/// Spawns an ortho-sheet quad facing the board/palette camera (+Y normal).
pub fn spawn_ortho_tile_visual(
    parent: &mut ChildSpawnerCommands<'_>,
    assets: &TransformTileAssets,
    icons: &TileIconAssets,
    position: Vec3,
    spec: TileVisualSpec,
    extra: impl Bundle,
    icon_extra: impl Bundle,
) {
    if !assets.ready {
        return;
    }

    let mesh = assets.meshes.for_frame(spec.frame);
    let materials = assets.materials(spec.sheet);
    let material = match spec.frame {
        frames::PLAIN => materials.plain.clone(),
        frames::HIGHLIGHT => materials.highlight.clone(),
        _ => materials.framed.clone(),
    };

    parent
        .spawn((
            extra,
            TransformTileBase,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            SCENE_NODE_VISIBILITY,
            Transform::from_translation(position).with_scale(spec.scale()),
        ))
        .with_children(|tile| {
            if let Some(icon) = spec.icon {
                tile.spawn(TransformTileIcon);
                spawn_tile_icon(tile, icons, icon, icon_extra);
            }
        });
}

/// Loading placeholder before ortho sheets are ready.
pub fn spawn_tile_placeholder(
    parent: &mut ChildSpawnerCommands<'_>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    position: Vec3,
    spec: TileVisualSpec,
    extra: impl Bundle,
) {
    parent.spawn((
        extra,
        Mesh3d(meshes.add(Cuboid::new(
            TILE_WORLD_WIDTH,
            TILE_WORLD_HEIGHT,
            TILE_WORLD_DEPTH,
        ))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.22, 0.24, 0.30),
            ..default()
        })),
        SCENE_NODE_VISIBILITY,
        Transform::from_translation(position).with_scale(spec.scale()),
        Pickable::default(),
    ));
}
