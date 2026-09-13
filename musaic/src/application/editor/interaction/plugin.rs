use crate::application::editor::preferences::keymap::Action;
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

use super::{blocks_board_picks_with_cursor, cursor::CursorInteraction, types::BoardPickEvent};

/// Registers pointer/keyboard intent interpretation. Called by the editor plugin.
pub fn register_input_interpretation(app: &mut App) {
    app.add_plugins(super::keyboard_navigation::plugin);
    // No-op when bevy_input already provided it; guarantees keyboard systems
    // have their input source in headless harnesses.
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_message::<BoardPickEvent>()
        .add_message::<ActiveSurfaceChanged>()
        .add_systems(
            Update,
            (
                super::keyboard::dispatch_delete_selection,
                super::keyboard::dispatch_edit_keys,
                super::keyboard::dispatch_undo_redo,
                super::keyboard::dispatch_transport_keys,
                super::keyboard::dispatch_cancel_keys,
            )
                .in_set(crate::infrastructure::app::MusaicSet::Input)
                .run_if(super::keyboard_navigation::shortcuts_available)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        )
        .add_systems(
            Update,
            (
                handle_board_pick_events,
                handle_shell_keyboard.run_if(super::keyboard_navigation::shortcuts_available),
                clear_drawer_override_on_surface_change,
            )
                .chain()
                .in_set(MusaicEditorSet::InterpretInput)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
}

fn handle_board_pick_events(
    mut keyboard_focus: Option<ResMut<bevy::input_focus::InputFocus>>,
    mut picks: MessageReader<'_, '_, BoardPickEvent>,
    project: Res<'_, MusaicProject>,
    session: Res<'_, EditorSession>,
    cursor: Res<'_, CursorInteraction>,
    attention: Res<'_, EditorAttention>,
    keyboard: Res<'_, ButtonInput<KeyCode>>,
    mut commands: MessageWriter<'_, EditorCommandBus>,
) {
    let queries = DocumentQueries::new(&project.document);

    for pick in picks.read() {
        if let Some(focus) = keyboard_focus.as_mut() {
            focus.clear();
        }
        if blocks_board_picks_with_cursor(&session, cursor.phase()) {
            continue;
        }

        if let Some(command) = super::logic::classify_board_pick_with_modifiers(
            &queries,
            &attention,
            &session,
            pick,
            super::picking::PickModifiers {
                additive: keyboard.pressed(KeyCode::ShiftLeft)
                    || keyboard.pressed(KeyCode::ShiftRight),
                toggle: keyboard.pressed(KeyCode::SuperLeft)
                    || keyboard.pressed(KeyCode::ControlLeft),
            },
        ) {
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
    preferences: Res<'_, crate::application::editor::preferences::EditorPreferences>,
    mut commands: MessageWriter<'_, EditorCommandBus>,
) {
    if preferences.shortcut(Action::Library, &keyboard).is_some() {
        commands.write(EditorCommandBus(EditorCommand::ToggleDrawer));
    }
    if preferences.shortcut(Action::Minimap, &keyboard).is_some() {
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

    let composition = result.invalidation.document || result.invalidation.compile;
    if composition {
        runtime.mark_composition_changed();
    } else if result.invalidation.lower {
        runtime.mark_sound_changed();
    }
    if result.invalidation.scene && !composition {
        runtime.mark_presentation_changed();
    }
    if result.invalidation.runtime && !composition && !result.invalidation.lower {
        runtime.mark_transport_changed();
    }
}

pub fn push_transaction_diagnostics(
    diagnostics: &mut DiagnosticStore,
    result: &crate::application::editor::transaction::EditorTransactionResult,
) {
    diagnostics.replace_phase(
        DiagnosticPhase::Transaction,
        result.diagnostics.iter().map(|diagnostic| {
            AppDiagnostic::Transaction(match diagnostic.severity {
                crate::application::editor::DiagnosticSeverity::Info => {
                    TransactionDiagnostic::Info {
                        message: diagnostic.message.clone(),
                    }
                }
                _ => TransactionDiagnostic::Rejected {
                    message: diagnostic.message.clone(),
                },
            })
        }),
    );
}

#[cfg(test)]
mod tests {
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
        application::pipeline::runtime::TimelineProvenanceStore,
        domain::board::BoardSlot,
        domain::document::{AtomValue, ContainerKind, NoteName, StackIndex, TileSpawnKind},
        infrastructure::app::{AppState, TransportMode},
    };

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
            .add_plugins((crate::application::editor::EditorPlugin, PlaybackPlugin));
        crate::infrastructure::app::configure_pipeline_schedule(&mut app);
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
                    slot: BoardSlot::new(5, 0),
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
            "the output should occupy its own non-overlapping single-cell footprint"
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
    fn timing_preview_stays_open_while_placing_notes_and_undoing_them() {
        use crate::application::{
            editor::{TimelinePanelState, WorkspaceLayoutKind},
            pipeline::ui_projection::EditorUiProjection,
        };
        use crate::domain::document::{DocumentNodeKind, StackIndex};
        let mut app = test_app();
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::AdoptProject {
                project: MusaicProject::demo(),
                path: None,
            }));
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::EnterTimelineMode));
        for _ in 0..4 {
            app.update();
        }
        let project = app.world().resource::<MusaicProject>();
        let container = project
            .document
            .graph
            .nodes_on_surface(project.document.root_surface)
            .into_iter()
            .find(|(_, node)| matches!(node.kind, DocumentNodeKind::Container(_)))
            .unwrap()
            .1
            .id
            .clone();
        let surface = project
            .document
            .graph
            .container_surface(&container)
            .unwrap();
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::EnterContainer {
                container,
            }));
        for (index, atom) in [
            (10, AtomValue::NoteName(NoteName::D)),
            (11, AtomValue::Octave(4)),
        ] {
            app.world_mut()
                .write_message(EditorCommandBus(EditorCommand::PlaceTile {
                    target: PlacementTarget::StackIndex {
                        surface,
                        index: StackIndex(index),
                    },
                    tile: TileSpawnKind::Atom { atom },
                }));
        }
        for _ in 0..4 {
            app.update();
        }
        assert!(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .node_at_stack_index(surface, StackIndex(10))
                .is_some(),
            "Opening the timing preview must not disable tile authoring"
        );
        assert!(app.world().resource::<TimelinePanelState>().open);
        let projection = app.world().resource::<EditorUiProjection>();
        assert_eq!(
            projection.layout_kind,
            WorkspaceLayoutKind::TimelineStackedOverBoardInspector
        );
        assert_eq!(
            projection.timeline_paint.start_count, 14,
            "{:?}",
            projection.diagnostics_summary
        );
        for _ in 0..2 {
            app.world_mut()
                .write_message(EditorCommandBus(EditorCommand::Undo));
            app.update();
        }
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(
            app.world()
                .resource::<EditorUiProjection>()
                .timeline_paint
                .start_count,
            12
        );
        assert!(app.world().resource::<TimelinePanelState>().open);
    }

    #[test]
    fn timeline_focus_and_jump_to_source_round_trip_through_messages() {
        use crate::application::{
            editor::TimelineSourceResolver, pipeline::runtime::RuntimePreviewSnapshot,
        };
        let mut app = test_app();
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::AdoptProject {
                project: MusaicProject::demo(),
                path: None,
            }));
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::EnterTimelineMode));
        for _ in 0..4 {
            app.update();
        }
        let event = app
            .world()
            .resource::<RuntimePreviewSnapshot>()
            .starts()
            .next()
            .expect("compiled demo preview")
            .id;
        let source = app
            .world()
            .resource::<TimelineProvenanceStore>()
            .source_for_event(event)
            .expect("authored source");
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::Focus {
                target: FocusTarget::TimelineEvent { event },
            }));
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::JumpToTimelineSource {
                event,
            }));
        app.update();
        let attention = app.world().resource::<EditorAttention>();
        assert_eq!(attention.workspace_mode, WorkspaceMode::Compose);
        assert_eq!(attention.active_space, ActiveSpace::Board(source.surface));
        assert_eq!(attention.focus, FocusTarget::Atom { node: source.node });
    }

    #[test]
    fn first_authoring_ritual_places_atom_inside_container() {
        let mut document = crate::domain::document::MusaicDocument::new_empty();
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
        let provenance = TimelineProvenanceStore::default();
        let mut attention =
            crate::application::editor::workspace::EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        let root_surface = document.root_surface;

        let result = execute_command(
            &mut document,
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
