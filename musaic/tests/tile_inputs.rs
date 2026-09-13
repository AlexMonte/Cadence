//! Pointer-session and contextual-drop acceptance beyond direct tile placement.
use bevy::{prelude::*, window::PrimaryWindow};
use musaic::{
    adapter::persistence::{export_project_bytes, import_project_bytes},
    application::{
        command::{EditorCommand, EditorCommandBus, PlacementTarget, editing::TileEdit},
        editor::{
            BoardPlacementPointer, DrawerTilePressQueue, DrawerTilePressed, EditorPlugin,
            EditorSession,
            interaction::{BoardTilePressQueue, BoardTilePressed},
        },
        pipeline::{PlaybackPlugin, scene_sync::VisibleBoardState},
        session::MusaicProject,
    },
    domain::{
        board::{
            BoardSlot, BoardSurfaceId,
            geometry::{stack_column_center, stack_row_center},
        },
        document::{
            self, AtomValue, ContainerKind, DocumentNodeKind, NodeLocation, NoteName,
            PlacementAddress, StackIndex, TileSpawnKind,
        },
    },
    infrastructure::app::{AppState, MusaicSet, TransportMode},
};
use tessera::prelude::{AtomModifier, NodeId, Rational};
fn number(n: i32) -> TileSpawnKind {
    TileSpawnKind::Atom {
        atom: AtomValue::Number(n),
    }
}
fn fixture(atoms: Vec<AtomValue>) -> (App, BoardSurfaceId, Vec<NodeId>, Entity) {
    let mut project = MusaicProject::new_empty();
    let root = project.document.root_surface;
    let container = project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        )
        .unwrap();
    let surface = project
        .document
        .graph
        .container_surface(&container)
        .unwrap();
    let ids = atoms
        .into_iter()
        .enumerate()
        .map(|(i, atom)| {
            project
                .document
                .graph
                .insert_tile(
                    &mut project.document.surfaces,
                    surface,
                    PlacementAddress::StackIndex(StackIndex(i)),
                    TileSpawnKind::Atom { atom },
                )
                .unwrap()
        })
        .collect();
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins(MinimalPlugins)
        .add_plugins((EditorPlugin, PlaybackPlugin));
    app.configure_sets(
        Update,
        (
            MusaicSet::Input,
            MusaicSet::Commands,
            MusaicSet::DocumentMutation,
            MusaicSet::Compile,
            MusaicSet::Lower,
            MusaicSet::Runtime,
            MusaicSet::SceneSync,
            MusaicSet::RenderUi,
        )
            .chain(),
    )
    .configure_sets(
        Update,
        (
            cadence::bevy::CadenceSet::ReplaceScores.in_set(MusaicSet::Runtime),
            cadence::bevy::CadenceSet::Tick.in_set(MusaicSet::Runtime),
        ),
    );
    app.init_resource::<ButtonInput<MouseButton>>();
    let window = app
        .world_mut()
        .spawn((
            Window {
                focused: true,
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    app.insert_state(AppState::Editor);
    send(
        &mut app,
        EditorCommand::AdoptProject {
            project,
            path: None,
        },
    );
    send(&mut app, EditorCommand::EnterContainer { container });
    (app, surface, ids, window)
}
fn tick(app: &mut App) {
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .clear();
}
fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    for _ in 0..3 {
        tick(app);
    }
}
fn layout(app: &App, surface: BoardSurfaceId) -> Vec<(NodeId, NodeLocation, DocumentNodeKind)> {
    let mut nodes = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .nodes_on_surface(surface)
        .into_iter()
        .map(|(loc, node)| (node.id.clone(), loc, node.kind.clone()))
        .collect::<Vec<_>>();
    nodes.sort_by_key(|(_, loc, _)| match loc.address {
        PlacementAddress::StackIndex(i) => i.0,
        _ => 0,
    });
    nodes
}
fn point(app: &mut App, window: Entity, screen: Vec2, display: Option<usize>) {
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(screen));
    app.world_mut()
        .resource_mut::<BoardPlacementPointer>()
        .cursor_world =
        display.map(|i| Vec3::new(stack_column_center(i % 12), 0., stack_row_center(i)));
}
fn press(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
}
fn release(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    tick(app);
}

#[test]
fn dropping_numbers_sets_and_replaces_owned_octave_without_touching_modifiers_or_next_note() {
    let (mut app, surface, ids, _) = fixture(vec![
        AtomValue::NoteName(NoteName::C),
        AtomValue::Modifier(AtomModifier::Fast(Rational::from_integer(3))),
        AtomValue::NoteName(NoteName::D),
    ]);
    let before = layout(&app, surface);
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(0),
            },
            tile: number(4),
        },
    );
    let after = layout(&app, surface);
    assert_eq!(after.len(), before.len() + 1);
    let octave = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .node_at_stack_index(surface, StackIndex(1))
        .unwrap();
    for n in [5, 2, -1, 9] {
        send(
            &mut app,
            EditorCommand::PlaceTile {
                target: PlacementTarget::StackIndex {
                    surface,
                    index: StackIndex(0),
                },
                tile: number(n),
            },
        );
        assert_eq!(layout(&app, surface).len(), after.len());
        assert!(
            matches!(&app.world().resource::<MusaicProject>().document.graph.node(&octave).unwrap().kind,DocumentNodeKind::Atom(a) if a.atom==AtomValue::Octave(n as i8))
        );
    }
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .node(&ids[1])
            .unwrap()
            .kind,
        before[1].2
    );
    let valid = layout(&app, surface);
    for n in [-2, 10] {
        send(
            &mut app,
            EditorCommand::PlaceTile {
                target: PlacementTarget::StackIndex {
                    surface,
                    index: StackIndex(0),
                },
                tile: number(n),
            },
        );
        assert_eq!(layout(&app, surface), valid);
    }
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(0),
            },
            tile: TileSpawnKind::Atom {
                atom: AtomValue::Ratio(Rational::new(9, 2)),
            },
        },
    );
    assert_eq!(layout(&app, surface), valid);
    let bytes = export_project_bytes(app.world().resource::<MusaicProject>()).unwrap();
    let restored = import_project_bytes(&bytes, None).unwrap();
    assert_eq!(
        restored.document.graph.node(&octave).unwrap().kind,
        valid[1].2
    );
    for _ in 0..5 {
        send(&mut app, EditorCommand::Undo);
    }
    assert_eq!(layout(&app, surface), before);
    for _ in 0..5 {
        send(&mut app, EditorCommand::Redo);
    }
    assert_eq!(layout(&app, surface), valid);
}
#[test]
fn a_fast_library_drag_commits_current_release_position_and_leaving_board_cancels() {
    let (mut app, surface, _, window) = fixture(vec![AtomValue::NoteName(NoteName::C)]);
    point(&mut app, window, Vec2::ZERO, None);
    press(&mut app);
    app.world_mut()
        .resource_mut::<DrawerTilePressQueue>()
        .pending = Some(DrawerTilePressed {
        tile: number(5),
        start_screen: Vec2::ZERO,
    });
    tick(&mut app);
    // Threshold and release can occur in the same frame. It must not wait for a
    // future release or use an earlier hover position.
    point(&mut app, window, Vec2::new(200., 100.), Some(0));
    release(&mut app);
    assert!(matches!(
        app.world().resource::<EditorSession>().mode,
        musaic::application::editor::EditorMode::Idle
    ));
    assert!(layout(&app, surface).iter().any(
        |(_, _, kind)| matches!(kind,DocumentNodeKind::Atom(a) if a.atom==AtomValue::Octave(5))
    ));
    let before = layout(&app, surface);
    point(&mut app, window, Vec2::ZERO, None);
    press(&mut app);
    app.world_mut()
        .resource_mut::<DrawerTilePressQueue>()
        .pending = Some(DrawerTilePressed {
        tile: number(6),
        start_screen: Vec2::ZERO,
    });
    tick(&mut app);
    point(&mut app, window, Vec2::new(200., 100.), Some(0));
    tick(&mut app);
    point(&mut app, window, Vec2::new(800., 100.), None);
    release(&mut app);
    assert_eq!(
        layout(&app, surface),
        before,
        "Outside release must not commit the last valid hover"
    );
}
#[test]
fn dragging_a_note_moves_its_owned_stack_and_reorders_without_recreating_ids() {
    let (mut app, surface, ids, window) = fixture(vec![
        AtomValue::NoteName(NoteName::C),
        AtomValue::Octave(4),
        AtomValue::Modifier(AtomModifier::Fast(Rational::from_integer(3))),
        AtomValue::NoteName(NoteName::E),
        AtomValue::Octave(5),
    ]);
    let before = layout(&app, surface);
    point(&mut app, window, Vec2::new(100., 100.), Some(1));
    press(&mut app);
    app.world_mut()
        .resource_mut::<BoardTilePressQueue>()
        .pending = Some(BoardTilePressed {
        node: ids[3].clone(),
        start_screen: Vec2::new(100., 100.),
    });
    tick(&mut app);
    point(&mut app, window, Vec2::new(250., 100.), Some(0));
    release(&mut app);
    let after = layout(&app, surface);
    assert_eq!(
        after
            .iter()
            .map(|(id, _, _)| id.clone())
            .collect::<Vec<_>>(),
        vec![
            ids[3].clone(),
            ids[4].clone(),
            ids[0].clone(),
            ids[1].clone(),
            ids[2].clone()
        ]
    );
    assert_eq!(
        app.world()
            .resource::<musaic::application::editor::SelectionState>()
            .nodes,
        std::collections::BTreeSet::from([ids[3].clone()])
    );
    assert!(
        musaic::application::editor::connection_endpoint_view(
            &document::DocumentQueries::new(&app.world().resource::<MusaicProject>().document),
            &ids[3],
            None,
            None,
        )
        .is_none(),
        "Pattern notes must not offer root-board port controls"
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(layout(&app, surface), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(layout(&app, surface), after);
    let display = app
        .world()
        .resource::<VisibleBoardState>()
        .display_address(PlacementAddress::StackIndex(StackIndex(5)));
    assert!(matches!(display, PlacementAddress::StackIndex(_)));
    send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::Move {
            node: ids[3].clone(),
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(12),
            },
        }),
    );
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .location_of(&ids[4])
            .unwrap()
            .address,
        PlacementAddress::StackIndex(StackIndex(13))
    );
}

#[test]
fn moving_an_existing_number_onto_a_note_consumes_it_and_undo_restores_bindings() {
    let (mut app, surface, ids, _) =
        fixture(vec![AtomValue::NoteName(NoteName::C), AtomValue::Octave(4)]);
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    let slot = BoardSlot::new(4, 4);
    send(&mut app, EditorCommand::NavigateToSurface { surface: root });
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot,
            },
            tile: number(5),
        },
    );
    let number_id = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .node_at_board_slot(root, slot)
        .unwrap();
    let before = app.world().resource::<MusaicProject>().clone();
    send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::Move {
            node: number_id.clone(),
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(0),
            },
        }),
    );
    let document = &app.world().resource::<MusaicProject>().document;
    assert!(document.graph.node(&number_id).is_none());
    assert!(!document.connections.bindings.contains_key(&number_id));
    for (node, binding) in &document.connections.bindings {
        assert_eq!(
            before.document.connections.bindings.get(node),
            Some(binding)
        );
    }
    assert_eq!(
        document.connections.explicit_relations,
        before.document.connections.explicit_relations
    );
    assert!(matches!(&document.graph.node(&ids[1]).unwrap().kind,
        DocumentNodeKind::Atom(a) if a.atom == AtomValue::Octave(5)));
    send(&mut app, EditorCommand::Undo);
    let restored = &app.world().resource::<MusaicProject>().document;
    assert_eq!(restored.graph, before.document.graph);
    assert_eq!(restored.connections, before.document.connections);
    send(&mut app, EditorCommand::Redo);
    assert!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .node(&number_id)
            .is_none()
    );
}

#[test]
fn escape_and_release_in_the_same_frame_cancels() {
    let (mut app, surface, _, window) = fixture(vec![AtomValue::NoteName(NoteName::C)]);
    let before = layout(&app, surface);
    point(&mut app, window, Vec2::ZERO, None);
    press(&mut app);
    app.world_mut()
        .resource_mut::<DrawerTilePressQueue>()
        .pending = Some(DrawerTilePressed {
        tile: number(5),
        start_screen: Vec2::ZERO,
    });
    tick(&mut app);
    point(&mut app, window, Vec2::new(200., 100.), Some(0));
    tick(&mut app);
    assert!(
        app.world()
            .resource::<EditorSession>()
            .is_placing_from_drawer()
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    release(&mut app);
    assert_eq!(
        layout(&app, surface),
        before,
        "Escape and release in the same frame should cancel"
    );
}
#[test]
fn reordering_across_a_gap_preserves_other_owned_notes() {
    let (mut app, surface, ids, _) = fixture(vec![
        AtomValue::NoteName(NoteName::C),
        AtomValue::Octave(4),
        AtomValue::NoteName(NoteName::D),
    ]);
    send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::Move {
            node: ids[2].clone(),
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(3),
            },
        }),
    );
    let before_reorder = layout(&app, surface);
    send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::Move {
            node: ids[2].clone(),
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(0),
            },
        }),
    );
    let document = &app.world().resource::<MusaicProject>().document;
    let pitch = document.graph.location_of(&ids[0]).unwrap();
    let octave = document.graph.location_of(&ids[1]).unwrap();
    let PlacementAddress::StackIndex(p) = pitch.address else {
        panic!()
    };
    assert_eq!(
        octave.address,
        PlacementAddress::StackIndex(StackIndex(p.0 + 1)),
        "Reordering D before C4 separated C from its octave across the gap"
    );
    let reordered = layout(&app, surface);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(layout(&app, surface), before_reorder);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(layout(&app, surface), reordered);
}

#[test]
fn a_board_drag_can_drop_in_an_empty_wrapped_row_beyond_the_append_hint() {
    let (mut app, surface, ids, window) = fixture(vec![
        AtomValue::NoteName(NoteName::C),
        AtomValue::Octave(4),
        AtomValue::NoteName(NoteName::E),
        AtomValue::Octave(4),
    ]);
    let before = layout(&app, surface);
    let target = app
        .world()
        .resource::<VisibleBoardState>()
        .stack_display
        .authored_index(StackIndex(12));
    point(&mut app, window, Vec2::new(100., 100.), Some(0));
    press(&mut app);
    app.world_mut()
        .resource_mut::<BoardTilePressQueue>()
        .pending = Some(BoardTilePressed {
        node: ids[0].clone(),
        start_screen: Vec2::new(100., 100.),
    });
    tick(&mut app);
    point(&mut app, window, Vec2::new(100., 300.), Some(12));
    release(&mut app);
    let document = &app.world().resource::<MusaicProject>().document;
    assert_eq!(
        document.graph.location_of(&ids[0]).unwrap().address,
        PlacementAddress::StackIndex(target)
    );
    assert_eq!(
        document.graph.location_of(&ids[1]).unwrap().address,
        PlacementAddress::StackIndex(StackIndex(target.0 + 1))
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(layout(&app, surface), before);
}
