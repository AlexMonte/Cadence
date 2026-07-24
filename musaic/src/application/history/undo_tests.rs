#[cfg(test)]
mod tests {
    use tessera::bevy::TesseraBoard;
    use tessera::prelude::NodeId;

    use crate::application::command::{
        EditorCommand, EditorInverse, execute_command, execute_inverse,
    };
    use crate::application::editor::transaction::PlacementTarget;
    use crate::application::editor::{EditorAttention, FocusTarget, SelectionState};
    use crate::application::history::{CommandHistory, HistoryEntry, HistoryPolicy};
    use crate::application::pipeline::runtime::TimelineProvenanceStore;
    use crate::domain::board::{BoardSlot, BoardSurfaceId};
    use crate::domain::document::{ContainerKind, DocumentQueries, MusaicDocument, TileSpawnKind};

    fn fresh_board() -> TesseraBoard {
        TesseraBoard::new()
    }

    fn place_container(
        document: &mut MusaicDocument,
        board: &mut TesseraBoard,
        attention: &mut EditorAttention,
        selection: &mut SelectionState,
    ) -> NodeId {
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;
        let command = EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root_surface,
                slot: BoardSlot::new(0, 0),
            },
            tile: TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        };
        let result = execute_command(document, board, attention, selection, &provenance, &command)
            .expect("place");
        assert!(result.is_accepted());
        assert_eq!(command.history_policy(), HistoryPolicy::RecordMutation);
        selection.nodes.iter().next().unwrap().clone()
    }

    #[test]
    fn place_undo_restores_document_revision() {
        let mut document = MusaicDocument::new_empty();
        let mut board = fresh_board();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let revision_before = document.revision.0;

        let command = EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: document.root_surface,
                slot: BoardSlot::new(0, 0),
            },
            tile: TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        };
        let placed = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &command,
        )
        .expect("place");
        assert!(placed.is_accepted());
        assert_eq!(document.revision.0, revision_before + 1);
        let node = selection.nodes.iter().next().unwrap().clone();

        let inverse = placed.undo.expect("undo payload");
        let undone = execute_inverse(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &inverse,
        )
        .expect("undo");
        assert!(undone.is_accepted());
        assert_eq!(document.revision.0, revision_before + 2);
        assert!(!document.graph.contains_node(&node));
    }

    #[test]
    fn connect_undo_removes_connection() {
        let mut document = MusaicDocument::new_empty();
        let mut board = fresh_board();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;

        let from = place_container(&mut document, &mut board, &mut attention, &mut selection);
        selection.clear();
        attention
            .focus(&DocumentQueries::new(&document), FocusTarget::None)
            .expect("clearing focus");

        let place_output = EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root_surface,
                slot: BoardSlot::new(2, 0),
            },
            tile: TileSpawnKind::Output {
                name: "main".into(),
            },
        };
        execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &place_output,
        )
        .expect("output");
        let to = selection.nodes.iter().next().unwrap().clone();

        let connect = EditorCommand::ConnectTiles {
            from: from.clone(),
            to: to.clone(),
        };
        let connected = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &connect,
        )
        .expect("connect");
        assert_eq!(
            DocumentQueries::new(&document)
                .connections_on_surface(document.root_surface)
                .len(),
            1
        );

        let inverse = connected.undo.expect("undo");
        execute_inverse(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &inverse,
        )
        .expect("disconnect");
        assert!(
            DocumentQueries::new(&document)
                .connections_on_surface(document.root_surface)
                .is_empty()
        );
    }

    #[test]
    fn bind_output_side_undo_restores_port_state() {
        use tessera::prelude::SpatialSide;

        let mut document = MusaicDocument::new_empty();
        let mut board = fresh_board();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let node = place_container(&mut document, &mut board, &mut attention, &mut selection);

        let bind = EditorCommand::BindOutputSide {
            node: node.clone(),
            side: SpatialSide::East,
        };
        let bound = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &bind,
        )
        .expect("bind");
        assert_eq!(
            document.port_endpoints.side_state(&node, SpatialSide::East),
            crate::application::editor::PortSlotState::Input
        );

        let inverse = bound.undo.expect("undo");
        execute_inverse(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &inverse,
        )
        .expect("restore");
        assert_eq!(
            document.port_endpoints.side_state(&node, SpatialSide::East),
            crate::application::editor::PortSlotState::None
        );
    }

    #[test]
    fn delete_undo_restores_subtree() {
        let mut document = MusaicDocument::new_empty();
        let mut board = fresh_board();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();

        let node = place_container(&mut document, &mut board, &mut attention, &mut selection);
        selection.select(
            node.clone(),
            crate::application::editor::SelectionMode::Replace,
        );

        let deleted = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::DeleteSelection,
        )
        .expect("delete");
        assert!(!document.graph.contains_node(&node));

        let inverse = deleted.undo.expect("undo");
        execute_inverse(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &inverse,
        )
        .expect("restore");
        assert!(document.graph.contains_node(&node));
    }

    #[test]
    fn navigate_is_ephemeral_and_history_records_mutations_only() {
        let mut history = CommandHistory::default();
        assert_eq!(
            EditorCommand::NavigateToSurface {
                surface: BoardSurfaceId(0)
            }
            .history_policy(),
            HistoryPolicy::Ephemeral
        );

        history.record_mutation(HistoryEntry {
            forward: EditorCommand::DeleteSelection,
            inverse: EditorInverse::RestoreSubtree {
                patch: Default::default(),
            },
            invalidation: Default::default(),
        });
        assert_eq!(history.undo_len(), 1);
    }

    #[test]
    fn redo_replays_forward_command() {
        let mut document = MusaicDocument::new_empty();
        let mut board = fresh_board();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let mut history = CommandHistory::default();

        let command = EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: document.root_surface,
                slot: BoardSlot::new(0, 0),
            },
            tile: TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        };
        let placed = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &command,
        )
        .expect("place");
        let inverse = placed.undo.clone().expect("inverse");
        history.record_mutation(HistoryEntry {
            forward: command.clone(),
            inverse: inverse.clone(),
            invalidation: placed.invalidation,
        });

        let entry = history.pop_undo().expect("undo entry");
        execute_inverse(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &entry.inverse,
        )
        .expect("undo execute");
        history.push_redo(entry);

        let redo_entry = history.pop_redo().expect("redo entry");
        execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &redo_entry.forward,
        )
        .expect("redo execute");
        assert_eq!(selection.nodes.len(), 1);
    }

    #[test]
    fn place_stores_document_changed_invalidation_for_undo() {
        let mut document = MusaicDocument::new_empty();
        let mut board = fresh_board();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();

        let surface = document.root_surface;
        let placed = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface,
                    slot: BoardSlot::new(0, 0),
                },
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        )
        .expect("place");

        let inv = placed.invalidation;
        assert!(inv.compile && inv.lower && inv.scene && inv.runtime && inv.save);
    }
}
