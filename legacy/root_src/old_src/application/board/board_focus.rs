use std::collections::BTreeMap;

use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::domain::board::BoardSurfaceId;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct BoardSpaceVec2 {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SlotLayout {
    pub slot_size: BoardSpaceVec2,
    pub slot_stride: BoardSpaceVec2,
    pub origin: BoardSpaceVec2,
}

impl Default for SlotLayout {
    fn default() -> Self {
        Self {
            slot_size: BoardSpaceVec2 { x: 116.0, y: 72.0 },
            slot_stride: BoardSpaceVec2 { x: 116.0, y: 72.0 },
            origin: BoardSpaceVec2 { x: 0.0, y: 0.0 },
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ViewportState {
    pub pan: BoardSpaceVec2,
    pub zoom: f32,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            pan: BoardSpaceVec2 { x: 0.0, y: 0.0 },
            zoom: 1.0,
        }
    }
}

#[derive(Debug, Clone, Resource, Serialize, Deserialize, PartialEq)]
pub struct BoardFocus {
    pub active_board: BoardSurfaceId,
    pub focus_path: Vec<BoardSurfaceId>,
    pub viewport_by_board: BTreeMap<BoardSurfaceId, ViewportState>,
    pub layout_by_board: BTreeMap<BoardSurfaceId, SlotLayout>,
}

impl BoardFocus {
    pub fn new(root: BoardSurfaceId) -> Self {
        let mut viewport_by_board = BTreeMap::new();
        viewport_by_board.insert(root.clone(), ViewportState::default());

        let mut layout_by_board = BTreeMap::new();
        layout_by_board.insert(root.clone(), SlotLayout::default());

        Self {
            active_board: root.clone(),
            focus_path: vec![root],
            viewport_by_board,
            layout_by_board,
        }
    }

    pub fn active_viewport(&self) -> ViewportState {
        self.viewport_by_board
            .get(&self.active_board)
            .copied()
            .unwrap_or_default()
    }

    pub fn active_layout(&self) -> SlotLayout {
        self.layout_by_board
            .get(&self.active_board)
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BoardPointer {
    pub board_id: BoardSurfaceId,
    pub board_position: BoardSpaceVec2,
}
