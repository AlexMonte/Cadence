use musaic::{
    application::{
        command::{
            EditorCommand as C,
            editing::{self, TileEdit},
        },
        editor::{EditorAttention, SelectionMode, SelectionState},
        session::MusaicProject,
    },
    domain::{
        board::BoardSlot,
        document::{
            self, AtomValue, ContainerKind, DocumentNodeKind, GraphTilePrototypeId, NoteName,
            PlacementAddress, StackIndex, TileSpawnKind,
        },
        instrument::{InstrumentDefinition, InstrumentSource, Waveform},
    },
};
use std::collections::BTreeSet;
use tessera::prelude::*;
#[path = "support/acceptance.rs"]
mod support;
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
fn pattern(p: &mut MusaicProject, x: i32, note: NoteName) -> NodeId {
    let node = add(
        p,
        x,
        0,
        TileSpawnKind::Container {
            kind: ContainerKind::Sequence,
        },
    );
    let surface = p.document.graph.container_surface(&node).unwrap();
    p.document
        .graph
        .insert_tile(
            &mut p.document.surfaces,
            surface,
            PlacementAddress::StackIndex(StackIndex(0)),
            TileSpawnKind::Atom {
                atom: AtomValue::NoteName(note),
            },
        )
        .unwrap();
    node
}
fn connect(p: &mut MusaicProject, from: &NodeId, to: &NodeId, port: &str) {
    let target = if matches!(
        p.document.graph.node(to).unwrap().kind,
        DocumentNodeKind::Output(_)
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
            endpoint: InputEndpoint::Socket(InputPort::new(port)),
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
    p.document.validate().unwrap();
}
struct Fixture {
    project: MusaicProject,
    instrument: NodeId,
    output: NodeId,
    amount: NodeId,
}
fn fixture() -> Fixture {
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
    let function =
        musaic::application::tricks::define(&mut p.document, transpose, "Octave".into(), Some(c))
            .unwrap();
    let inner = processor(&mut p, 8, 4, function);
    connect(&mut p, &d, &inner, "main");
    sync(&mut p);
    let melody =
        musaic::application::tricks::define(&mut p.document, inner, "Melody".into(), None).unwrap();
    let call = processor(&mut p, 12, 4, melody);
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
            name: "Original".into(),
        },
    );
    connect(&mut p, &call, &instrument, "main");
    connect(&mut p, &instrument, &output, "main");
    sync(&mut p);
    Fixture {
        project: p,
        instrument,
        output,
        amount,
    }
}
fn selected(p: &MusaicProject, node: &NodeId) -> (SelectionState, EditorAttention) {
    let mut selection = SelectionState::default();
    selection.nodes.insert(node.clone());
    let mut attention = EditorAttention::new(p.document.root_surface);
    attention.focus = musaic::application::editor::FocusTarget::Tile { node: node.clone() };
    (selection, attention)
}
fn events(p: &MusaicProject, output: &NodeId) -> Vec<PatternEvent> {
    let ir = musaic::application::compile::compile_project_ir(p).unwrap();
    let mut events = ir
        .outputs
        .iter()
        .find(|o| &o.id == output)
        .unwrap()
        .root
        .query(CycleSpan::new(
            CycleTime(Rational::zero()),
            CycleDuration(Rational::one()),
        ))
        .events;
    for event in &mut events {
        event.source = None;
    }
    events
}
#[test]
fn transitive_function_and_sound_copy_can_change_without_touching_original_and_survive_save() {
    let Fixture {
        mut project,
        instrument,
        output,
        amount,
    } = fixture();
    let baseline = project.clone();
    let expected = events(&project, &output);
    let originals: BTreeSet<_> = project
        .document
        .graph
        .nodes()
        .map(|n| n.id.clone())
        .collect();
    let (mut selection, mut attention) = selected(&project, &instrument);
    let outcome = editing::execute(
        &mut project,
        &mut selection,
        &mut attention,
        &mut Default::default(),
        &TileEdit::IndependentVariation,
    )
    .unwrap();
    assert!(outcome.record.is_some());
    assert_eq!(project.document.tricks.len(), 4);
    for node in baseline.document.graph.nodes() {
        assert_eq!(project.document.graph.node(&node.id), Some(node));
        assert_eq!(
            project.document.graph.location_of(&node.id),
            baseline.document.graph.location_of(&node.id)
        );
    }
    for (_, definition) in project
        .document
        .tricks
        .iter()
        .filter(|(id, _)| **id >= 1002)
    {
        assert!(!originals.contains(&definition.source));
        assert!(
            definition
                .input
                .as_ref()
                .is_none_or(|id| !originals.contains(id))
        );
    }
    assert!(project.document.graph.nodes().filter(|n| !originals.contains(&n.id)).all(|n| !matches!(&n.kind, DocumentNodeKind::TrickInstance(t) if t.prototype.0 == 1000 || t.prototype.0 == 1001)));
    let copied_instrument = project
        .document
        .graph
        .nodes()
        .find(|node| {
            !originals.contains(&node.id) && matches!(node.kind, DocumentNodeKind::Sound(_))
        })
        .unwrap()
        .id
        .clone();
    assert_eq!(
        project.sound_definition(&copied_instrument),
        project.sound_definition(&instrument)
    );
    let copied_amount = project
        .document
        .graph
        .nodes()
        .find(|n| {
            !originals.contains(&n.id)
                && matches!(&n.kind, DocumentNodeKind::Atom(a) if a.atom == AtomValue::Number(12))
        })
        .unwrap()
        .id
        .clone();
    let copy_output = add(
        &mut project,
        24,
        12,
        TileSpawnKind::Output {
            name: "Variation".into(),
        },
    );
    connect(&mut project, &copied_instrument, &copy_output, "main");
    sync(&mut project);
    assert_eq!(events(&project, &copy_output), expected);
    assert_eq!(events(&project, &output), expected);
    project
        .document
        .graph
        .set_atom_value(&copied_amount, AtomValue::Number(24))
        .unwrap();
    sync(&mut project);
    assert_eq!(
        project.document.graph.node(&amount),
        baseline.document.graph.node(&amount)
    );
    assert_eq!(events(&project, &output), expected);
    assert_ne!(events(&project, &copy_output), expected);
    let bytes = musaic::adapter::persistence::export_project_bytes(&project).unwrap();
    let loaded = musaic::adapter::persistence::import_project_bytes(&bytes, None).unwrap();
    assert_eq!(loaded.document.tricks, project.document.tricks);
    assert_eq!(
        events(&loaded, &copy_output),
        events(&project, &copy_output)
    );
}
#[test]
fn direct_command_is_one_undo() {
    let f = fixture();
    let mut app = support::editor(f.project);
    support::send(
        &mut app,
        C::SelectNode {
            node: f.instrument,
            mode: SelectionMode::Replace,
        },
    );
    let before = app.world().resource::<MusaicProject>().clone();
    support::send(&mut app, C::EditTiles(TileEdit::IndependentVariation));
    let after = app.world().resource::<MusaicProject>().clone();
    assert_eq!(after.document.tricks.len(), 4);
    support::send(&mut app, C::Undo);
    let restored = app.world().resource::<MusaicProject>();
    assert_eq!(restored.document.tricks, before.document.tricks);
    assert_eq!(
        restored.document.graph.nodes().collect::<Vec<_>>(),
        before.document.graph.nodes().collect::<Vec<_>>()
    );
    support::send(&mut app, C::Redo);
    assert_eq!(
        app.world().resource::<MusaicProject>().document.tricks,
        after.document.tricks
    );
}
#[test]
fn rejected_copy_is_atomic_and_nested_selection_includes_its_containing_pattern() {
    let mut p = MusaicProject::new_empty();
    let source = pattern(&mut p, 0, NoteName::C);
    let child = p
        .document
        .graph
        .nodes_on_surface(p.document.graph.container_surface(&source).unwrap())[0]
        .1
        .id
        .clone();
    sync(&mut p);
    let (mut selection, mut attention) = selected(&p, &child);
    editing::execute(
        &mut p,
        &mut selection,
        &mut attention,
        &mut Default::default(),
        &TileEdit::IndependentVariation,
    )
    .unwrap();
    assert_eq!(selection.nodes.len(), 1);
    let copied = selection.nodes.first().unwrap();
    assert_ne!(copied, &source);
    assert!(p.document.graph.container_surface(copied).is_some());
    let high = add(
        &mut p,
        100,
        i32::MAX - 1,
        TileSpawnKind::Atom {
            atom: AtomValue::Number(2),
        },
    );
    sync(&mut p);
    let before = p.clone();
    let (mut selection, mut attention) = selected(&p, &high);
    let old_selection = selection.clone();
    assert!(
        editing::execute(
            &mut p,
            &mut selection,
            &mut attention,
            &mut Default::default(),
            &TileEdit::IndependentVariation
        )
        .is_err()
    );
    assert_eq!(
        p.document.graph.nodes().collect::<Vec<_>>(),
        before.document.graph.nodes().collect::<Vec<_>>()
    );
    assert_eq!(p.document.tricks, before.document.tricks);
    assert_eq!(selection, old_selection);
}

#[test]
fn spatial_connections_stay_with_the_copy_and_regular_duplicate_keeps_links() {
    let starter = musaic::application::session::first_loop();
    let mut p = starter.project;
    sync(&mut p);
    let id =
        musaic::application::tricks::define(&mut p.document, starter.pattern, "Theme".into(), None)
            .unwrap();
    let call = processor(&mut p, 0, 22, id);
    sync(&mut p);
    let (mut selection, mut attention) = selected(&p, &call);
    editing::execute(
        &mut p,
        &mut selection,
        &mut attention,
        &mut Default::default(),
        &TileEdit::Duplicate,
    )
    .unwrap();
    assert_eq!(p.document.tricks.len(), 1);
    assert!(
        matches!(&p.document.graph.node(selection.nodes.first().unwrap()).unwrap().kind, DocumentNodeKind::TrickInstance(t) if t.prototype.0 == id)
    );
    let instrument = p
        .document
        .graph
        .nodes()
        .find(|n| matches!(&n.kind, DocumentNodeKind::Sound(_)))
        .unwrap()
        .id
        .clone();
    let originals: BTreeSet<_> = p.document.graph.nodes().map(|n| n.id.clone()).collect();
    let before = document::connection_policy::endpoint_connections(
        &document::export_document_program(&p.document).unwrap(),
    );
    assert!(!before.is_empty());
    let (mut selection, mut attention) = selected(&p, &instrument);
    editing::execute(
        &mut p,
        &mut selection,
        &mut attention,
        &mut Default::default(),
        &TileEdit::IndependentVariation,
    )
    .unwrap();
    let after = document::connection_policy::endpoint_connections(
        &document::export_document_program(&p.document).unwrap(),
    );
    for edge in &before {
        assert!(after.contains(edge));
    }
    let new: Vec<_> = after.difference(&before).collect();
    assert_eq!(new.len(), 1, "copied pattern still feeds its instrument");
    assert!(
        new.iter()
            .all(|edge| !originals.contains(&edge.from) && !originals.contains(&edge.to))
    );
}
