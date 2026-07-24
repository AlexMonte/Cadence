//! Render-neutral board projection types (document + attention + selection → scene).

use tessera::prelude::NodeId;

use crate::domain::board::{BoardSlot, BoardSurfaceId, SurfaceLayoutKind};
use crate::domain::document::{PlacementAddress, StackIndex};

use super::board_scene::{BoardSceneAtomCompound, BoardSceneConnection, BoardSceneTile};

/// Singular render focus derived from canonical [`EditorAttention`](crate::application::editor::EditorAttention).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderBoardFocus {
    BoardSlot(BoardSlot),
    StackInsert(StackIndex),
    Tile {
        node: NodeId,
        address: PlacementAddress,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleBoardNode {
    pub node: NodeId,
    pub address: PlacementAddress,
    pub tessera_footprint: Option<tessera::prelude::TileFootprint>,
    pub kind: VisibleNodeKind,
    pub selected: bool,
    pub focused: bool,
    pub icon: Option<crate::adapter::tile_icons::TileIconId>,
    pub surface_content: TileSurfaceContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileSurfaceContent {
    Empty,
    Scalar { display: String },
    Compound { display: String },
    Transform { label: String, aux: Option<String> },
}

impl TileSurfaceContent {
    pub fn display(&self) -> Option<String> {
        match self {
            TileSurfaceContent::Empty => None,
            TileSurfaceContent::Scalar { display } | TileSurfaceContent::Compound { display } => {
                Some(display.clone())
            }
            TileSurfaceContent::Transform { label, aux } => Some(match aux {
                Some(aux) => format!("{label}\n{aux}"),
                None => label.clone(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibleNodeKind {
    Tile,
    Atom,
    Container,
    Output,
    TrickInstance,
}

impl From<VisibleNodeKind> for crate::domain::document::RootBoardTileKind {
    fn from(kind: VisibleNodeKind) -> Self {
        match kind {
            VisibleNodeKind::Container => Self::Container,
            VisibleNodeKind::Output => Self::Output,
            VisibleNodeKind::Tile | VisibleNodeKind::Atom | VisibleNodeKind::TrickInstance => {
                Self::Other
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleAtomCompound {
    pub slot: BoardSlot,
    pub compound: crate::application::editor::AtomCompoundView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleBoardConnection {
    pub from: NodeId,
    pub to: NodeId,
    pub from_slot: BoardSlot,
    pub to_slot: BoardSlot,
    pub kind: crate::domain::document::PortSlotState,
}

/// Renderer-neutral projection of the authored board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardScene {
    pub surface: BoardSurfaceId,
    pub layout: SurfaceLayoutKind,
    pub tiles: Vec<BoardSceneTile>,
    pub compounds: Vec<BoardSceneAtomCompound>,
    pub stack_inserts: Vec<StackIndex>,
    pub stack_locked_slots: Vec<StackIndex>,
    pub connections: Vec<BoardSceneConnection>,
    pub focus: Option<RenderBoardFocus>,
}
