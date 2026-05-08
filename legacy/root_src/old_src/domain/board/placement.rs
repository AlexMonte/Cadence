use serde::{Deserialize, Serialize};

use crate::domain::board::{BoardSurfaceKind, TileClass};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacementRule;

impl PlacementRule {
    pub fn accepts(surface: BoardSurfaceKind, class: TileClass) -> bool {
        match surface {
            BoardSurfaceKind::Flow => {
                matches!(
                    class,
                    TileClass::Container | TileClass::Transform | TileClass::Terminal
                )
            }
            BoardSurfaceKind::ContainerLocal => {
                matches!(class, TileClass::Atom | TileClass::Container)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlacementVerdict {
    Valid,
    Invalid { reason: String },
}

impl PlacementVerdict {
    pub fn valid(self) -> bool {
        matches!(self, Self::Valid)
    }
}
