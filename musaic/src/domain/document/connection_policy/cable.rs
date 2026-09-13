//! Deliberate cables reuse the same endpoint typing, occupancy and cycle checks.
use super::*;

pub fn authorize_manual_connection(
    program: &AuthoredTesseraProgram,
    from: &NodeId,
    to: &NodeId,
) -> Result<AuthoredEdge, ConnectionPolicyError> {
    match authorize_connection(program, from, to) {
        Err(ConnectionPolicyError::NotAdjacent) => {}
        Ok(mut edge) => {
            edge.explicit = true;
            return Ok(edge);
        }
        Err(error) => return Err(error),
    }
    let source = &program.root_surface.placements[from];
    let target = &program.root_surface.placements[to];
    // Twice the centers: integer comparison avoids overflow at negative/extreme coordinates.
    let dx = 2 * (i64::from(target.slot.x) - i64::from(source.slot.x))
        + i64::from(target.footprint.width)
        - i64::from(source.footprint.width);
    let dy = 2 * (i64::from(target.slot.y) - i64::from(source.slot.y))
        + i64::from(target.footprint.height)
        - i64::from(source.footprint.height);
    let side = if dx.abs() >= dy.abs() {
        if dx >= 0 {
            SpatialSide::East
        } else {
            SpatialSide::West
        }
    } else if dy >= 0 {
        SpatialSide::South
    } else {
        SpatialSide::North
    };
    authorize_on_side(program, from, to, side, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(name: &str) -> NodeId {
        NodeId::new(name)
    }
    #[test]
    fn distant_feedback_loop_is_rejected_without_changing_the_route() {
        let mut board = Board::new();
        board
            .at(0, 0)
            .named("a")
            .transform(TransformKind::Gain)
            .unwrap();
        board
            .at(10, 0)
            .named("b")
            .transform(TransformKind::Gain)
            .unwrap();
        let mut program = board.finish();
        let edge = authorize_manual_connection(&program, &id("a"), &id("b")).unwrap();
        apply_edge(&mut program, &id("a"), &id("b"), &edge);
        let before = program.clone();
        assert_eq!(
            authorize_manual_connection(&program, &id("b"), &id("a")),
            Err(ConnectionPolicyError::Cycle)
        );
        assert_eq!(program, before);
    }
    #[test]
    fn cable_does_not_silently_connect_an_unrelated_facing_neighbor() {
        let mut board = Board::new();
        board
            .at(0, 0)
            .named("source")
            .scalar(Rational::one())
            .unwrap();
        board
            .at(1, 0)
            .named("neighbor")
            .transform(TransformKind::Gain)
            .unwrap();
        board
            .at(9, 2)
            .named("target")
            .transform(TransformKind::Gain)
            .unwrap();
        let mut program = board.finish();
        let mut source = effective_bindings(&program, &id("source"));
        for side in source.outputs.values_mut() {
            *side = SpatialSide::North;
        }
        program.root_surface.bindings.insert(id("source"), source);
        let mut neighbor = effective_bindings(&program, &id("neighbor"));
        neighbor.inputs.insert(
            InputEndpoint::Socket(InputPort::new("amount")),
            SpatialSide::West,
        );
        program
            .root_surface
            .bindings
            .insert(id("neighbor"), neighbor);
        let before = program.clone();
        assert_eq!(
            authorize_manual_connection(&program, &id("source"), &id("target")),
            Err(ConnectionPolicyError::Ambiguous)
        );
        assert_eq!(program, before);
    }
}
