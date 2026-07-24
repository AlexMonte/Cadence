use tessera::prelude::*;

fn place<T>(result: Result<T, BoardError>) -> T {
    result.expect("placement should succeed")
}

#[test]
fn board_adjacent_container_to_output_resolves_flow() {
    let program = {
        let mut board = Board::new();
        place(board.at(0, 0).sequence(notes(["a", "b", "c"])));
        place(board.at(1, 0).output());
        board.finish()
    };
    let resolved = TesseraCompiler::new()
        .resolve(&program)
        .expect("spatial program should resolve");
    assert_eq!(resolved.relations.len(), 1);
}

#[test]
fn board_adjacent_transform_chain_resolves() {
    let program = {
        let mut board = Board::new();
        place(board.at(0, 0).sequence(notes(["a", "b", "c"])));
        place(board.at(1, 0).slow());
        place(board.at(2, 0).output());
        board.finish()
    };
    let resolved = TesseraCompiler::new()
        .resolve(&program)
        .expect("spatial program should resolve");
    assert_eq!(resolved.relations.len(), 2);
}

#[test]
fn board_factor_above_slow_resolves_factor_input() {
    let mut board = Board::new();
    place(board.at(0, 1).sequence(notes(["a", "b", "c"])));
    let factor = place(board.at(1, 0).sequence(vec![scalar(2)]));
    place(board.at(1, 1).slow());
    place(board.at(2, 1).output());
    board
        .bind_output_side(
            &factor,
            OutputEndpoint::Socket(OutputPort::new("out")),
            SpatialSide::South,
        )
        .expect("bind output");
    let program = board.finish();
    let resolved = TesseraCompiler::new()
        .resolve(&program)
        .expect("spatial program should resolve");
    assert_eq!(resolved.relations.len(), 3);
}

#[test]
fn board_off_input_does_not_resolve() {
    let mut board = Board::new();
    place(board.at(0, 0).sequence(notes(["a"])));
    place(board.at(1, 0).slow());
    board
        .bind_input_side(
            &TileRef {
                id: NodeId::new("n_1_0"),
                slot: slot(1, 0),
            },
            InputEndpoint::Socket(InputPort::new("main")),
            SpatialSide::Off,
        )
        .expect("bind input");
    let program = board.finish();
    let resolved = TesseraCompiler::new()
        .resolve(&program)
        .expect("off binding should not fail resolution");
    assert!(resolved.relations.is_empty());
}

#[test]
fn board_compiles_to_ir() {
    let program = {
        let mut board = Board::new();
        place(board.at(0, 0).sequence(notes(["a", "b", "c"])));
        place(board.at(1, 0).output());
        board.finish()
    };
    let ir = TesseraCompiler::new()
        .compile_authored_ir(&program)
        .expect("authored spatial program should compile");
    assert_eq!(ir.outputs.len(), 1);
}

#[test]
fn flow_builder_lays_out_left_to_right() {
    let program = Flow::new()
        .source(Sequence::new().notes(["a", "b", "c"]).build())
        .expect("source")
        .then(TransformKind::Slow)
        .expect("slow")
        .output()
        .expect("output");
    let resolved = TesseraCompiler::new()
        .resolve(&program)
        .expect("flow program should resolve");
    assert_eq!(resolved.relations.len(), 2);
}

#[test]
fn flow_cursor_horizontal_chain() {
    let program = {
        let mut board = Board::new();
        board
            .start(0, 0)
            .sequence(notes(["a", "b", "c"]))
            .expect("sequence")
            .slow()
            .expect("slow")
            .output()
            .expect("output");
        board.finish()
    };
    let ir = TesseraCompiler::new()
        .compile_authored_ir(&program)
        .expect("cursor chain should compile");
    assert_eq!(ir.outputs.len(), 1);
}

#[test]
fn sequence_stack_fluent_atoms() {
    let stack = SequenceStack::new().note("a").elongate(3).note("b").build();
    let program = {
        let mut board = Board::new();
        place(board.at(0, 0).sequence(stack));
        place(board.at(1, 0).output());
        board.finish()
    };
    let ir = TesseraCompiler::new()
        .compile_authored_ir(&program)
        .expect("sequence stack program should compile");
    assert_eq!(ir.outputs.len(), 1);
}

#[test]
fn board_replace_at_removes_previous_occupant() {
    let mut board = Board::new();
    place(board.at(0, 0).sequence(notes(["a"])));
    place(board.replace_at(0, 0).slow());
    let program = board.finish();
    assert_eq!(program.root_surface.nodes.len(), 1);
    assert!(matches!(
        program.root_surface.nodes.values().next(),
        Some(RootSurfaceNodeKind::Transform(_))
    ));
    TesseraCompiler::new()
        .resolve(&program)
        .expect("replaced board should validate");
}

#[test]
fn board_from_program_round_trip() {
    let original = {
        let mut board = Board::new();
        place(board.at(0, 0).sequence(notes(["a", "b"])));
        place(board.at(1, 0).output());
        board.finish()
    };
    let board = Board::from_program(original.clone());
    assert_eq!(
        board.tile_at(slot(0, 0)).map(|t| t.id),
        board.tile_at(slot(0, 0)).map(|t| t.id)
    );
    assert!(board.tile_at(slot(0, 0)).is_some());
    assert!(board.tile_at(slot(1, 0)).is_some());
    let round_trip = board.finish();
    assert_eq!(
        round_trip.root_surface.nodes.len(),
        original.root_surface.nodes.len()
    );
}

#[test]
fn board_move_tile_updates_slot_index() {
    let mut board = Board::new();
    let tile = place(board.at(0, 0).sequence(notes(["a"])));
    let moved = board
        .move_tile(&tile.id, slot(2, 1))
        .expect("move should succeed");
    assert_eq!(moved.slot, slot(2, 1));
    assert!(board.tile_at(slot(0, 0)).is_none());
    assert_eq!(board.tile_at(slot(2, 1)).unwrap().id, tile.id);
}

#[test]
fn tile_handle_bind_and_set_sequence() {
    let mut board = Board::new();
    let tile = place(board.at(0, 0).sequence(notes(["a"])));
    board
        .handle(&tile.id)
        .expect("tile handle")
        .set_sequence(notes(["b", "c"]))
        .expect("set sequence");
    place(board.at(1, 0).output());
    let program = board.finish();
    let ir = TesseraCompiler::new()
        .compile_authored_ir(&program)
        .expect("edited sequence should compile");
    assert_eq!(ir.outputs.len(), 1);
}

#[test]
fn board_rejects_duplicate_named_id_at_different_slot() {
    let mut board = Board::new();
    place(board.at(0, 0).named("shared").sequence(notes(["a"])));
    let err = board
        .at(1, 0)
        .named("shared")
        .sequence(notes(["b"]))
        .expect_err("duplicate id should fail");
    assert!(matches!(err, BoardError::DuplicateId { .. }));
}

#[test]
fn board_stale_tile_ref_rejected_after_replace() {
    let mut board = Board::new();
    let old = place(board.at(0, 0).sequence(notes(["a"])));
    place(board.replace_at(0, 0).slow());
    let err = board
        .bind_input_side(
            &old,
            InputEndpoint::Socket(InputPort::new("main")),
            SpatialSide::West,
        )
        .expect_err("stale tile ref should fail");
    assert_eq!(err, BoardError::UnknownTile);
}

#[test]
fn board_attach_neighbor_places_and_binds_domino() {
    let mut board = Board::new();
    let source = place(board.at(0, 0).sequence(notes(["a", "b"])));
    let next = board
        .attach_neighbor(&source, SpatialSide::East, |slot| slot.slow())
        .expect("attach neighbor");
    assert_eq!(next.slot, slot(1, 0));
    let program = board.finish();
    let resolved = TesseraCompiler::new()
        .resolve(&program)
        .expect("attached domino should resolve");
    assert_eq!(resolved.relations.len(), 1);
}

#[test]
fn board_rejects_overlapping_footprints_via_validation() {
    use tessera::prelude::{RootPlacement, TileFootprint};
    let mut program = AuthoredTesseraProgram::empty();
    program
        .place_sequence("left", slot(0, 0), notes(["a"]))
        .place_sequence("right", slot(2, 0), notes(["b"]));
    program.root_surface.placements.insert(
        NodeId::new("left"),
        RootPlacement::new(slot(0, 0), TileFootprint::new(2, 1)),
    );
    program.root_surface.placements.insert(
        NodeId::new("right"),
        RootPlacement::new(slot(1, 0), TileFootprint::unit()),
    );
    let err = TesseraCompiler::new()
        .resolve(&program)
        .expect_err("overlapping placements should fail validation");
    assert!(!err.is_empty());
}

#[test]
fn board_remove_cleans_nested_containers() {
    let mut program = AuthoredTesseraProgram::default();
    program.containers.insert(
        ContainerId::new("child"),
        Container {
            kind: ContainerKind::Sequence,
            axis: ContainerAxis::Time,
            stack: Vec::new(),
        },
    );
    let mut board = Board::from_program(program);
    let tile = place(board.at(0, 0).sequence(vec![nested("child")]));
    place(board.at(1, 0).output());
    board.remove_at(tile.slot);
    let program = board.finish();
    assert!(!program.containers.contains_key(&ContainerId::new("child")));
    assert!(!program.containers.contains_key(&ContainerId::new("n_0_0")));
}
