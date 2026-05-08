use crate::application::board::{BoardSpaceVec2, SlotLayout};
use crate::domain::board::SlotCoord;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogicalRect {
    pub center: BoardSpaceVec2,
    pub size: BoardSpaceVec2,
}

pub fn slot_rect(layout: SlotLayout, slot: SlotCoord) -> LogicalRect {
    LogicalRect {
        center: BoardSpaceVec2 {
            x: layout.origin.x + slot.col as f32 * layout.slot_stride.x,
            y: layout.origin.y - slot.row as f32 * layout.slot_stride.y,
        },
        size: layout.slot_size,
    }
}
