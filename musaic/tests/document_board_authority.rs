use musaic::domain::board::BoardSlot;
use musaic::domain::document::{AtomValue, ContainerKind, NoteName};
use musaic::domain::document::{
    MusaicDocument, PlacementAddress, StackIndex, TileSpawnKind, export_document_to_board,
    finish_board_export, hydrate_board_from_document,
};
use tessera::bevy::TesseraBoard;
use tessera::prelude::TesseraCompiler;

#[test]
fn bpm_defaults_to_120() {
    let document = MusaicDocument::new_empty();
    assert!((document.playback.bpm - 120.0).abs() < f64::EPSILON);
}

#[test]
fn export_is_idempotent_for_sequence_and_output() {
    let mut document = build_sequence_output_document();
    let first = compile_exported_ir(&document);
    let second = compile_exported_ir(&document);
    assert_eq!(first.outputs.len(), second.outputs.len());
}

#[test]
fn connections_survive_board_reexport() {
    let mut document = build_sequence_output_document();
    let root = document.root_surface;
    let nodes: Vec<_> = document
        .graph
        .nodes_on_surface(root)
        .into_iter()
        .map(|(_, node)| node.id.clone())
        .collect();
    assert_eq!(nodes.len(), 2);

    let mut board = TesseraBoard::new();
    hydrate_board_from_document(&mut board, &document).expect("export");
    musaic::domain::document::bind_tiles_on_board(
        &mut board,
        &nodes[0],
        &nodes[1],
        tessera::prelude::SpatialSide::East,
    )
    .expect("bind");
    document.tessera.authored_program = board.authored_program();

    hydrate_board_from_document(&mut board, &document).expect("re-export");
    let binding_count = board.authored_program().root_surface.bindings.len();
    assert!(binding_count >= 1, "expected output bindings on board");
}

fn build_sequence_output_document() -> MusaicDocument {
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
    let container_surface = document.graph.container_surface(&sequence).unwrap();
    document
        .graph
        .insert_tile(
            &mut document.surfaces,
            container_surface,
            PlacementAddress::StackIndex(StackIndex(0)),
            TileSpawnKind::Atom {
                atom: AtomValue::NoteName(NoteName::A),
            },
        )
        .unwrap();
    document
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
    document
}

fn compile_exported_ir(document: &MusaicDocument) -> tessera::prelude::PatternIr {
    let export = export_document_to_board(document).expect("export");
    let program = finish_board_export(export);
    let (_, _, report) = TesseraCompiler::new()
        .compile_authored_pipeline(&program)
        .expect("compile");
    report.ir
}
