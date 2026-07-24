use musaic::domain::board::BoardSlot;
use musaic::domain::document::{AtomValue, ContainerKind, NoteName};
use musaic::domain::document::{
    MusaicDocument, PlacementAddress, StackIndex, TileSpawnKind, bind_tiles_on_board,
    hydrate_board_from_document,
};
use tessera::bevy::TesseraBoard;
use tessera::prelude::SpatialSide;

#[test]
fn connection_round_trip_preserves_relation_count() {
    let mut document = MusaicDocument::new_empty();
    let root = document.root_surface;

    let sequence = document
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
    let surface = document.graph.container_surface(&sequence).unwrap();
    document
        .graph
        .insert_tile(
            &mut document.surfaces,
            surface,
            PlacementAddress::StackIndex(StackIndex(0)),
            TileSpawnKind::Atom {
                atom: AtomValue::NoteName(NoteName::C),
            },
        )
        .unwrap();
    let output = document
        .graph
        .insert_tile(
            &mut document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(1, 0)),
            TileSpawnKind::Output {
                name: "main".into(),
            },
        )
        .unwrap();

    let mut board = TesseraBoard::new();
    hydrate_board_from_document(&mut board, &document).expect("export");
    bind_tiles_on_board(&mut board, &sequence, &output, SpatialSide::East).expect("bind");
    document.tessera.authored_program = board.authored_program();

    let relations = board
        .authored_program()
        .root_surface
        .explicit_relations
        .len()
        + board.authored_program().root_surface.bindings.len();
    assert!(relations >= 1, "expected authored relations on board");

    hydrate_board_from_document(&mut board, &document).expect("re-export");
    let relations_after = board
        .authored_program()
        .root_surface
        .explicit_relations
        .len()
        + board.authored_program().root_surface.bindings.len();
    assert_eq!(relations_after, relations);
}
