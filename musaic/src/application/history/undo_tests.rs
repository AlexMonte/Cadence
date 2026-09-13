#[cfg(test)]
mod tests {
    use tessera::prelude::{NodeId, SpatialSide};

    use crate::application::command::{
        EditorCommand, EditorInverse, execute_command, execute_inverse,
    };
    use crate::application::editor::transaction::PlacementTarget;
    use crate::application::editor::{EditorAttention, FocusTarget, SelectionState};
    use crate::application::history::{CommandHistory, HistoryEntry, HistoryPolicy};
    use crate::application::pipeline::runtime::TimelineProvenanceStore;
    use crate::domain::board::{BoardSlot, BoardSurfaceId};
    use crate::domain::document::{
        ContainerKind, DocumentQueries, MusaicDocument, PortSlotState, TileSpawnKind,
        connection_policy, export_document_program, port_config, port_state_for_side,
    };

    fn place_container(
        document: &mut MusaicDocument,
        attention: &mut EditorAttention,
        selection: &mut SelectionState,
    ) -> NodeId {
        let provenance = TimelineProvenanceStore::default();
        let command = EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: document.root_surface,
                slot: BoardSlot::new(0, 0),
            },
            tile: TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        };
        let result = execute_command(document, attention, selection, &provenance, &command)
            .expect("place container");
        assert!(result.is_accepted(), "{:?}", result.diagnostics);
        assert_eq!(command.history_policy(), HistoryPolicy::RecordMutation);
        selection.nodes.iter().next().unwrap().clone()
    }

    fn side_state(document: &MusaicDocument, node: &NodeId, side: SpatialSide) -> PortSlotState {
        let program = export_document_program(document).expect("export current document");
        let bindings = connection_policy::effective_bindings(&program, node);
        port_state_for_side(&port_config(&bindings), side)
    }

    #[test]
    fn place_undo_restores_document_revision() {
        let mut document = MusaicDocument::new_empty();
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
        let undone =
            execute_inverse(&mut document, &mut attention, &mut selection, &inverse).expect("undo");
        assert!(undone.is_accepted());
        assert_eq!(document.revision.0, revision_before + 2);
        assert!(!document.graph.contains_node(&node));
        assert!(document.validate().is_ok());
    }

    #[test]
    fn connect_undo_restores_canonical_connections() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;

        let from = place_container(&mut document, &mut attention, &mut selection);
        selection.clear();
        attention
            .focus(&DocumentQueries::new(&document), FocusTarget::None)
            .expect("clear focus");
        execute_command(
            &mut document,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root_surface,
                    slot: BoardSlot::new(5, 0),
                },
                tile: TileSpawnKind::Output {
                    name: "main".into(),
                },
            },
        )
        .expect("place output");
        let to = selection.nodes.iter().next().unwrap().clone();

        crate::application::editor::transaction::disconnect_tiles(&mut document, &from, &to);
        let before = document.connections.clone();
        let connected = execute_command(
            &mut document,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::ConnectTiles {
                from: from.clone(),
                to: to.clone(),
            },
        )
        .expect("connect");
        assert!(connected.is_accepted(), "{:?}", connected.diagnostics);
        assert_eq!(
            DocumentQueries::new(&document)
                .connections_on_surface(document.root_surface)
                .len(),
            1
        );

        execute_inverse(
            &mut document,
            &mut attention,
            &mut selection,
            &connected.undo.expect("connection inverse"),
        )
        .expect("undo connect");
        assert_eq!(document.connections, before);
        assert!(
            DocumentQueries::new(&document)
                .connections_on_surface(document.root_surface)
                .is_empty()
        );
    }

    #[test]
    fn bind_output_side_undo_restores_derived_port_presentation() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let node = place_container(&mut document, &mut attention, &mut selection);
        let before = document.connections.clone();
        let before_side = side_state(&document, &node, SpatialSide::East);

        let bound = execute_command(
            &mut document,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::BindOutputSide {
                node: node.clone(),
                side: SpatialSide::East,
            },
        )
        .expect("cycle side");
        assert!(bound.is_accepted());
        assert_ne!(document.connections, before);
        assert_ne!(side_state(&document, &node, SpatialSide::East), before_side);

        execute_inverse(
            &mut document,
            &mut attention,
            &mut selection,
            &bound.undo.expect("side inverse"),
        )
        .expect("undo side cycle");
        assert_eq!(document.connections, before);
        assert_eq!(side_state(&document, &node, SpatialSide::East), before_side);
    }

    #[test]
    fn delete_undo_restores_subtree_and_connections() {
        let mut document = MusaicDocument::new_empty();
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root = document.root_surface;
        let source = place_container(&mut document, &mut attention, &mut selection);
        execute_command(
            &mut document,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root,
                    slot: BoardSlot::new(5, 0),
                },
                tile: TileSpawnKind::Output {
                    name: "main".into(),
                },
            },
        )
        .expect("place output");
        let output = selection.nodes.iter().next().unwrap().clone();
        let before_delete = document.clone();

        let deleted = execute_command(
            &mut document,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::DeleteSelection,
        )
        .expect("delete");
        assert!(!document.graph.contains_node(&output));

        execute_inverse(
            &mut document,
            &mut attention,
            &mut selection,
            &deleted.undo.expect("subtree inverse"),
        )
        .expect("restore");
        assert!(document.graph.contains_node(&source));
        assert!(document.graph.contains_node(&output));
        assert_eq!(document.connections, before_delete.connections);
        assert!(document.validate().is_ok());
    }

    #[test]
    fn navigation_is_ephemeral_and_history_records_mutations_only() {
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
            &mut attention,
            &mut selection,
            &provenance,
            &command,
        )
        .expect("place");
        history.record_mutation(HistoryEntry {
            forward: command.clone(),
            inverse: placed.undo.clone().expect("inverse"),
            invalidation: placed.invalidation,
        });

        let entry = history.pop_undo().expect("undo entry");
        execute_inverse(
            &mut document,
            &mut attention,
            &mut selection,
            &entry.inverse,
        )
        .expect("execute undo");
        history.push_redo(entry);

        let redo_entry = history.pop_redo().expect("redo entry");
        execute_command(
            &mut document,
            &mut attention,
            &mut selection,
            &provenance,
            &redo_entry.forward,
        )
        .expect("execute redo");
        assert_eq!(selection.nodes.len(), 1);
    }
}
