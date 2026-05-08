use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::board::surface::BoardSurfaceId;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct TileInstanceId(pub String);

impl TileInstanceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TileClass {
    Container,
    Transform,
    Atom,
    Terminal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TileInstance {
    pub id: TileInstanceId,
    pub piece_id: String,
    pub class: TileClass,
    pub parent_surface: BoardSurfaceId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_surface_id: Option<BoardSurfaceId>,
}
