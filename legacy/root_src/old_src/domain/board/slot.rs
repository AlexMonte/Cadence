use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SlotCoord {
    pub col: i32,
    pub row: i32,
}

impl SlotCoord {
    pub const fn new(col: i32, row: i32) -> Self {
        Self { col, row }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct BoardExtent {
    pub cols: u32,
    pub rows: u32,
}

impl BoardExtent {
    pub const fn new(cols: u32, rows: u32) -> Self {
        Self { cols, rows }
    }

    pub fn contains(self, slot: SlotCoord) -> bool {
        slot.col >= 0
            && slot.row >= 0
            && (slot.col as u32) < self.cols
            && (slot.row as u32) < self.rows
    }
}
