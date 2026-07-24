use musaic::application::command::{EditorCommand, execute_command};
use musaic::application::editor::PortSlotState;
use musaic::application::editor::{EditorAttention, SelectionState, connection_endpoint_view};
use musaic::application::pipeline::runtime::TimelineProvenanceStore;
use musaic::domain::board::BoardSlot;
use musaic::domain::document::{
    ContainerKind, DocumentQueries, MusaicDocument, PlacementAddress, TileSpawnKind,
};
use tessera::bevy::TesseraBoard;
use tessera::prelude::SpatialSide;

#[test]
fn port_cycle_persists_in_document_store() {
    let mut document = MusaicDocument::new_empty();
    let root = document.root_surface;
    let tile = document
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
    document.sync_tile_store_from_graph();

    let mut board = TesseraBoard::new();
    let mut attention = EditorAttention::new(root);
    let mut selection = SelectionState::default();
    let provenance = TimelineProvenanceStore::default();

    for (expected, _) in [
        (PortSlotState::Input, "none->input"),
        (PortSlotState::Output, "input->output"),
        (PortSlotState::None, "output->none"),
    ] {
        let result = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::BindOutputSide {
                node: tile.clone(),
                side: SpatialSide::East,
            },
        )
        .expect("bind should run");
        assert!(result.is_accepted(), "bind should accept");

        let queries = DocumentQueries::new(&document);
        let view = connection_endpoint_view(&queries, &tile, Some(BoardSlot::new(0, 0)), None)
            .expect("endpoint view");
        assert_eq!(view.east, expected, "cycle step");
    }

    assert_eq!(
        document.port_endpoints.side_state(&tile, SpatialSide::East),
        PortSlotState::None
    );
}
