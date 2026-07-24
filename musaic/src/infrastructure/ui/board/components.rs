use bevy::prelude::*;
use tessera::prelude::NodeId;

use crate::application::editor::interaction::BoardPickTargetKind;
use crate::application::pipeline::scene_sync::{VisibleBoardNode, VisibleNodeKind};
use crate::domain::board::{BoardSlot, BoardSurfaceId};
use crate::domain::document::PlacementAddress;

#[derive(Component)]
pub struct Board3dRoot;

/// Persistent infinite-board backdrop: pick slab + grid lines that follow the
/// camera in `SLOT_SIZE` snaps so the root board reads as an endless surface.
#[derive(Component)]
pub struct BoardGridAnchor;

/// The large pickable plane under the grid (click → slot resolution).
#[derive(Component)]
pub struct BoardGridPickSlab;

#[derive(Component)]
pub(super) struct Board3dLight;

#[derive(Component, Clone, Copy)]
pub(super) struct BoardDragSurface {
    pub(super) surface: BoardSurfaceId,
}

#[derive(Component)]
pub struct Board3dCamera;

pub type BoardTileId = NodeId;

#[derive(Component, Clone, Debug)]
pub struct BoardPickTarget {
    pub surface_id: BoardSurfaceId,
    pub kind: BoardPickTargetKind,
}

#[derive(Component)]
pub struct Board3dTile {
    pub surface: BoardSurfaceId,
    pub node: NodeId,
    pub address: PlacementAddress,
}

#[derive(Component)]
pub(super) struct BoardDragPreview;

#[derive(Component)]
pub(super) struct BoardConnectionPreview;

#[derive(Component)]
pub(super) struct BoardSlotHighlight;

#[derive(Component)]
pub(super) struct TilePlacementPulse {
    pub(super) elapsed: f32,
    pub(super) base_scale: Vec3,
}

#[derive(Component)]
pub(super) struct StackInsertEntity;

#[derive(Component)]
pub(super) struct StackLockedEntity;

#[derive(Component)]
pub(super) struct ConnectionLineEntity;

/// Per-tile visual identity for keyed board reconcile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TileVisualKey {
    address: PlacementAddress,
    kind: VisibleNodeKind,
    selected: bool,
    focused: bool,
    footprint: Option<(u32, u32)>,
    surface_display: Option<String>,
    icon: Option<crate::adapter::tile_icons::TileIconId>,
}

impl TileVisualKey {
    pub(super) fn from_node(node: &VisibleBoardNode) -> Self {
        Self {
            address: node.address,
            kind: node.kind,
            selected: node.selected,
            focused: node.focused,
            footprint: node.tessera_footprint.map(|f| (f.width, f.height)),
            surface_display: node.surface_content.display(),
            icon: node.icon,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ConnectionVisualKey {
    pub(super) from_slot: BoardSlot,
    pub(super) to_slot: BoardSlot,
    pub(super) kind: crate::domain::document::PortSlotState,
}

