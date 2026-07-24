//! Raw board pick events — infrastructure emits, the classifier turns them
//! into [`EditorCommand`](crate::application::command::EditorCommand)s.

use bevy::prelude::{Message, Vec3};
use tessera::prelude::{NodeId, SpatialSide};

use crate::domain::board::{BoardSlot, BoardSurfaceId};
use crate::domain::document::StackIndex;

#[derive(Message, Clone, Debug)]
pub enum BoardPickEvent {
    Hit(BoardPickHit),
    Miss,
}

#[derive(Clone, Debug)]
pub struct BoardPickHit {
    pub surface_id: BoardSurfaceId,
    pub kind: BoardPickTargetKind,
    pub world_position: Vec3,
    pub distance: f32,
}

#[derive(Clone, Debug)]
pub enum BoardPickTargetKind {
    Slot { slot: BoardSlot },
    StackInsert { index: StackIndex },
    BoardTile { tile_id: NodeId },
    StackTile { tile_id: NodeId },
    Connection { from: NodeId, to: NodeId },
    PortSide { tile_id: NodeId, side: SpatialSide },
}
