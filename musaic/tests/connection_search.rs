//! Connection search plans commit atomically through ordinary editor history.
use bevy::prelude::*;
use musaic::{
    MusaicProject,
    application::{
        command::{
            EditorCommand, EditorCommandBus,
            connection::{self, ConnectionPlan, ConnectionReceipt},
            editing::TileEdit,
        },
        editor::EditorPlugin,
        history::CommandHistory,
        pipeline::PlaybackPlugin,
    },
    domain::{
        board::BoardSlot,
        document::{
            self, AtomValue, ContainerKind, GraphTilePrototypeId, NoteName, PlacementAddress,
            StackIndex, TileSpawnKind, connection_policy::endpoint_connections,
        },
    },
    infrastructure::app::{AppState, TransportMode},
};
use tessera::prelude::*;
fn id(s: &str) -> NodeId {
    NodeId::new(s)
}
fn fixture() -> MusaicProject {
    let mut project = MusaicProject::new_empty();
    let root = project.document.root_surface;
    for (node, slot, tile) in [
        (
            "gain",
            BoardSlot::new(0, 0),
            TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(3),
            },
        ),
        (
            "value",
            BoardSlot::new(-1, 0),
            TileSpawnKind::Atom {
                atom: AtomValue::Ratio(Rational::one()),
            },
        ),
        (
            "notes",
            BoardSlot::new(-1, -1),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        ),
        (
            "far",
            BoardSlot::new(-8, 0),
            TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(3),
            },
        ),
    ] {
        project
            .document
            .graph
            .insert_tile_at_id(
                &mut project.document.surfaces,
                root,
                PlacementAddress::BoardSlot(slot),
                id(node),
                tile,
            )
            .unwrap();
    }
    let notes_surface = project
        .document
        .graph
        .container_surface(&id("notes"))
        .unwrap();
    project
        .document
        .graph
        .insert_tile(
            &mut project.document.surfaces,
            notes_surface,
            PlacementAddress::StackIndex(StackIndex(0)),
            TileSpawnKind::Atom {
                atom: AtomValue::NoteName(NoteName::C),
            },
        )
        .unwrap();
    let mut value = NodeSpatialBindings::default();
    value.outputs.insert(
        OutputEndpoint::Socket(OutputPort::new("out")),
        SpatialSide::South,
    );
    project
        .document
        .connections
        .bindings
        .insert(id("value"), value);
    project
}
fn app() -> App {
    let mut app = App::new();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins(MinimalPlugins)
        .add_plugins((EditorPlugin, PlaybackPlugin));
    app.insert_state(AppState::Editor);
    send(
        &mut app,
        EditorCommand::AdoptProject {
            project: fixture(),
            path: None,
        },
    );
    app
}
fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    app.update();
}
fn planned(app: &App) -> ConnectionPlan {
    connection::plan(
        &app.world().resource::<MusaicProject>().document,
        &id("value"),
        &id("gain"),
    )
    .unwrap()
}
fn program(app: &App) -> AuthoredTesseraProgram {
    document::export_document_program(&app.world().resource::<MusaicProject>().document).unwrap()
}
fn connect(app: &mut App, request: u64, plan: ConnectionPlan) -> Result<(), String> {
    send(
        app,
        EditorCommand::EditTiles(TileEdit::Connect { request, plan }),
    );
    let receipts: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<ConnectionReceipt>>()
        .drain()
        .collect();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].request, request);
    receipts.into_iter().next().unwrap().result
}
#[test]
fn choices_include_distant_compatible_tiles_and_exclude_incompatible_or_occupied_ports() {
    let mut app = app();
    let choices = connection::destinations(
        &app.world().resource::<MusaicProject>().document,
        &id("value"),
    );
    assert_eq!(choices.len(), 2);
    assert!(choices.iter().any(|p| p.to == id("far")));
    let chosen = choices.iter().find(|p| p.to == id("gain")).unwrap();
    assert_eq!(
        chosen.edge.input,
        InputEndpoint::Socket(InputPort::new("amount"))
    );
    connect(&mut app, 1, chosen.clone()).unwrap();
    assert!(
        connection::destinations(
            &app.world().resource::<MusaicProject>().document,
            &id("value")
        )
        .is_empty()
    );
}
#[test]
fn wide_pattern_at_negative_coordinates_uses_its_actual_edge() {
    let mut project = MusaicProject::new_empty();
    let root = project.document.root_surface;
    project
        .document
        .graph
        .insert_tile_at_id(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(-3, -2)),
            id("wide"),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        )
        .unwrap();
    project
        .document
        .graph
        .insert_tile_at_id(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(2, -2)),
            id("gain"),
            TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(3),
            },
        )
        .unwrap();
    let mut wide = NodeSpatialBindings::default();
    wide.outputs.insert(
        OutputEndpoint::Socket(OutputPort::new("out")),
        SpatialSide::North,
    );
    project
        .document
        .connections
        .bindings
        .insert(id("wide"), wide);
    let choices = connection::destinations(&project.document, &id("wide"));
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].edge.side, SpatialSide::East);
    assert_eq!(
        choices[0].edge.input,
        InputEndpoint::Socket(InputPort::new("main"))
    );
}
#[test]
fn accepted_plan_has_one_history_entry_and_exact_save_undo_redo() {
    let mut app = app();
    let before = program(&app);
    let plan = planned(&app);
    connect(&mut app, 7, plan.clone()).unwrap();
    let after = program(&app);
    assert_eq!(
        endpoint_connections(&after).len(),
        endpoint_connections(&before).len() + 1
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
    let bytes =
        musaic::adapter::persistence::export_project_bytes(app.world().resource::<MusaicProject>())
            .unwrap();
    let reopened = musaic::adapter::persistence::import_project_bytes(&bytes, None).unwrap();
    assert_eq!(
        document::export_document_program(&reopened.document).unwrap(),
        after
    );
    assert!(connect(&mut app, 8, plan).is_err());
    assert_eq!(program(&app), after);
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(program(&app), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(program(&app), after);
}
#[test]
fn moved_or_removed_destination_rejects_without_changing_music_or_history() {
    for remove in [false, true] {
        let mut app = app();
        let plan = planned(&app);
        if remove {
            app.world_mut()
                .resource_mut::<musaic::application::editor::SelectionState>()
                .nodes
                .insert(id("gain"));
            send(&mut app, EditorCommand::DeleteSelection);
        } else {
            let surface = app
                .world()
                .resource::<MusaicProject>()
                .document
                .root_surface;
            send(
                &mut app,
                EditorCommand::EditTiles(TileEdit::Move {
                    node: id("gain"),
                    target: musaic::application::command::PlacementTarget::BoardSlot {
                        surface,
                        slot: musaic::domain::board::BoardSlot::new(5, 5),
                    },
                }),
            );
        }
        let before = program(&app);
        let history = app.world().resource::<CommandHistory>().undo_len();
        assert!(connect(&mut app, 11, plan).is_err());
        assert_eq!(program(&app), before);
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), history);
    }
}
#[test]
fn changed_endpoint_choice_is_rejected_even_when_the_tiles_are_still_compatible() {
    let mut app = app();
    let mut plan = planned(&app);
    plan.edge.input = InputEndpoint::Socket(InputPort::new("main"));
    let before = program(&app);
    assert!(
        connect(&mut app, 15, plan)
            .unwrap_err()
            .contains("route changed")
    );
    assert_eq!(program(&app), before);
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
}

#[test]
fn distant_cable_preserves_music_through_movement_save_disconnect_and_undo() {
    use musaic::application::{
        command::PlacementTarget, compile::compile_project_ir, editor::SelectionState,
    };
    use musaic::domain::{
        board::BoardSlot,
        document::{DocumentNodeKind, PlacementAddress},
    };
    let mut app = app();
    send(
        &mut app,
        EditorCommand::AdoptProject {
            project: MusaicProject::demo(),
            path: None,
        },
    );
    let project = app.world().resource::<MusaicProject>();
    let expected = compile_project_ir(project).unwrap().flat_outputs();
    #[cfg(not(target_arch = "wasm32"))]
    let expected_audio = musaic::application::audio_export::render_project_wav(project, 1).unwrap();
    let surface = project.document.root_surface;
    let source = project
        .document
        .graph
        .nodes()
        .find(|n| {
            matches!(n.kind, DocumentNodeKind::Container(_))
                && project
                    .document
                    .graph
                    .location_of(&n.id)
                    .is_some_and(|p| p.surface == surface)
        })
        .unwrap()
        .id
        .clone();
    let target = endpoint_connections(&program(&app))
        .into_iter()
        .find(|e| e.from == source)
        .unwrap()
        .to;
    let move_to = |slot| {
        EditorCommand::EditTiles(TileEdit::Move {
            node: source.clone(),
            target: PlacementTarget::BoardSlot { surface, slot },
        })
    };
    send(&mut app, move_to(BoardSlot::new(-8, 2)));
    assert!(
        !endpoint_connections(&program(&app))
            .iter()
            .any(|e| e.from == source)
    );
    let before = program(&app);
    let plan = connection::plan(
        &app.world().resource::<MusaicProject>().document,
        &source,
        &target,
    )
    .unwrap();
    assert!(plan.edge.explicit);
    let history = app.world().resource::<CommandHistory>().undo_len();
    connect(&mut app, 91, plan).unwrap();
    assert_eq!(
        app.world().resource::<CommandHistory>().undo_len(),
        history + 1
    );
    let connected = program(&app);
    assert_eq!(
        compile_project_ir(app.world().resource::<MusaicProject>())
            .unwrap()
            .flat_outputs(),
        expected
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(program(&app), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(program(&app), connected);
    send(&mut app, move_to(BoardSlot::new(-10, -4)));
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .location_of(&source)
            .unwrap()
            .address,
        PlacementAddress::BoardSlot(BoardSlot::new(-10, -4))
    );
    assert_eq!(
        endpoint_connections(&program(&app)),
        endpoint_connections(&connected)
    );
    assert_eq!(
        compile_project_ir(app.world().resource::<MusaicProject>())
            .unwrap()
            .flat_outputs(),
        expected
    );
    let moved = program(&app);
    let bytes =
        musaic::adapter::persistence::export_project_bytes(app.world().resource::<MusaicProject>())
            .unwrap();
    let reopened = musaic::adapter::persistence::import_project_bytes(&bytes, None).unwrap();
    assert_eq!(
        compile_project_ir(&reopened).unwrap().flat_outputs(),
        expected
    );
    assert_eq!(
        document::export_document_program(&reopened.document).unwrap(),
        moved
    );
    #[cfg(not(target_arch = "wasm32"))]
    assert!(
        musaic::application::audio_export::render_project_wav(&reopened, 1).unwrap()
            == expected_audio,
        "moving and reopening a cable preserves exact rendered audio"
    );
    app.world_mut().resource_mut::<SelectionState>().nodes = [source.clone()].into();
    send(&mut app, EditorCommand::EditTiles(TileEdit::Duplicate));
    assert_eq!(
        endpoint_connections(&program(&app)),
        endpoint_connections(&moved),
        "duplicating a source must not steal or duplicate its external cable"
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(program(&app), moved);
    send(
        &mut app,
        EditorCommand::CycleConnection {
            from: source.clone(),
            to: target,
        },
    );
    assert!(
        !endpoint_connections(&program(&app))
            .iter()
            .any(|e| e.from == source)
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(program(&app), moved);
    app.world_mut().resource_mut::<SelectionState>().nodes = [source.clone()].into();
    send(&mut app, EditorCommand::DeleteSelection);
    assert!(
        !endpoint_connections(&program(&app))
            .iter()
            .any(|e| e.from == source)
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(program(&app), moved);
}
