use tessera::prelude::{AuthoredTesseraProgram, NodeId, SpatialSide};
use thiserror::Error;

use super::{PortSlotState, default_connection_kind, neighbor_at_side};

/// A connection that root-board geometry permits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoredEdge {
    /// Spatial side on `from` that reaches `to`.
    pub side: SpatialSide,
    /// Bindable port state to apply to both ends of the edge.
    pub kind: PortSlotState,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConnectionPolicyError {
    #[error("a tile cannot connect to itself")]
    SameNode,
    #[error("both tiles must have root-board placements")]
    MissingPlacement,
    #[error("tiles must be edge-adjacent")]
    NotAdjacent,
}

/// Authorizes a root-board connection from footprint-aware placement geometry.
pub fn authorize_connection(
    program: &AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
) -> Result<AuthoredEdge, ConnectionPolicyError> {
    if from == to {
        return Err(ConnectionPolicyError::SameNode);
    }

    let from_placement = program
        .root_surface
        .placements
        .get(from)
        .ok_or(ConnectionPolicyError::MissingPlacement)?;
    let to_placement = program
        .root_surface
        .placements
        .get(to)
        .ok_or(ConnectionPolicyError::MissingPlacement)?;

    let side = [
        SpatialSide::North,
        SpatialSide::South,
        SpatialSide::West,
        SpatialSide::East,
    ]
    .into_iter()
    .find(|side| neighbor_at_side(program, from, *side).as_ref() == Some(to))
    .ok_or(ConnectionPolicyError::NotAdjacent)?;

    Ok(AuthoredEdge {
        side,
        kind: default_connection_kind(from_placement.slot.x, to_placement.slot.x),
    })
}

#[cfg(test)]
mod tests {
    use tessera::prelude::{Board, NodeId, SequenceStack, TileFootprint};

    use super::*;

    fn program_with_output_at(x: i32, y: i32) -> AuthoredTesseraProgram {
        let mut board = Board::new();
        board
            .at(0, 0)
            .named("container")
            .footprint(TileFootprint::new(2, 2))
            .sequence(SequenceStack::new().build())
            .unwrap();
        board.at(x, y).named("output").output().unwrap();
        board.finish()
    }

    #[test]
    fn authorizes_a_2x2_root_edge_neighbor() {
        let edge = authorize_connection(
            &program_with_output_at(2, 0),
            &NodeId("container".into()),
            &NodeId("output".into()),
        )
        .unwrap();

        assert_eq!(edge.side, SpatialSide::East);
        assert_eq!(edge.kind, PortSlotState::Output);
    }

    #[test]
    fn rejects_distant_root_tiles() {
        let error = authorize_connection(
            &program_with_output_at(3, 0),
            &NodeId("container".into()),
            &NodeId("output".into()),
        )
        .unwrap_err();

        assert_eq!(error, ConnectionPolicyError::NotAdjacent);
    }

    #[test]
    fn rejects_a_connection_to_self() {
        let error = authorize_connection(
            &program_with_output_at(2, 0),
            &NodeId("container".into()),
            &NodeId("container".into()),
        )
        .unwrap_err();

        assert_eq!(error, ConnectionPolicyError::SameNode);
    }
}
