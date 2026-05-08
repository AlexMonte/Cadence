use bevy::prelude::Resource;

use crate::domain::board::{BoardSurfaceId, PlacementVerdict, SlotCoord, TileClass, TileInstanceId};

use super::{BoardPointer, BoardSpaceVec2};

#[derive(Debug, Clone)]
pub enum DragSource {
    Palette(String),
    BoardTile(TileInstanceId),
}

#[derive(Debug, Clone)]
pub enum DragPayload {
    NewPiece { piece_id: String, class: TileClass },
    ExistingTile { tile_id: TileInstanceId, class: TileClass },
}

#[derive(Debug, Clone)]
pub struct ResolvedTarget {
    pub board_id: BoardSurfaceId,
    pub slot_coord: SlotCoord,
    pub validity: PlacementVerdict,
}

#[derive(Debug, Clone)]
pub struct DragSession {
    pub source: DragSource,
    pub payload: DragPayload,
    pub origin: BoardSurfaceId,
    pub current_pointer: Option<BoardPointer>,
    pub resolved_target: Option<ResolvedTarget>,
    pub preview_offset: Option<BoardSpaceVec2>,
}

#[derive(Debug, Clone, Default)]
pub enum BoardInteraction {
    #[default]
    Idle,
    HoveringSlot {
        board_id: BoardSurfaceId,
        slot: SlotCoord,
    },
    Dragging(DragSession),
    OpeningContainer {
        tile_id: TileInstanceId,
    },
}

#[derive(Debug, Clone, Resource, Default)]
pub struct InteractionStore {
    pub interaction: BoardInteraction,
}
