use bevy::prelude::*;
use bevy::state::condition::in_state;

use crate::{
    application::session::MusaicProject,
    infrastructure::diagnostics::{
        AppDiagnostic, DiagnosticPhase, DiagnosticStore, TransactionDiagnostic,
    },
};

use crate::application::command::{EditorCommand, EditorCommandBus};
use crate::application::editor::{
    EditorSession, MusaicEditorSet,
    panels::DrawerPanelState,
    workspace::{ActiveSurfaceChanged, EditorAttention},
};
use crate::domain::document::DocumentQueries;

use super::{
    blocks_board_picks_with_cursor, cursor::CursorInteraction, logic::classify_board_pick,
    types::BoardPickEvent,
};

/// Registers pointer/keyboard intent interpretation. Called by the editor plugin.
pub fn register_input_interpretation(app: &mut App) {
    // No-op when bevy_input already provided it; guarantees keyboard systems
    // have their input source in headless harnesses.
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_message::<BoardPickEvent>()
        .add_message::<ActiveSurfaceChanged>()
        .add_systems(
            Update,
            (
                super::keyboard::dispatch_delete_selection,
                super::keyboard::dispatch_undo_redo,
                super::keyboard::dispatch_transport_keys,
                super::keyboard::dispatch_cancel_keys,
            )
                .in_set(crate::infrastructure::app::MusaicSet::Input)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        )
        .add_systems(
            Update,
            (
                handle_board_pick_events,
                handle_shell_keyboard,
                clear_drawer_override_on_surface_change,
            )
                .chain()
                .in_set(MusaicEditorSet::InterpretInput)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
}

fn handle_board_pick_events(
    mut picks: MessageReader<'_, '_, BoardPickEvent>,
    project: Res<'_, MusaicProject>,
    session: Res<'_, EditorSession>,
    cursor: Res<'_, CursorInteraction>,
    attention: Res<'_, EditorAttention>,
    mut commands: MessageWriter<'_, EditorCommandBus>,
) {
    let queries = DocumentQueries::new(&project.document);

    for pick in picks.read() {
        if blocks_board_picks_with_cursor(&session, cursor.phase()) {
            continue;
        }

        if let Some(command) = classify_board_pick(&queries, &attention, &session, pick) {
            commands.write(EditorCommandBus(command));
        }
    }
}

fn clear_drawer_override_on_surface_change(
    mut surface_changes: MessageReader<'_, '_, ActiveSurfaceChanged>,
    mut drawer_panel: ResMut<'_, DrawerPanelState>,
) {
    if surface_changes.read().next().is_some() {
        drawer_panel.clear_override();
    }
}

fn handle_shell_keyboard(
    keyboard: Res<'_, ButtonInput<KeyCode>>,
    mut commands: MessageWriter<'_, EditorCommandBus>,
) {
    if keyboard.just_pressed(KeyCode::KeyD) {
        commands.write(EditorCommandBus(EditorCommand::ToggleDrawer));
    }
    if keyboard.just_pressed(KeyCode::KeyM) {
        commands.write(EditorCommandBus(EditorCommand::ToggleMinimap));
    }
}

pub fn apply_invalidation(
    runtime: &mut crate::application::pipeline::runtime::RuntimeState,
    result: &crate::application::editor::transaction::EditorTransactionResult,
) {
    if !result.is_accepted() {
        return;
    }

    if result.invalidation.needs_board_reexport {
        runtime.dirty.board_export = true;
    }
    if result.invalidation.compile {
        runtime.dirty.tessera = true;
    }
    if result.invalidation.lower {
        runtime.dirty.lowering = true;
    }
    if result.invalidation.scene {
        runtime.dirty.scene = true;
    }
    if result.invalidation.runtime {
        runtime.dirty.runtime = true;
    }
}

pub fn push_transaction_diagnostics(
    diagnostics: &mut DiagnosticStore,
    result: &crate::application::editor::transaction::EditorTransactionResult,
) {
    if result.diagnostics.is_empty() {
        return;
    }
    diagnostics.replace_phase(
        DiagnosticPhase::Transaction,
        result.diagnostics.iter().map(|diagnostic| {
            AppDiagnostic::Transaction(TransactionDiagnostic::Rejected {
                message: diagnostic.message.clone(),
            })
        }),
    );
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::super::types::{BoardPickHit, BoardPickTargetKind};
    use super::*;
    use crate::{
        application::command::execute_command,
        application::editor::{
            ActiveSpace, FocusTarget, InspectorLayout, InspectorPanelKind, SelectionState,
            TileLibraryContextKind, WorkspaceMode, basic_tile_options, derive_inspector_layout,
            transaction::PlacementTarget,
        },
        application::pipeline::PlaybackPlugin,
        application::pipeline::runtime::{ProjectedEventId, TimelineProvenanceStore},
        domain::board::BoardSlot,
        domain::document::{AtomValue, ContainerKind, NoteName, StackIndex, TileSpawnKind},
        infrastructure::app::{AppState, TransportMode},
    };
    use tessera::bevy::{TesseraBoard, TesseraPlugin};

    fn slot_pick(surface: crate::domain::board::BoardSurfaceId, slot: BoardSlot) -> BoardPickEvent {
        BoardPickEvent::Hit(BoardPickHit {
            surface_id: surface,
            kind: BoardPickTargetKind::Slot { slot },
            world_position: Vec3::ZERO,
            distance: 0.0,
        })
    }

    fn test_app() -> App {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_state::<TransportMode>()
            .add_plugins(MinimalPlugins)
            .add_plugins((
                TesseraPlugin,
                crate::application::editor::EditorPlugin,
                PlaybackPlugin,
            ));
        app.insert_state(AppState::Editor);
        app
    }

    #[test]
    fn board_click_focuses_empty_slot() {
        let mut app = test_app();
        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;

        app.world_mut()
            .write_message(slot_pick(root_surface, BoardSlot::new(2, 1)));
        app.update();

        let attention = app.world().resource::<EditorAttention>();
        assert_eq!(
            attention.focus,
            crate::application::editor::workspace::FocusTarget::EmptySlot {
                surface: root_surface,
                slot: BoardSlot::new(2, 1),
            }
        );
    }

    #[test]
    fn context_panel_place_tile_creates_container_from_focused_root_slot() {
        let mut app = test_app();
        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;

        app.world_mut()
            .write_message(slot_pick(root_surface, BoardSlot::new(0, 0)));
        app.update();

        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root_surface,
                    slot: BoardSlot::new(0, 0),
                },
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            }));
        app.update();
        app.update();

        let project = app.world().resource::<MusaicProject>();
        let selection = app.world().resource::<SelectionState>();
        assert!(project.document.revision.0 >= 1);
        assert_eq!(selection.nodes.len(), 1);
    }

    #[test]
    fn start_connection_then_click_second_tile_creates_connection() {
        let mut app = test_app();
        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;

        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root_surface,
                    slot: BoardSlot::new(0, 0),
                },
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            }));
        app.update();
        let first = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root_surface,
                    slot: BoardSlot::new(2, 0),
                },
                tile: TileSpawnKind::Output {
                    name: "main".into(),
                },
            }));
        app.update();
        let second = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();
        assert_ne!(
            first, second,
            "the output should occupy its own non-overlapping 2×2 footprint"
        );

        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::StartConnection {
                source: first.clone(),
            }));
        app.update();
        app.world_mut()
            .write_message(BoardPickEvent::Hit(BoardPickHit {
                surface_id: root_surface,
                kind: BoardPickTargetKind::BoardTile {
                    tile_id: second.clone(),
                },
                world_position: Vec3::ZERO,
                distance: 0.0,
            }));
        app.update();
        let project = app.world().resource::<MusaicProject>();
        let connections =
            DocumentQueries::new(&project.document).connections_on_surface(root_surface);
        assert_eq!(connections.len(), 1);
        let connection = &connections[0];
        assert_eq!(connection.from, first);
        assert_eq!(connection.to, second);
    }

    #[test]
    fn clicking_focused_container_again_enters_its_local_surface() {
        let mut app = test_app();
        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;

        app.world_mut()
            .write_message(slot_pick(root_surface, BoardSlot::new(0, 0)));
        app.update();

        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root_surface,
                    slot: BoardSlot::new(0, 0),
                },
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            }));
        app.update();

        let container = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();
        let local_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .graph
            .container_surface(&container)
            .unwrap();

        app.world_mut()
            .write_message(BoardPickEvent::Hit(BoardPickHit {
                surface_id: root_surface,
                kind: BoardPickTargetKind::BoardTile {
                    tile_id: container.clone(),
                },
                world_position: Vec3::ZERO,
                distance: 0.0,
            }));
        app.update();

        let attention = app.world().resource::<EditorAttention>();
        assert_eq!(attention.active_space, ActiveSpace::Board(local_surface));
        assert_eq!(attention.focus, FocusTarget::None);
    }

    #[test]
    fn timeline_focus_and_jump_to_source_round_trip_through_messages() {
        let mut app = test_app();
        let root_surface = app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface;

        app.world_mut()
            .write_message(slot_pick(root_surface, BoardSlot::new(0, 0)));
        app.update();
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root_surface,
                    slot: BoardSlot::new(0, 0),
                },
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            }));
        app.update();

        let source_node = app
            .world()
            .resource::<SelectionState>()
            .nodes
            .iter()
            .next()
            .unwrap()
            .clone();
        {
            let mut provenance = app.world_mut().resource_mut::<TimelineProvenanceStore>();
            provenance.insert_source(ProjectedEventId(5), root_surface, source_node.clone());
        }

        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::EnterTimelineMode));
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::Focus {
                target: FocusTarget::TimelineEvent {
                    event: ProjectedEventId(5),
                },
            }));
        app.update();

        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::JumpToTimelineSource {
                event: ProjectedEventId(5),
            }));
        app.update();

        let attention = app.world().resource::<EditorAttention>();
        assert_eq!(attention.workspace_mode, WorkspaceMode::Compose);
        assert_eq!(
            attention.active_space,
            crate::application::editor::ActiveSpace::Board(root_surface)
        );
        assert_eq!(
            attention.focus,
            crate::application::editor::workspace::FocusTarget::Tile { node: source_node }
        );
    }

    #[test]
    fn first_authoring_ritual_places_atom_inside_container() {
        let mut document = crate::domain::document::MusaicDocument::new_empty();
        let mut board = TesseraBoard::new();
        let mut attention =
            crate::application::editor::workspace::EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let provenance = TimelineProvenanceStore::default();
        let root_surface = document.root_surface;
        let root_slot = BoardSlot::new(0, 0);

        let queries = DocumentQueries::new(&document);
        attention
            .focus(
                &queries,
                FocusTarget::EmptySlot {
                    surface: root_surface,
                    slot: root_slot,
                },
            )
            .unwrap();
        let layout = derive_inspector_layout(&attention, &selection, &queries, true);
        assert_eq!(
            layout,
            InspectorLayout::single(InspectorPanelKind::DrawerPanel {
                target: Some(PlacementTarget::BoardSlot {
                    surface: root_surface,
                    slot: root_slot,
                }),
                context: TileLibraryContextKind::RootBoard,
            })
        );

        let root_options = basic_tile_options(TileLibraryContextKind::RootBoard);
        assert!(root_options.iter().any(|item| {
            matches!(
                item.spawn,
                TileSpawnKind::Container {
                    kind: ContainerKind::Sequence
                }
            )
        }));

        let result = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root_surface,
                    slot: root_slot,
                },
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        )
        .unwrap();
        assert!(result.is_accepted());

        let container = selection.nodes.iter().next().unwrap().clone();
        assert_eq!(
            attention.focus,
            FocusTarget::Tile {
                node: container.clone()
            }
        );
        let local_surface = document.graph.container_surface(&container).unwrap();

        let result = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::EnterContainer {
                container: container.clone(),
            },
        )
        .unwrap();
        assert!(result.is_accepted());
        assert_eq!(attention.active_space, ActiveSpace::Board(local_surface));
        assert_eq!(attention.focus, FocusTarget::None);

        let atom_slot = BoardSlot::new(0, 0);
        let queries = DocumentQueries::new(&document);
        attention
            .focus(
                &queries,
                FocusTarget::EmptySlot {
                    surface: local_surface,
                    slot: atom_slot,
                },
            )
            .unwrap();
        let layout = derive_inspector_layout(&attention, &selection, &queries, true);
        assert_eq!(
            layout,
            InspectorLayout::single(InspectorPanelKind::DrawerPanel {
                target: Some(PlacementTarget::BoardSlot {
                    surface: local_surface,
                    slot: atom_slot,
                }),
                context: TileLibraryContextKind::ContainerBody,
            })
        );

        let atom_options = basic_tile_options(TileLibraryContextKind::ContainerBody);
        assert!(atom_options.iter().any(|item| {
            matches!(
                item.spawn,
                TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(NoteName::A)
                }
            )
        }));

        let result = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: local_surface,
                    slot: atom_slot,
                },
                tile: TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(NoteName::A),
                },
            },
        )
        .unwrap();
        assert!(result.is_accepted());
        assert_eq!(
            attention.focus,
            FocusTarget::StackInsert {
                surface: local_surface,
                index: StackIndex(1),
            }
        );
    }

    #[test]
    fn breadcrumb_navigation_request_returns_to_root_surface() {
        let mut document = crate::domain::document::MusaicDocument::new_empty();
        let mut board = TesseraBoard::new();
        let provenance = TimelineProvenanceStore::default();
        let mut attention =
            crate::application::editor::workspace::EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let root_surface = document.root_surface;

        let result = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root_surface,
                    slot: BoardSlot::new(0, 0),
                },
                tile: TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            },
        )
        .unwrap();
        assert!(result.is_accepted());

        let container = selection.nodes.iter().next().unwrap().clone();
        let local_surface = document.graph.container_surface(&container).unwrap();
        let result = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::EnterContainer {
                container: container.clone(),
            },
        )
        .unwrap();
        assert!(result.is_accepted());
        assert_eq!(attention.active_space, ActiveSpace::Board(local_surface));

        let result = execute_command(
            &mut document,
            &mut board,
            &mut attention,
            &mut selection,
            &provenance,
            &EditorCommand::NavigateToSurface {
                surface: root_surface,
            },
        )
        .unwrap();
        assert!(result.is_accepted());
        assert_eq!(attention.active_space, ActiveSpace::Board(root_surface));
        assert_eq!(attention.workspace_mode, WorkspaceMode::Compose);
        assert!(selection.nodes.is_empty());
    }
}
