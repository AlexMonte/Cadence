use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::board::{SlotCoord, TileInstanceId};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlotOccupancy(pub BTreeMap<SlotCoord, TileInstanceId>);

impl SlotOccupancy {
    pub fn get(&self, slot: SlotCoord) -> Option<&TileInstanceId> {
        self.0.get(&slot)
    }

    pub fn insert(&mut self, slot: SlotCoord, tile_id: TileInstanceId) -> Option<TileInstanceId> {
        self.0.insert(slot, tile_id)
    }

    pub fn remove(&mut self, slot: SlotCoord) -> Option<TileInstanceId> {
        self.0.remove(&slot)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&SlotCoord, &TileInstanceId)> {
        self.0.iter()
    }
}
