use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use tessera::prelude::ContainerId;

use crate::selection::SelectionStore;

#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoardState {
    pub active_surface: BoardSurfaceId,
    pub surfaces: BTreeMap<BoardSurfaceId, BoardViewState>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoardViewState {
    pub camera: BoardCameraState,
    pub grid: GridSettings,
    pub visible_region: BoardRect,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BoardSurfaceId {
    #[default]
    Root,
    Container(ContainerId),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BoardCameraState {
    pub center: BoardPoint,
    pub zoom: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GridSettings {
    pub tile_size: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BoardRect {
    pub min: BoardPoint,
    pub max: BoardPoint,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct BoardPoint {
    pub x: f32,
    pub y: f32,
}

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoardState>()
            .init_resource::<SelectionStore>();
    }
}
