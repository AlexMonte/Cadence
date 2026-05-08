use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::board::{BoardExtent, SlotOccupancy};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct BoardSurfaceId(pub String);

impl BoardSurfaceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BoardSurfaceKind {
    Flow,
    ContainerLocal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoardSurface {
    pub id: BoardSurfaceId,
    pub kind: BoardSurfaceKind,
    pub extent: BoardExtent,
    #[serde(default)]
    pub occupancy: SlotOccupancy,
}

impl BoardSurface {
    pub fn new(id: BoardSurfaceId, kind: BoardSurfaceKind, extent: BoardExtent) -> Self {
        Self {
            id,
            kind,
            extent,
            occupancy: SlotOccupancy::default(),
        }
    }
}
