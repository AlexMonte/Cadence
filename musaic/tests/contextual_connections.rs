//! Contextual wiring preserves occupied endpoints while matching actual stream types.
use bevy::prelude::*;
use musaic::{
    MusaicProject,
    application::{
        command::{EditorCommand, EditorCommandBus, editing::TileEdit},
        editor::{
            EditorPlugin, PlacementTarget, SelectionState, connection::plan_contextual_connections,
        },
        pipeline::PlaybackPlugin,
    },
    domain::{
        board::BoardSlot,
        document::{
            self, AtomValue, ContainerKind, GraphTilePrototypeId, NoteName, PlacementAddress,
            StackIndex, TileSpawnKind,
            connection_policy::{
                ConnectionPolicyError, authorize_connection, endpoint_connections,
            },
        },
    },
    infrastructure::app::{AppState, MusaicSet, TransportMode},
};
use tessera::prelude::*;
fn id(name: &str) -> NodeId {
    NodeId::new(name)
}
fn socket(name: &str) -> InputEndpoint {
    InputEndpoint::Socket(InputPort::new(name))
}
fn output() -> OutputEndpoint {
    OutputEndpoint::Socket(OutputPort::new("out"))
}
fn member(group: &str, name: &str) -> InputEndpoint {
    InputEndpoint::GroupMember {
        group: PortGroupId::new(group),
        member: PortMemberId::new(name),
    }
}
fn branch(name: &str) -> OutputEndpoint {
    OutputEndpoint::GroupMember {
        group: PortGroupId::new("branches"),
        member: PortMemberId::new(name),
    }
}
fn off(program: &mut AuthoredTesseraProgram) {
    for binding in program.root_surface.bindings.values_mut() {
        for side in binding
            .inputs
            .values_mut()
            .chain(binding.outputs.values_mut())
        {
            *side = SpatialSide::Off;
        }
    }
}
fn apply_plan(
    before: &AuthoredTesseraProgram,
    mut current: AuthoredTesseraProgram,
    node: &str,
) -> (AuthoredTesseraProgram, Vec<String>) {
    let plan = plan_contextual_connections(before, &current, &[id(node)], None).unwrap();
    current.root_surface.bindings = plan.bindings;
    current.root_surface.explicit_relations = plan.explicit_relations;
    (current, plan.feedback)
}
fn number(b: &mut Board, x: i32, y: i32, name: &str) {
    b.at(x, y).named(name).scalar(Rational::one()).unwrap();
}
fn notes(b: &mut Board, x: i32, y: i32, name: &str) {
    b.at(x, y)
        .named(name)
        .sequence(SequenceStack::new().note("c").octave(4).build())
        .unwrap();
}
#[test]
fn notes_and_values_find_their_own_inputs_on_every_side_without_focus() {
    for (x, y, side) in [
        (1, 0, SpatialSide::East),
        (-1, 0, SpatialSide::West),
        (0, 1, SpatialSide::South),
        (0, -1, SpatialSide::North),
    ] {
        for scalar in [false, true] {
            let mut b = Board::new();
            b.at(0, 0)
                .named("gain")
                .transform(TransformKind::Gain)
                .unwrap();
            let mut before = b.clone().finish();
            off(&mut before);
            if scalar {
                number(&mut b, x, y, "source");
            } else {
                notes(&mut b, x, y, "source");
            }
            let mut current = b.finish();
            current.root_surface.bindings.insert(
                id("gain"),
                before.root_surface.bindings[&id("gain")].clone(),
            );
            let (after, feedback) = apply_plan(&before, current, "source");
            assert!(feedback.is_empty(), "{feedback:?}");
            let edges = endpoint_connections(&after);
            assert_eq!(edges.len(), 1);
            let edge = edges.first().unwrap();
            assert_eq!(edge.from, id("source"));
            assert_eq!(edge.to, id("gain"));
            assert_eq!(edge.input, socket(if scalar { "amount" } else { "main" }));
            assert_eq!(
                after.root_surface.bindings[&id("gain")].inputs[&edge.input],
                side
            );
        }
    }
}
#[test]
fn saturated_value_input_and_incompatible_outputs_keep_all_existing_bindings() {
    let mut b = Board::new();
    b.at(0, 0)
        .named("gain")
        .transform(TransformKind::Gain)
        .unwrap();
    number(&mut b, 0, -1, "first");
    let first = b.tile(&id("first")).unwrap();
    b.bind_output_side(&first, output(), SpatialSide::South)
        .unwrap();
    let before = b.clone().finish();
    number(&mut b, -1, 0, "extra");
    let (after, feedback) = apply_plan(&before, b.finish(), "extra");
    assert!(
        feedback.iter().any(|m| m.contains("already connected")),
        "{feedback:?}"
    );
    assert_eq!(endpoint_connections(&after), endpoint_connections(&before));
    for (node, binding) in &before.root_surface.bindings {
        assert_eq!(&after.root_surface.bindings[node], binding);
    }
    assert!(matches!(
        authorize_connection(&after, &id("extra"), &id("gain")),
        Err(ConnectionPolicyError::Occupied)
    ));
    let mut b = Board::new();
    b.at(0, 0).named("sink").output().unwrap();
    let before = b.clone().finish();
    number(&mut b, -1, 0, "number");
    let (after, feedback) = apply_plan(&before, b.finish(), "number");
    assert!(endpoint_connections(&after).is_empty());
    assert!(feedback.iter().any(|m| m.contains("compatible")));
    assert_eq!(
        after.root_surface.bindings[&id("sink")],
        before.root_surface.bindings[&id("sink")]
    );
}
#[test]
fn named_alternative_inputs_keep_member_order_and_the_connected_sibling() {
    let mut flow = FlowControlNode::new(FlowControlKind::Layer);
    flow.members.inputs.insert(
        PortGroupId::new("streams"),
        vec![PortMemberId::new("zeta"), PortMemberId::new("alpha")],
    );
    let mut b = Board::new();
    notes(&mut b, -1, 0, "first");
    b.at(0, 0)
        .named("layer")
        .flow_control_node(flow.clone())
        .unwrap();
    let layer = b.tile(&id("layer")).unwrap();
    b.bind_input_side(&layer, member("streams", "alpha"), SpatialSide::Off)
        .unwrap();
    let before = b.clone().finish();
    notes(&mut b, 0, -1, "second");
    let (after, feedback) = apply_plan(&before, b.finish(), "second");
    assert!(feedback.is_empty(), "{feedback:?}");
    assert!(endpoint_connections(&before).is_subset(&endpoint_connections(&after)));
    assert_eq!(endpoint_connections(&after).len(), 2);
    assert_eq!(
        after.root_surface.bindings[&id("layer")].inputs[&member("streams", "alpha")],
        SpatialSide::North
    );
    assert_eq!(
        after.root_surface.nodes[&id("layer")],
        RootSurfaceNodeKind::FlowControl(flow)
    );
}
#[test]
fn free_named_output_is_selected_without_repointing_an_occupied_branch() {
    let mut b = Board::new();
    b.at(0, 0)
        .named("split")
        .flow_control(FlowControlKind::Split)
        .unwrap();
    b.at(1, 0).named("first").output().unwrap();
    let split = b.tile(&id("split")).unwrap();
    b.bind_output_side(&split, branch("odd"), SpatialSide::Off)
        .unwrap();
    let before = b.clone().finish();
    b.at(0, -1).named("second").output().unwrap();
    let (after, feedback) = apply_plan(&before, b.finish(), "second");
    assert!(feedback.is_empty(), "{feedback:?}");
    assert_eq!(endpoint_connections(&after).len(), 2);
    assert!(endpoint_connections(&before).is_subset(&endpoint_connections(&after)));
    assert_eq!(
        after.root_surface.bindings[&id("split")].outputs[&branch("even")],
        SpatialSide::East
    );
    assert_eq!(
        after.root_surface.bindings[&id("split")].outputs[&branch("odd")],
        SpatialSide::North
    );
}
#[test]
fn shared_edge_cannot_steal_an_unrelated_connection_and_unbound_outputs_draw_no_wire() {
    let mut b = Board::new();
    notes(&mut b, 0, 0, "source");
    b.at(1, 0).named("out").output().unwrap();
    let mut before = b.finish();
    off(&mut before);
    before
        .root_surface
        .bindings
        .get_mut(&id("source"))
        .unwrap()
        .outputs
        .insert(output(), SpatialSide::East);
    assert!(document::connections_from_program(&before).is_empty());
    let mut b = Board::new();
    b.at(0, 0)
        .named("source")
        .footprint(TileFootprint::new(2, 2))
        .sequence(SequenceStack::new().note("c").build())
        .unwrap();
    b.at(2, 0)
        .named("out")
        .footprint(TileFootprint::new(2, 2))
        .output()
        .unwrap();
    let before = b.clone().finish();
    number(&mut b, 2, -1, "number");
    let (after, _) = apply_plan(&before, b.finish(), "number");
    assert!(endpoint_connections(&before).is_subset(&endpoint_connections(&after)));
}
fn insert_root(
    project: &mut MusaicProject,
    name: &str,
    x: i32,
    y: i32,
    tile: TileSpawnKind,
) -> NodeId {
    let node = id(name);
    let root = project.document.root_surface;
    project
        .document
        .graph
        .insert_tile_at_id(
            &mut project.document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(x, y)),
            node.clone(),
            tile,
        )
        .unwrap();
    node
}

fn insert_notes(project: &mut MusaicProject, name: &str, x: i32, y: i32) -> NodeId {
    let node = insert_root(
        project,
        name,
        x,
        y,
        TileSpawnKind::Container {
            kind: ContainerKind::Sequence,
        },
    );
    let surface = project.document.graph.container_surface(&node).unwrap();
    for (index, atom) in [AtomValue::NoteName(NoteName::C), AtomValue::Octave(4)]
        .into_iter()
        .enumerate()
    {
        project
            .document
            .graph
            .insert_tile(
                &mut project.document.surfaces,
                surface,
                PlacementAddress::StackIndex(StackIndex(index)),
                TileSpawnKind::Atom { atom },
            )
            .unwrap();
    }
    node
}

fn connect(project: &mut MusaicProject, from: &NodeId, to: &NodeId, side: SpatialSide) {
    let mut program = document::export_document_program(&project.document).unwrap();
    document::bind_tiles(&mut program, from, to, side).unwrap();
    project.document.replace_connections_from(&program);
}

fn app(project: MusaicProject) -> App {
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
    );
    app.insert_state(AppState::Editor);
    send(
        &mut app,
        EditorCommand::AdoptProject {
            project,
            path: None,
        },
    );
    app
}
fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    for _ in 0..3 {
        app.update();
    }
}
fn program(app: &App) -> AuthoredTesseraProgram {
    document::export_document_program(&app.world().resource::<MusaicProject>().document).unwrap()
}
#[test]
fn moving_a_value_reconnects_only_free_ports_and_undo_redo_save_keep_exact_bindings() {
    let mut project = MusaicProject::new_empty();
    insert_root(
        &mut project,
        "gain",
        0,
        0,
        TileSpawnKind::TrickInstance {
            prototype: GraphTilePrototypeId(3),
        },
    );
    insert_root(
        &mut project,
        "other",
        4,
        0,
        TileSpawnKind::TrickInstance {
            prototype: GraphTilePrototypeId(3),
        },
    );
    let value = insert_root(
        &mut project,
        "value",
        0,
        -1,
        TileSpawnKind::Atom {
            atom: AtomValue::Number(1),
        },
    );
    connect(&mut project, &value, &id("gain"), SpatialSide::South);
    let mut app = app(project);
    let before = program(&app);
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::Move {
            node: id("value"),
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(4, -1),
            },
        }),
    );
    let after = program(&app);
    let edges = endpoint_connections(&after);
    assert_eq!(edges.len(), 1);
    let edge = edges.first().unwrap();
    assert_eq!(edge.to, id("other"));
    assert_eq!(edge.input, socket("amount"));
    assert_eq!(
        after.root_surface.bindings[&id("gain")].inputs[&socket("amount")],
        SpatialSide::Off
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(program(&app), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(program(&app), after);
    let bytes =
        musaic::adapter::persistence::export_project_bytes(app.world().resource::<MusaicProject>())
            .unwrap();
    let saved = musaic::adapter::persistence::import_project_bytes(&bytes, None).unwrap();
    assert_eq!(
        document::export_document_program(&saved.document)
            .unwrap()
            .root_surface
            .bindings,
        after.root_surface.bindings
    );
}
#[test]
fn placing_beside_unfocused_tiles_records_connection_identity_in_history() {
    let mut project = MusaicProject::new_empty();
    insert_root(
        &mut project,
        "gain",
        0,
        0,
        TileSpawnKind::TrickInstance {
            prototype: GraphTilePrototypeId(3),
        },
    );
    let mut app = app(project);
    let before = program(&app);
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(0, -1),
            },
            tile: TileSpawnKind::Atom {
                atom: document::AtomValue::Number(2),
            },
        },
    );
    let placed = app
        .world()
        .resource::<SelectionState>()
        .nodes
        .iter()
        .next()
        .unwrap()
        .clone();
    let after = program(&app);
    assert!(
        endpoint_connections(&after)
            .iter()
            .any(|c| c.from == placed && c.input == socket("amount"))
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(program(&app), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(program(&app), after);
}

#[test]
fn moving_an_existing_shared_pattern_preserves_all_named_inputs_and_free_port_settings() {
    let mut b = Board::new();
    notes(&mut b, 0, 0, "source");
    b.at(1, 0)
        .named("mix")
        .flow_control(FlowControlKind::Mix)
        .unwrap();
    b.at(2, 0).named("out").output().unwrap();
    let before = b.finish();
    assert_eq!(endpoint_connections(&before).len(), 3);
    let mut moved = before.clone();
    for placement in moved.root_surface.placements.values_mut() {
        placement.slot.x += 10;
    }
    let plan =
        plan_contextual_connections(&before, &moved, &[id("source"), id("mix"), id("out")], None)
            .unwrap();
    moved.root_surface.bindings = plan.bindings;
    assert_eq!(endpoint_connections(&moved), endpoint_connections(&before));
    assert_eq!(moved.root_surface.bindings, before.root_surface.bindings);
}

#[test]
fn explicit_connection_undo_restores_the_previous_unused_port_orientations() {
    let mut project = MusaicProject::new_empty();
    insert_root(
        &mut project,
        "gain",
        0,
        0,
        TileSpawnKind::TrickInstance {
            prototype: GraphTilePrototypeId(3),
        },
    );
    let value = insert_root(
        &mut project,
        "value",
        -1,
        0,
        TileSpawnKind::Atom {
            atom: AtomValue::Number(1),
        },
    );
    let mut authored = document::export_document_program(&project.document).unwrap();
    authored
        .root_surface
        .bindings
        .get_mut(&value)
        .unwrap()
        .outputs
        .insert(output(), SpatialSide::South);
    project.document.replace_connections_from(&authored);
    let mut app = app(project);
    let before = program(&app);
    send(
        &mut app,
        EditorCommand::ConnectTiles {
            from: id("value"),
            to: id("gain"),
        },
    );
    let after = program(&app);
    assert_eq!(endpoint_connections(&after).len(), 1);
    assert_eq!(
        endpoint_connections(&after).first().unwrap().input,
        socket("amount")
    );
    assert_eq!(
        after.root_surface.bindings[&id("gain")].inputs[&socket("main")],
        SpatialSide::Off
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(program(&app), before);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(program(&app), after);
}

#[test]
fn custom_unused_socket_can_receive_a_value_without_resetting_its_signature() {
    let mut flow = FlowControlNode::new(FlowControlKind::Layer);
    flow.signature.input_sockets.push(InputSocketSpec {
        port: InputPort::new("pressure"),
        role: NodeInputRole::Aux,
        shape: StreamShape::ScalarPattern,
        connection: ConnectionRule::Optional,
        side: None,
        default: None,
    });
    let mut b = Board::new();
    b.at(0, 0)
        .named("custom")
        .flow_control_node(flow.clone())
        .unwrap();
    let before = b.clone().finish();
    number(&mut b, 0, -1, "value");
    let (after, feedback) = apply_plan(&before, b.finish(), "value");
    assert!(feedback.is_empty(), "{feedback:?}");
    assert_eq!(
        endpoint_connections(&after).first().unwrap().input,
        socket("pressure")
    );
    assert_eq!(
        after.root_surface.nodes[&id("custom")],
        RootSurfaceNodeKind::FlowControl(flow)
    );
}
#[test]
fn disconnecting_one_destination_keeps_an_existing_shared_output_alive() {
    let mut b = Board::new();
    b.at(0, 0)
        .named("source")
        .footprint(TileFootprint::new(2, 2))
        .sequence(SequenceStack::new().note("c").build())
        .unwrap();
    for (name, y) in [("first", 0), ("second", 1)] {
        let tile = b
            .at(2, y)
            .named(name)
            .transform(TransformKind::Gain)
            .unwrap();
        b.bind_input_side(&tile, socket("amount"), SpatialSide::Off)
            .unwrap();
    }
    let original = b.finish();
    let mut after = original.clone();
    assert_eq!(endpoint_connections(&after).len(), 2);
    document::unbind_connection(&mut after, &id("source"), &id("first")).unwrap();
    assert_eq!(endpoint_connections(&after).len(), 1);
    assert_eq!(
        endpoint_connections(&after).first().unwrap().to,
        id("second")
    );
    assert_eq!(
        after.root_surface.bindings[&id("source")].outputs[&output()],
        SpatialSide::East
    );
    // The same guarantee must hold when a user clicks the rendered cable, including Undo.
    let mut project = MusaicProject::new_empty();
    let source = insert_notes(&mut project, "source", 0, 0);
    for (name, y) in [("first", 0), ("second", 1)] {
        insert_root(
            &mut project,
            name,
            5,
            y,
            TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(3),
            },
        );
    }
    let mut authored = document::export_document_program(&project.document).unwrap();
    authored
        .root_surface
        .bindings
        .get_mut(&source)
        .unwrap()
        .outputs
        .insert(output(), SpatialSide::East);
    for destination in ["first", "second"] {
        authored
            .root_surface
            .explicit_relations
            .push(RootRelation::FlowsTo {
                from: StreamSource {
                    node: source.clone(),
                    endpoint: output(),
                },
                to: StreamTarget::TransformInput {
                    node: id(destination),
                    endpoint: socket("amount"),
                },
            });
    }
    project.document.replace_connections_from(&authored);
    let mut document = project.document;
    let before = document::export_document_program(&document).unwrap();
    let mut attention = musaic::application::editor::EditorAttention::new(document.root_surface);
    let result = musaic::application::editor::transaction::cycle_connection(
        &mut document,
        &attention,
        &id("source"),
        &id("first"),
    );
    assert!(result.is_accepted(), "{:?}", result.diagnostics);
    let remaining = endpoint_connections(&document::export_document_program(&document).unwrap());
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining.first().unwrap().to, id("second"));
    musaic::application::command::execute_inverse(
        &mut document,
        &mut attention,
        &mut SelectionState::default(),
        &result.undo.unwrap(),
    )
    .unwrap();
    assert_eq!(
        document::export_document_program(&document).unwrap(),
        before
    );
}
#[test]
fn free_endpoints_cannot_close_a_feedback_loop() {
    let mut b = Board::new();
    for (name, x, y) in [("a", 0, 0), ("b", 1, 0), ("c", 1, 1), ("d", 0, 1)] {
        b.at(x, y)
            .named(name)
            .transform(TransformKind::Gain)
            .unwrap();
    }
    let mut program = b.finish();
    off(&mut program);
    for (from, to) in [("a", "b"), ("b", "c"), ("c", "d")] {
        let edge = authorize_connection(&program, &id(from), &id(to)).unwrap();
        document::connection_policy::apply_edge(&mut program, &id(from), &id(to), &edge);
    }
    let before = program.clone();
    assert_eq!(
        authorize_connection(&program, &id("d"), &id("a")),
        Err(ConnectionPolicyError::Cycle)
    );
    assert_eq!(program, before);
}

#[test]
fn absent_binding_maps_are_free_hints_and_never_phantom_connections() {
    let mut b = Board::new();
    notes(&mut b, 0, 0, "source");
    b.at(1, 0).named("out").output().unwrap();
    let mut program = b.finish();
    program.root_surface.bindings.remove(&id("out"));
    assert!(endpoint_connections(&program).is_empty());
    assert!(
        TesseraCompiler::new()
            .resolve(&program)
            .unwrap()
            .relations
            .is_empty()
    );
    program.root_surface.bindings.remove(&id("source"));
    assert!(endpoint_connections(&program).is_empty());
    let edge = authorize_connection(&program, &id("source"), &id("out")).unwrap();
    document::connection_policy::apply_edge(&mut program, &id("source"), &id("out"), &edge);
    assert_eq!(endpoint_connections(&program).len(), 1);
    assert_eq!(
        TesseraCompiler::new()
            .resolve(&program)
            .unwrap()
            .relations
            .len(),
        1
    );
}

#[test]
fn moving_around_the_same_choice_preserves_its_exact_named_option() {
    let mut flow = FlowControlNode::new(FlowControlKind::Choice);
    flow.members.inputs.insert(
        PortGroupId::new("options"),
        vec![PortMemberId::new("first"), PortMemberId::new("second")],
    );
    let mut b = Board::new();
    notes(&mut b, -1, 0, "source");
    let choice = b.at(0, 0).named("choice").flow_control_node(flow).unwrap();
    b.bind_input_side(&choice, member("options", "first"), SpatialSide::Off)
        .unwrap();
    b.bind_input_side(&choice, socket("control"), SpatialSide::Off)
        .unwrap();
    let before = b.finish();
    assert_eq!(
        endpoint_connections(&before).first().unwrap().input,
        member("options", "second")
    );
    let mut moved = before.clone();
    moved
        .root_surface
        .placements
        .get_mut(&id("source"))
        .unwrap()
        .slot = BoardSlot::new(0, -1);
    let plan = plan_contextual_connections(&before, &moved, &[id("source")], None).unwrap();
    moved.root_surface.bindings = plan.bindings;
    assert_eq!(endpoint_connections(&moved), endpoint_connections(&before));
    assert_eq!(
        moved.root_surface.bindings[&id("choice")].inputs[&member("options", "second")],
        SpatialSide::North
    );
    assert_eq!(
        moved.root_surface.bindings[&id("choice")].inputs[&member("options", "first")],
        SpatialSide::Off
    );
}

#[test]
fn contextual_speed_value_changes_real_engine_onsets_and_undo_restores_them() {
    use cadence::prelude::{
        BuiltInSynthSource, CadenceCompiler, Intent, PreparedScore, Span, Time,
    };
    let mut project = MusaicProject::new_empty();
    let notes = insert_notes(&mut project, "notes", 0, 0);
    let fast = insert_root(
        &mut project,
        "fast",
        5,
        0,
        TileSpawnKind::TrickInstance {
            prototype: GraphTilePrototypeId(1),
        },
    );
    let output_node = insert_root(
        &mut project,
        "out",
        6,
        0,
        TileSpawnKind::Output {
            name: String::new(),
        },
    );
    connect(&mut project, &notes, &fast, SpatialSide::East);
    connect(&mut project, &fast, &output_node, SpatialSide::East);
    let mut app = app(project);
    let onsets = |app: &App| {
        let ir = musaic::application::compile::compile_project_ir(
            app.world().resource::<MusaicProject>(),
        )
        .unwrap();
        let (scores, diagnostics) =
            musaic::application::pipeline::lowering::lower_tessera_ir_with_sounds(
                &ir,
                &std::collections::BTreeMap::from([(
                    id("out"),
                    Intent::synth(BuiltInSynthSource::Sine),
                )]),
            );
        assert!(diagnostics.is_empty());
        let prepared = PreparedScore::new(scores[&id("out")].clone()).unwrap();
        CadenceCompiler::new()
            .preview(&prepared, &Span::new(Time::ZERO, Time::ONE).unwrap())
            .unwrap()
            .starts()
            .map(|event| event.projected().whole().start())
            .collect::<Vec<_>>()
    };
    let original = onsets(&app);
    assert_eq!(original, vec![Time::ZERO, Time::new(1, 2)]);
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(5, -1),
            },
            tile: TileSpawnKind::Atom {
                atom: document::AtomValue::Number(3),
            },
        },
    );
    assert_eq!(
        onsets(&app),
        vec![Time::ZERO, Time::new(1, 3), Time::new(2, 3)]
    );
    send(&mut app, EditorCommand::Undo);
    assert_eq!(onsets(&app), original);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(onsets(&app).len(), 3);
}

#[test]
fn library_wires_route_around_a_corner_and_survive_undo_and_save() {
    let mut project = MusaicProject::new_empty();
    insert_notes(&mut project, "notes", 0, 0);
    insert_root(
        &mut project,
        "fast",
        5,
        2,
        TileSpawnKind::TrickInstance {
            prototype: GraphTilePrototypeId(1),
        },
    );
    insert_root(
        &mut project,
        "out",
        6,
        2,
        TileSpawnKind::Output {
            name: String::new(),
        },
    );
    let mut app = app(project);
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    let wire = musaic::application::editor::panels::context_panel::labeled_tile_drawer_items()
        .into_iter()
        .find(|item| item.label == "Wire")
        .expect("wire is available in the library")
        .spawn;
    for (x, y) in [(5, 0), (5, 1)] {
        send(
            &mut app,
            EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root,
                    slot: BoardSlot::new(x, y),
                },
                tile: wire.clone(),
            },
        );
    }
    let after = program(&app);
    let edges = endpoint_connections(&after);
    assert_eq!(
        edges.len(),
        4,
        "notes -> bend -> wire -> fast -> output: {edges:?}"
    );
    let compiler = TesseraCompiler::new();
    let ir = compiler.compile_authored(&after).unwrap().ir;
    let span = CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one()));
    let expected = ir.outputs[0].root.query(span).events;
    assert_eq!(expected.len(), 2);
    assert_eq!(expected[1].span.start.0, Rational::new(1, 2));
    send(&mut app, EditorCommand::Undo);
    assert_ne!(endpoint_connections(&program(&app)), edges);
    send(&mut app, EditorCommand::Redo);
    assert_eq!(program(&app), after);
    let saved = musaic::adapter::persistence::import_project_bytes(
        &musaic::adapter::persistence::export_project_bytes(
            app.world().resource::<MusaicProject>(),
        )
        .unwrap(),
        None,
    )
    .unwrap();
    let restored = document::export_document_program(&saved.document).unwrap();
    assert_eq!(endpoint_connections(&restored), edges);
    assert_eq!(
        compiler.compile_authored(&restored).unwrap().ir.outputs[0]
            .root
            .query(span)
            .events,
        expected
    );
}
