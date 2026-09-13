//! Render-neutral board projection types (document + attention + selection → scene).

use tessera::prelude::NodeId;

use crate::application::editor::ConnectionEndpointView;
use crate::domain::board::BoardSlot;
use crate::domain::document::{AtomValue, PlacementAddress, StackIndex};

/// Presentation layout for owned expressions and wide containers. Canonical
/// musical addresses and authored rests never change when a face grows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct StackDisplayMap {
    hidden: std::collections::BTreeMap<StackIndex, StackIndex>,
    spans: Vec<(StackIndex, StackIndex, usize, usize)>,
}

impl StackDisplayMap {
    pub(super) fn new(
        hidden: std::collections::BTreeMap<StackIndex, StackIndex>,
        widths: std::collections::BTreeMap<StackIndex, usize>,
    ) -> Self {
        let mut result = Self {
            hidden,
            spans: Vec::new(),
        };
        let mut extra = 0;
        for (authored, width) in widths {
            let compact = result.compact_index(authored);
            let start = compact + extra;
            let columns = crate::domain::board::geometry::STACK_COLUMNS;
            let padding = if start % columns + width > columns {
                columns - start % columns
            } else {
                0
            };
            result
                .spans
                .push((authored, StackIndex(start + padding), width, padding));
            extra += width - 1 + padding;
        }
        result
    }

    fn compact_index(&self, authored: StackIndex) -> usize {
        authored
            .0
            .saturating_sub(self.hidden.range(..authored).count())
    }

    pub fn display_index(&self, authored: StackIndex) -> StackIndex {
        let anchor = self.hidden.get(&authored).copied().unwrap_or(authored);
        let mut extra = 0;
        for &(owner, display, width, _) in &self.spans {
            if owner > anchor {
                break;
            }
            if owner == anchor {
                return display;
            }
            extra = display.0 + width - self.compact_index(owner) - 1;
        }
        StackIndex(self.compact_index(anchor) + extra)
    }

    /// Every cell of a wide face resolves to its owner; row-end padding inserts
    /// before the next face instead of creating a hidden musical rest.
    pub fn authored_index(&self, display: StackIndex) -> StackIndex {
        let mut extra = 0;
        for &(owner, start, width, padding) in &self.spans {
            if display.0 < start.0.saturating_sub(padding) {
                break;
            }
            if display.0 < start.0 + width {
                return owner;
            }
            extra = start.0 + width - self.compact_index(owner) - 1;
        }
        let mut authored = display.0.saturating_sub(extra);
        for hidden in self.hidden.keys() {
            if hidden.0 > authored {
                break;
            }
            authored = authored.saturating_add(1);
        }
        StackIndex(authored)
    }

    pub fn display_address(&self, authored: PlacementAddress) -> PlacementAddress {
        match authored {
            PlacementAddress::StackIndex(index) => {
                PlacementAddress::StackIndex(self.display_index(index))
            }
            address => address,
        }
    }
}

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
    /// Trick / catalog icon identity (board chrome; never re-queried in Render).
    pub icon: Option<crate::adapter::tile_icons::TileIconId>,
    /// Atom value for texture / ortho sheet / minimap color (board chrome).
    pub atom: Option<AtomValue>,
    /// Port compass view for root-board tiles (board + inspector chrome).
    pub ports: Option<ConnectionEndpointView>,
    pub surface_content: TileSurfaceContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TileSurfaceContent {
    Wire {
        ports: crate::domain::document::PortEndpointConfig,
    },
    Empty,
    Scalar {
        display: String,
    },
    Compound {
        display: String,
        layers: usize,
        /// Actual authored constituents, in stack order, for the face preview.
        parts: Vec<String>,
    },
    Transform {
        label: String,
        aux: Option<String>,
    },
    Container {
        kind: crate::domain::document::ContainerKind,
        children: Vec<TilePreviewCell>,
        /// All authored child tiles, including layers summarized by a compound.
        child_count: usize,
    },
}

/// A bounded thumbnail of authored content, using the same compound projection
/// as the opened board. Positions retain the authored row and column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TilePreviewCell {
    pub slot: BoardSlot,
    pub kind: VisibleNodeKind,
    pub content: TileSurfaceContent,
}

impl TileSurfaceContent {
    pub fn display(&self) -> Option<String> {
        match self {
            TileSurfaceContent::Empty => None,
            TileSurfaceContent::Wire { .. } => Some("Wire".into()),
            TileSurfaceContent::Scalar { display }
            | TileSurfaceContent::Compound { display, .. } => Some(display.clone()),
            TileSurfaceContent::Transform { label, aux } => Some(match aux {
                Some(aux) => format!("{label}\n{aux}"),
                None => label.clone(),
            }),
            TileSurfaceContent::Container {
                kind, child_count, ..
            } => Some(format!("{kind:?} · {child_count} tiles")),
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
    /// First member's atom for minimap fill without reopening the document.
    pub primary_atom: Option<AtomValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleBoardConnection {
    pub side: tessera::prelude::SpatialSide,
    pub from: NodeId,
    pub to: NodeId,
    pub from_slot: BoardSlot,
    pub to_slot: BoardSlot,
    pub kind: crate::domain::document::PortSlotState,
}
