use musaic::{
    application::{
        compile::compile_project_ir, pipeline::lowering::lower_project_ir, session::MusaicProject,
    },
    domain::{
        board::BoardSlot,
        document::{
            self, AtomValue, ContainerKind, GraphTilePrototypeId, NoteName, PlacementAddress,
            StackIndex, TileSpawnKind,
        },
        instrument::{InstrumentDefinition, InstrumentSource, Waveform},
    },
};
use tessera::prelude::*;
fn add(p: &mut MusaicProject, x: i32, y: i32, tile: TileSpawnKind) -> NodeId {
    p.document
        .graph
        .insert_tile(
            &mut p.document.surfaces,
            p.document.root_surface,
            PlacementAddress::BoardSlot(BoardSlot::new(x, y)),
            tile,
        )
        .unwrap()
}
fn pattern(p: &mut MusaicProject, x: i32, note: NoteName) -> NodeId {
    let n = add(
        p,
        x,
        0,
        TileSpawnKind::Container {
            kind: ContainerKind::Sequence,
        },
    );
    let s = p.document.graph.container_surface(&n).unwrap();
    p.document
        .graph
        .insert_tile(
            &mut p.document.surfaces,
            s,
            PlacementAddress::StackIndex(StackIndex(0)),
            TileSpawnKind::Atom {
                atom: AtomValue::NoteName(note),
            },
        )
        .unwrap();
    n
}
fn processor(p: &mut MusaicProject, x: i32, y: i32, id: u64) -> NodeId {
    add(
        p,
        x,
        y,
        TileSpawnKind::TrickInstance {
            prototype: GraphTilePrototypeId(id),
        },
    )
}
fn sound(p: &mut MusaicProject, x: i32, y: i32, definition: InstrumentDefinition) -> NodeId {
    add(p, x, y, TileSpawnKind::sound(definition))
}
fn connect(p: &mut MusaicProject, from: &NodeId, to: &NodeId, port: &str) {
    let endpoint = InputEndpoint::Socket(InputPort::new(port));
    let target = if matches!(
        p.document.graph.node(to).map(|n| &n.kind),
        Some(document::DocumentNodeKind::Output(_))
    ) {
        StreamTarget::OutputInput {
            node: to.clone(),
            endpoint: InputEndpoint::GroupMember {
                group: PortGroupId::new("inputs"),
                member: PortMemberId::new(port),
            },
        }
    } else {
        StreamTarget::TransformInput {
            node: to.clone(),
            endpoint,
        }
    };
    p.document
        .connections
        .explicit_relations
        .push(RootRelation::FlowsTo {
            from: StreamSource {
                node: from.clone(),
                endpoint: OutputEndpoint::Socket(OutputPort::new("out")),
            },
            to: target,
        });
}
fn sync(p: &mut MusaicProject) {
    let mut a = document::export_document_program(&p.document).unwrap();
    for binding in a.root_surface.bindings.values_mut() {
        for side in binding
            .inputs
            .values_mut()
            .chain(binding.outputs.values_mut())
        {
            *side = SpatialSide::Off;
        }
    }
    p.document.replace_connections_from(&a);
}
fn events(p: &MusaicProject) -> Vec<PatternEvent> {
    compile_project_ir(p).unwrap().outputs[0]
        .root
        .query(CycleSpan::new(
            CycleTime(Rational::zero()),
            CycleDuration(Rational::one()),
        ))
        .events
}
#[test]
fn tricks_are_composable_pattern_functions_and_outputs_only_name_lanes() {
    let mut p = MusaicProject::new_empty();
    let c = pattern(&mut p, 0, NoteName::C);
    let d = pattern(&mut p, 8, NoteName::D);
    let transpose = processor(&mut p, 0, 4, 5);
    let amount = add(
        &mut p,
        4,
        4,
        TileSpawnKind::Atom {
            atom: AtomValue::Number(12),
        },
    );
    connect(&mut p, &c, &transpose, "main");
    connect(&mut p, &amount, &transpose, "amount");
    sync(&mut p);
    let trick = musaic::application::tricks::define(
        &mut p.document,
        transpose,
        "Octave up".into(),
        Some(c),
    )
    .unwrap();
    let instance = processor(&mut p, 8, 4, trick);
    connect(&mut p, &d, &instance, "main");
    sync(&mut p);
    let nested = musaic::application::tricks::define(
        &mut p.document,
        instance.clone(),
        "Raised melody".into(),
        None,
    )
    .unwrap();
    let call = processor(&mut p, 12, 4, nested);
    let instrument = sound(
        &mut p,
        16,
        4,
        InstrumentDefinition::new(InstrumentSource::Synth(Waveform::Triangle)),
    );
    let output = add(
        &mut p,
        20,
        4,
        TileSpawnKind::Output {
            name: String::new(),
        },
    );
    connect(&mut p, &call, &instrument, "main");
    connect(&mut p, &instrument, &output, "main");
    sync(&mut p);
    let before = events(&p);
    assert_eq!(before.len(), 1);
    assert!(
        matches!(&before[0].value,EventValue::Note{value,..} if value.eq_ignore_ascii_case("d"))
    );
    assert!(
        before[0]
            .fields
            .contains(&EventField::Transpose(FieldValue::rational(
                Rational::from_integer(12)
            )))
    );
    assert_eq!(
        before[0].source.as_ref().unwrap().instrument.as_ref(),
        Some(&instrument)
    );
    assert_eq!(
        musaic::application::outputs::timeline_name(&p, &output),
        "Triangle"
    );
    p.document.graph.rename_output(&output, "Lead").unwrap();
    assert_eq!(
        musaic::application::outputs::timeline_name(&p, &output),
        "Lead"
    );
    assert_eq!(before, events(&p));
    let ir = compile_project_ir(&p).unwrap();
    let (_, diagnostics, _) = lower_project_ir(&p, &ir);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let bytes = musaic::adapter::persistence::export_project_bytes(&p).unwrap();
    let loaded = musaic::adapter::persistence::import_project_bytes(&bytes, None).unwrap();
    assert_eq!(events(&loaded), before);
    assert_eq!(loaded.document.tricks, p.document.tricks);
}
#[test]
fn channel_count_is_enforced() {
    let mut p = MusaicProject::new_empty();
    p.document.channels = 1;
    add(
        &mut p,
        0,
        0,
        TileSpawnKind::Output {
            name: String::new(),
        },
    );
    add(
        &mut p,
        4,
        0,
        TileSpawnKind::Output {
            name: String::new(),
        },
    );
    assert!(musaic::application::outputs::validate(&p.document).is_err());
    assert!(musaic::adapter::persistence::export_project_bytes(&p).is_err());
}
#[path = "support/acceptance.rs"]
mod support;
#[test]
fn editor_placement_preserves_trick_identity_and_undo_and_channels() {
    use musaic::application::{
        command::{EditorCommand, PlacementTarget, editing::TileEdit},
        editor::{FocusTarget, SelectionMode},
    };
    let mut p = MusaicProject::new_empty();
    let source = pattern(&mut p, 0, NoteName::C);
    sync(&mut p);
    let root = p.document.root_surface;
    let mut app = support::editor(p);
    support::send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::DefineTrick {
            node: source.clone(),
            name: "Melody".into(),
            input: None,
        }),
    );
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .tricks
            .len(),
        1
    );
    support::send(&mut app, EditorCommand::Undo);
    assert!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .tricks
            .is_empty()
    );
    support::send(&mut app, EditorCommand::Redo);
    support::send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::DefineTrick {
            node: source,
            name: "Second melody".into(),
            input: None,
        }),
    );
    let id = *app
        .world()
        .resource::<MusaicProject>()
        .document
        .tricks
        .keys()
        .next_back()
        .unwrap();
    assert_eq!(id, 1001);
    support::send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(8, 0),
            },
            tile: TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(id),
            },
        },
    );
    let p = app.world().resource::<MusaicProject>();
    let call = p
        .document
        .graph
        .node_at_board_slot(root, BoardSlot::new(8, 0))
        .unwrap();
    assert!(
        matches!(&p.document.graph.node(&call).unwrap().kind,document::DocumentNodeKind::TrickInstance(t) if t.prototype.0==1001)
    );
    support::send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::RenameTrick {
            id,
            name: "Reusable melody".into(),
        }),
    );
    assert_eq!(
        app.world().resource::<MusaicProject>().document.tricks[&id].name,
        "Reusable melody"
    );
    assert!(
        app.world()
            .resource::<musaic::application::editor::SelectionState>()
            .nodes
            .contains(&call)
    );
    support::send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::SetChannels { channels: 1 }),
    );
    support::send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(12, 0),
            },
            tile: TileSpawnKind::Output {
                name: String::new(),
            },
        },
    );
    let output = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .node_at_board_slot(root, BoardSlot::new(12, 0))
        .unwrap();
    support::send(
        &mut app,
        EditorCommand::EditTiles(TileEdit::RenameOutput {
            node: output.clone(),
            name: "Lead".into(),
        }),
    );
    assert_eq!(
        musaic::application::outputs::timeline_name(app.world().resource(), &output),
        "Lead"
    );
    support::send(&mut app, EditorCommand::Undo);
    assert_eq!(
        musaic::application::outputs::timeline_name(app.world().resource(), &output),
        "Output"
    );
    support::send(&mut app, EditorCommand::Redo);
    support::send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::BoardSlot {
                surface: root,
                slot: BoardSlot::new(16, 0),
            },
            tile: TileSpawnKind::Output {
                name: "Extra".into(),
            },
        },
    );
    assert!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .node_at_board_slot(root, BoardSlot::new(16, 0))
            .is_none()
    );
    support::send(
        &mut app,
        EditorCommand::SelectNode {
            node: output.clone(),
            mode: SelectionMode::Replace,
        },
    );
    support::send(
        &mut app,
        EditorCommand::Focus {
            target: FocusTarget::Tile {
                node: output.clone(),
            },
        },
    );
    support::send(&mut app, EditorCommand::EditTiles(TileEdit::Duplicate));
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .nodes()
            .filter(|n| matches!(n.kind, document::DocumentNodeKind::Output(_)))
            .count(),
        1
    );
    support::send(
        &mut app,
        EditorCommand::SetSound {
            sound: output.clone(),
            definition: Default::default(),
        },
    );
    assert!(
        app.world()
            .resource::<MusaicProject>()
            .sound_definition(&output)
            .is_none()
    );
}
#[test]
fn recursion_and_deleted_trick_sources_are_rejected() {
    let mut p = MusaicProject::new_empty();
    let c = pattern(&mut p, 0, NoteName::C);
    sync(&mut p);
    let id = musaic::application::tricks::define(&mut p.document, c.clone(), "Melody".into(), None)
        .unwrap();
    let call = processor(&mut p, 8, 0, id);
    p.document.tricks.get_mut(&id).unwrap().source = call;
    assert!(musaic::application::tricks::validate(&p.document).is_err());
    p.document.tricks.get_mut(&id).unwrap().source = c.clone();
    p.document
        .graph
        .delete_subtree(&c, &mut p.document.surfaces)
        .unwrap();
    assert!(musaic::application::tricks::validate(&p.document).is_err());
}

#[test]
fn numeric_tricks_feed_control_inputs_and_names_follow_sounds_inside_tricks() {
    let mut p = MusaicProject::new_empty();
    let c = pattern(&mut p, 0, NoteName::C);
    let value = add(
        &mut p,
        8,
        0,
        TileSpawnKind::Atom {
            atom: AtomValue::Number(2),
        },
    );
    sync(&mut p);
    let id =
        musaic::application::tricks::define(&mut p.document, value, "Twice".into(), None).unwrap();
    let variable = processor(&mut p, 0, 4, id);
    let fast = processor(&mut p, 4, 4, 1);
    connect(&mut p, &c, &fast, "main");
    connect(&mut p, &variable, &fast, "factor");
    let instrument = sound(
        &mut p,
        8,
        4,
        InstrumentDefinition::new(InstrumentSource::Synth(Waveform::Square)),
    );
    connect(&mut p, &fast, &instrument, "main");
    sync(&mut p);
    let sound = musaic::application::tricks::define(
        &mut p.document,
        instrument,
        "Fast square".into(),
        None,
    )
    .unwrap();
    let call = processor(&mut p, 12, 4, sound);
    let output = add(
        &mut p,
        16,
        4,
        TileSpawnKind::Output {
            name: String::new(),
        },
    );
    connect(&mut p, &call, &output, "main");
    sync(&mut p);
    assert_eq!(events(&p).len(), 2);
    assert_eq!(
        musaic::application::outputs::timeline_name(&p, &output),
        "Square"
    );
}
