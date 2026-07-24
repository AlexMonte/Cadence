use tessera::prelude::TileFootprint;

use super::graph::DocumentNodeKind;
use super::TileSpawnKind;

/// Root-board tile categories whose authored Tessera placement spans 2×2 slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootBoardTileKind {
    Container,
    Output,
    Other,
}

impl From<&TileSpawnKind> for RootBoardTileKind {
    fn from(kind: &TileSpawnKind) -> Self {
        match kind {
            TileSpawnKind::Container { .. } => Self::Container,
            TileSpawnKind::Output { .. } => Self::Output,
            _ => Self::Other,
        }
    }
}

impl From<&DocumentNodeKind> for RootBoardTileKind {
    fn from(kind: &DocumentNodeKind) -> Self {
        match kind {
            DocumentNodeKind::Container(_) => Self::Container,
            DocumentNodeKind::Output(_) => Self::Output,
            _ => Self::Other,
        }
    }
}

/// The authored Tessera occupancy for a tile placed on the root board.
pub fn root_board_tile_footprint(kind: impl Into<RootBoardTileKind>) -> TileFootprint {
    match kind.into() {
        RootBoardTileKind::Container | RootBoardTileKind::Output => TileFootprint::new(2, 2),
        RootBoardTileKind::Other => TileFootprint::unit(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::board::BoardSlot;
    use crate::domain::document::{
        ContainerKind, DocumentGraphError, MusaicDocument, PlacementAddress,
    };

    #[test]
    fn containers_and_outputs_occupy_two_by_two_root_board_slots() {
        assert_eq!(
            root_board_tile_footprint(&TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            }),
            TileFootprint::new(2, 2)
        );
        assert_eq!(
            root_board_tile_footprint(&TileSpawnKind::Output {
                name: "main".into(),
            }),
            TileFootprint::new(2, 2)
        );
        assert_eq!(
            root_board_tile_footprint(RootBoardTileKind::Other),
            TileFootprint::unit()
        );
    }

    #[test]
    fn insert_rejects_output_anchored_inside_container_footprint() {
        let mut document = MusaicDocument::new_empty();
        let root = document.root_surface;
        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            )
            .unwrap();

        let err = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(1, 0)),
                TileSpawnKind::Output {
                    name: "main".into(),
                },
            )
            .unwrap_err();

        assert!(matches!(err, DocumentGraphError::OccupiedAddress { .. }));
    }
}
