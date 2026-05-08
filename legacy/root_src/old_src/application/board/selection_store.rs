use std::collections::BTreeMap;

use bevy::prelude::Resource;

use crate::domain::board::{BoardSurfaceId, SlotCoord};

#[derive(Debug, Clone, Resource, Default)]
pub struct SelectionStore {
    pub selection_by_board: BTreeMap<BoardSurfaceId, Option<SlotCoord>>,
}

impl SelectionStore {
    pub fn new(root: BoardSurfaceId) -> Self {
        let mut selection_by_board = BTreeMap::new();
        selection_by_board.insert(root, None);
        Self { selection_by_board }
    }

    pub fn selected_slot(&self, board_id: &BoardSurfaceId) -> Option<SlotCoord> {
        self.selection_by_board.get(board_id).copied().flatten()
    }

    pub fn set_selected_slot(&mut self, board_id: BoardSurfaceId, slot: Option<SlotCoord>) {
        self.selection_by_board.insert(board_id, slot);
    }
}
