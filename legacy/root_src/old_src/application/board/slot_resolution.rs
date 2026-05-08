use crate::domain::board::SlotCoord;

use super::{BoardSpaceVec2, SlotLayout};

pub fn resolve_slot(layout: SlotLayout, position: BoardSpaceVec2) -> SlotCoord {
    let col = ((position.x - layout.origin.x) / layout.slot_stride.x).round() as i32;
    let row = ((layout.origin.y - position.y) / layout.slot_stride.y).round() as i32;
    SlotCoord::new(col, row)
}
