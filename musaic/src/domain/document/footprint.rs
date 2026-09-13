use tessera::prelude::TileFootprint;

use super::TileSpawnKind;
use super::graph::DocumentNodeKind;

/// Root-board tile categories with non-unit authored occupancy.
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
        RootBoardTileKind::Container => TileFootprint::new(5, 1),
        RootBoardTileKind::Output => TileFootprint::unit(),
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
    use crate::domain::instrument::{InstrumentDefinition, InstrumentSource, Waveform};

    #[test]
    fn containers_occupy_five_cells_and_outputs_one() {
        assert_eq!(
            root_board_tile_footprint(&TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            }),
            TileFootprint::new(5, 1)
        );
        assert_eq!(
            root_board_tile_footprint(&TileSpawnKind::Output {
                name: "main".into(),
            }),
            TileFootprint::unit()
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
                PlacementAddress::BoardSlot(BoardSlot::new(4, 0)),
                TileSpawnKind::Output {
                    name: "main".into(),
                },
            )
            .unwrap_err();

        assert!(matches!(err, DocumentGraphError::OccupiedAddress { .. }));
    }

    #[test]
    fn invalid_sound_is_rejected_before_document_identity_is_allocated() {
        let mut document = MusaicDocument::new_empty();
        let root = document.root_surface;
        let mut invalid = InstrumentDefinition::new(InstrumentSource::Synth(Waveform::Sine));
        invalid.rate = tessera::prelude::Rational::zero();

        let err = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::sound(invalid),
            )
            .unwrap_err();
        assert!(matches!(err, DocumentGraphError::InvalidNode(_)));
        assert!(document.graph.nodes().next().is_none());

        let sound = document
            .graph
            .insert_tile(
                &mut document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::sound(InstrumentDefinition::default()),
            )
            .unwrap();
        assert_eq!(sound.0, "doc_1");
    }
}
