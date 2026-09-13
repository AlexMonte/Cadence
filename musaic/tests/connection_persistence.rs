use musaic::domain::board::BoardSlot;
use musaic::domain::document::{AtomValue, ContainerKind, NoteName};
use musaic::domain::document::{
    MusaicDocument, PlacementAddress, StackIndex, TileSpawnKind, bind_tiles,
    export_document_program,
};
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
            // The 5×1 sequence fills columns 0–4; its east neighbor begins at column 5.
            PlacementAddress::BoardSlot(BoardSlot::new(5, 0)),
            TileSpawnKind::Output {
                name: "main".into(),
            },
        )
        .unwrap();

    let mut program = export_document_program(&document).expect("export");
    bind_tiles(&mut program, &sequence, &output, SpatialSide::East).expect("bind");
    document.replace_connections_from(&program);

    let relations =
        program.root_surface.explicit_relations.len() + program.root_surface.bindings.len();
    assert!(relations >= 1, "expected authored relations on board");

    let reexported = export_document_program(&document).expect("re-export");
    let relations_after =
        reexported.root_surface.explicit_relations.len() + reexported.root_surface.bindings.len();
    assert_eq!(relations_after, relations);
}
