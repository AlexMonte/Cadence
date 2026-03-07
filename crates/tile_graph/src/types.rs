use std::cmp::Ordering;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Determinism contract: GridPos ordering is always col-first, then row.
// Keep Ord impl explicit so future field edits do not silently change topo tie-breaking.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GridPos {
    pub col: i32,
    pub row: i32,
}

impl GridPos {
    pub fn adjacent_in_direction(&self, side: TileSide) -> Self {
        match side {
            TileSide::North => Self {
                col: self.col,
                row: self.row - 1,
            },
            TileSide::South => Self {
                col: self.col,
                row: self.row + 1,
            },
            TileSide::East => Self {
                col: self.col + 1,
                row: self.row,
            },
            TileSide::West => Self {
                col: self.col - 1,
                row: self.row,
            },
        }
    }
}

pub fn adjacent_in_direction(pos: &GridPos, side: &TileSide) -> GridPos {
    pos.adjacent_in_direction(*side)
}

impl PartialOrd for GridPos {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GridPos {
    fn cmp(&self, other: &Self) -> Ordering {
        self.col.cmp(&other.col).then(self.row.cmp(&other.row))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EdgeId(pub Uuid);

impl EdgeId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PortType(String);

impl PortType {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn number() -> Self {
        Self::new("number")
    }

    pub fn text() -> Self {
        Self::new("text")
    }

    pub fn bool() -> Self {
        Self::new("bool")
    }

    pub fn any() -> Self {
        Self::new("any")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_any(&self) -> bool {
        self.as_str() == "any"
    }

    pub fn accepts(&self, other: &PortType) -> bool {
        self.is_any() || other.is_any() || self == other
    }
}

impl From<&str> for PortType {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PortType {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileSide {
    North,
    South,
    East,
    West,
}

impl TileSide {
    pub fn faces(self, other: TileSide) -> bool {
        matches!(
            (self, other),
            (TileSide::East, TileSide::West)
                | (TileSide::West, TileSide::East)
                | (TileSide::North, TileSide::South)
                | (TileSide::South, TileSide::North)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PieceCategory {
    Generator,
    Transform,
    Trick,
    Constant,
    Output,
    Control,
}
