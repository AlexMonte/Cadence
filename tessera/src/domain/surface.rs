use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::{
    Container, ContainerId, InputEndpoint, NodeId, OutputEndpoint, RootRelation,
    RootSurfaceNodeKind,
};

#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct AuthoredTesseraProgram {
    pub root_surface: RootSurface,
    pub containers: BTreeMap<ContainerId, Container>,
}

impl AuthoredTesseraProgram {
    pub fn empty() -> Self {
        Self {
            root_surface: RootSurface::default(),
            containers: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct RootSurface {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub nodes: BTreeMap<NodeId, RootSurfaceNodeKind>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub placements: BTreeMap<NodeId, RootPlacement>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<NodeId, NodeSpatialBindings>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explicit_relations: Vec<RootRelation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct BoardSlot {
    pub x: i32,
    pub y: i32,
}

impl BoardSlot {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn offset(self, dx: i32, dy: i32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct TileFootprint {
    pub width: u32,
    pub height: u32,
}

impl TileFootprint {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn unit() -> Self {
        Self {
            width: 1,
            height: 1,
        }
    }

    pub fn cell_count(&self) -> u32 {
        self.width.saturating_mul(self.height)
    }

    pub fn occupied_cells(&self, origin: BoardSlot) -> impl Iterator<Item = BoardSlot> + use<'_> {
        (0..self.height)
            .flat_map(move |dy| (0..self.width).map(move |dx| origin.offset(dx as i32, dy as i32)))
    }

    pub fn occupies(&self, origin: BoardSlot, cell: BoardSlot) -> bool {
        let dx = cell.x - origin.x;
        let dy = cell.y - origin.y;
        dx >= 0 && dy >= 0 && (dx as u32) < self.width && (dy as u32) < self.height
    }

    pub fn edge_cells(&self, origin: BoardSlot, side: SpatialSide) -> Vec<BoardSlot> {
        if !side.is_enabled() || self.width == 0 || self.height == 0 {
            return Vec::new();
        }
        match side {
            SpatialSide::Off => Vec::new(),
            SpatialSide::North => (0..self.width)
                .map(|dx| origin.offset(dx as i32, 0))
                .collect(),
            SpatialSide::South => (0..self.width)
                .map(|dx| origin.offset(dx as i32, self.height as i32 - 1))
                .collect(),
            SpatialSide::West => (0..self.height)
                .map(|dy| origin.offset(0, dy as i32))
                .collect(),
            SpatialSide::East => (0..self.height)
                .map(|dy| origin.offset(self.width as i32 - 1, dy as i32))
                .collect(),
        }
    }

    pub fn edge_adjacent(&self, origin: BoardSlot, side: SpatialSide) -> Vec<BoardSlot> {
        let Some((dx, dy)) = side.offset() else {
            return Vec::new();
        };
        self.edge_cells(origin, side)
            .into_iter()
            .map(|cell| cell.offset(dx, dy))
            .collect()
    }

    pub fn anchor_for_neighbor(
        &self,
        origin: BoardSlot,
        side: SpatialSide,
        neighbor: TileFootprint,
    ) -> BoardSlot {
        match side {
            SpatialSide::Off => origin,
            SpatialSide::North => BoardSlot::new(origin.x, origin.y - neighbor.height as i32),
            SpatialSide::South => BoardSlot::new(origin.x, origin.y + self.height as i32),
            SpatialSide::West => BoardSlot::new(origin.x - neighbor.width as i32, origin.y),
            SpatialSide::East => BoardSlot::new(origin.x + self.width as i32, origin.y),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct RootPlacement {
    pub slot: BoardSlot,
    pub footprint: TileFootprint,
}

impl RootPlacement {
    pub fn new(slot: BoardSlot, footprint: TileFootprint) -> Self {
        Self { slot, footprint }
    }

    pub fn unit(x: i32, y: i32) -> Self {
        Self {
            slot: BoardSlot::new(x, y),
            footprint: TileFootprint::unit(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum SpatialSide {
    Off,
    North,
    South,
    West,
    East,
}

impl SpatialSide {
    pub fn is_enabled(self) -> bool {
        !matches!(self, SpatialSide::Off)
    }

    pub fn offset(self) -> Option<(i32, i32)> {
        match self {
            SpatialSide::Off => None,
            SpatialSide::North => Some((0, -1)),
            SpatialSide::South => Some((0, 1)),
            SpatialSide::West => Some((-1, 0)),
            SpatialSide::East => Some((1, 0)),
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            SpatialSide::Off => SpatialSide::Off,
            SpatialSide::North => SpatialSide::South,
            SpatialSide::South => SpatialSide::North,
            SpatialSide::West => SpatialSide::East,
            SpatialSide::East => SpatialSide::West,
        }
    }

    pub fn rotate_cw(self) -> Self {
        match self {
            SpatialSide::Off => SpatialSide::Off,
            SpatialSide::North => SpatialSide::East,
            SpatialSide::East => SpatialSide::South,
            SpatialSide::South => SpatialSide::West,
            SpatialSide::West => SpatialSide::North,
        }
    }

    pub fn rotate_ccw(self) -> Self {
        match self {
            SpatialSide::Off => SpatialSide::Off,
            SpatialSide::North => SpatialSide::West,
            SpatialSide::West => SpatialSide::South,
            SpatialSide::South => SpatialSide::East,
            SpatialSide::East => SpatialSide::North,
        }
    }

    pub fn mirror_vertical(self) -> Self {
        match self {
            SpatialSide::North => SpatialSide::South,
            SpatialSide::South => SpatialSide::North,
            other => other,
        }
    }

    pub fn mirror_horizontal(self) -> Self {
        match self {
            SpatialSide::West => SpatialSide::East,
            SpatialSide::East => SpatialSide::West,
            other => other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct NodeSpatialBindings {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub inputs: BTreeMap<InputEndpoint, SpatialSide>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub outputs: BTreeMap<OutputEndpoint, SpatialSide>,
}

impl NodeSpatialBindings {
    pub fn bind_input(mut self, endpoint: InputEndpoint, side: SpatialSide) -> Self {
        self.inputs.insert(endpoint, side);
        self
    }

    pub fn bind_output(mut self, endpoint: OutputEndpoint, side: SpatialSide) -> Self {
        self.outputs.insert(endpoint, side);
        self
    }

    pub fn rotate_cw(&mut self) {
        for side in self.inputs.values_mut() {
            *side = side.rotate_cw();
        }
        for side in self.outputs.values_mut() {
            *side = side.rotate_cw();
        }
    }

    pub fn rotate_ccw(&mut self) {
        for side in self.inputs.values_mut() {
            *side = side.rotate_ccw();
        }
        for side in self.outputs.values_mut() {
            *side = side.rotate_ccw();
        }
    }

    pub fn mirror_vertical(&mut self) {
        for side in self.inputs.values_mut() {
            *side = side.mirror_vertical();
        }
        for side in self.outputs.values_mut() {
            *side = side.mirror_vertical();
        }
    }

    pub fn mirror_horizontal(&mut self) {
        for side in self.inputs.values_mut() {
            *side = side.mirror_horizontal();
        }
        for side in self.outputs.values_mut() {
            *side = side.mirror_horizontal();
        }
    }
}
