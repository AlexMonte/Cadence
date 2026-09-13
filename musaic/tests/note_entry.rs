//! Note shorthand produces ordinary persisted tiles, preserves owned expressions,
//! and commits through the real command bus with one undo and an explicit receipt.
use bevy::prelude::*;
use musaic::{
    adapter::persistence::{export_project_bytes, import_project_bytes},
    application::{
        command::{
            EditorCommand, EditorCommandBus, PlacementTarget,
            editing::TileEdit,
            note_entry::{self, NoteEntryReceipt, NoteEntryTarget},
        },
        editor::EditorPlugin,
        history::CommandHistory,
        pipeline::PlaybackPlugin,
        session::MusaicProject,
    },
    domain::{
        board::BoardSlot,
        document::{
            AtomValue, DocumentNodeKind, PlacementAddress, StackIndex, export_document_program,
        },
    },
    infrastructure::app::{AppState, TransportMode},
};
use tessera::prelude::NodeId;

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
            project: MusaicProject::new_empty(),
            path: None,
        },
    );
    app
}
fn send(app: &mut App, command: EditorCommand) {
    app.world_mut().write_message(EditorCommandBus(command));
    app.update();
}
fn insert(app: &mut App, request: u64, target: NoteEntryTarget, text: &str) -> Result<(), String> {
    insert_receipt(app, request, target, text).result
}
fn insert_receipt(
    app: &mut App,
    request: u64,
    target: NoteEntryTarget,
    text: &str,
) -> NoteEntryReceipt {
    send(
        app,
        EditorCommand::EditTiles(TileEdit::InsertNotes {
            request,
            target,
            text: text.into(),
        }),
    );
    let receipts: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<NoteEntryReceipt>>()
        .drain()
        .collect();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].request, request);
    receipts.into_iter().next().unwrap()
}
fn root_target(app: &App) -> NoteEntryTarget {
    NoteEntryTarget::Cursor(PlacementTarget::BoardSlot {
        surface: app
            .world()
            .resource::<MusaicProject>()
            .document
            .root_surface,
        slot: BoardSlot::new(-4, -3),
    })
}
fn sequence(app: &App) -> (NodeId, musaic::domain::board::BoardSurfaceId) {
    let project = app.world().resource::<MusaicProject>();
    let owner = project
        .document
        .graph
        .nodes()
        .find(|n| matches!(n.kind, DocumentNodeKind::Container(_)))
        .unwrap()
        .id
        .clone();
    let surface = project.document.graph.container_surface(&owner).unwrap();
    (owner, surface)
}
fn atoms(app: &App, surface: musaic::domain::board::BoardSurfaceId) -> Vec<(NodeId, AtomValue)> {
    let mut placed = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .nodes_on_surface(surface);
    placed.sort_by_key(|(loc, _)| match loc.address {
        PlacementAddress::StackIndex(index) => index.0,
        _ => unreachable!(),
    });
    placed
        .into_iter()
        .filter_map(|(_, n)| match &n.kind {
            DocumentNodeKind::Atom(a) => Some((n.id.clone(), a.atom.clone())),
            _ => None,
        })
        .collect()
}
#[test]
fn complete_melody_survives_save_reopen_and_one_undo_redo() {
    let mut app = app();
    let before = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .clone();
    let target = root_target(&app);
    insert(&mut app, 1, target, "C4 F#4 Bb3 ~").unwrap();
    let (_, surface) = sequence(&app);
    let expected: Vec<_> = note_entry::parse("C4 F#4 Bb3 ~")
        .unwrap()
        .into_iter()
        .flatten()
        .collect();
    assert_eq!(
        atoms(&app, surface)
            .into_iter()
            .map(|(_, atom)| atom)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
    let saved = export_project_bytes(app.world().resource::<MusaicProject>()).unwrap();
    let reopened = import_project_bytes(&saved, None).unwrap();
    assert_eq!(
        reopened.document.graph,
        app.world().resource::<MusaicProject>().document.graph
    );
    let authored =
        export_document_program(&app.world().resource::<MusaicProject>().document).unwrap();
    tessera::prelude::TesseraCompiler::new()
        .compile_authored(&authored)
        .unwrap();
    send(&mut app, EditorCommand::Undo);
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .nodes()
            .count(),
        before.nodes().count()
    );
    send(&mut app, EditorCommand::Redo);
    assert_eq!(
        app.world().resource::<MusaicProject>().document.graph,
        reopened.document.graph
    );
}
#[test]
fn insertion_from_an_octave_preserves_the_whole_expression_and_neighbor_ids() {
    let mut app = app();
    let target = root_target(&app);
    insert(&mut app, 1, target, "C#4").unwrap();
    let (container, surface) = sequence(&app);
    send(&mut app, EditorCommand::EnterContainer { container });
    send(
        &mut app,
        EditorCommand::PlaceTile {
            target: PlacementTarget::StackIndex {
                surface,
                index: StackIndex(3),
            },
            tile: musaic::domain::document::TileSpawnKind::Atom {
                atom: AtomValue::Modifier(tessera::prelude::AtomModifier::Fast(
                    tessera::prelude::Rational::from_integer(3),
                )),
            },
        },
    );
    insert(&mut app, 5, NoteEntryTarget::End(surface), "E4").unwrap();
    let original = atoms(&app, surface);
    assert!(matches!(original[3].1, AtomValue::Modifier(_)));
    let octave = original[2].0.clone();
    // Resolve all members, including speed, even when focus is on an octave.
    insert(
        &mut app,
        2,
        NoteEntryTarget::Expression {
            node: octave,
            after: true,
        },
        "G4 ~",
    )
    .unwrap();
    let result = atoms(&app, surface);
    assert_eq!(&result[..4], &original[..4]);
    assert_eq!(&result[7..], &original[4..]);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(atoms(&app, surface), original);
    // A raw cursor on the sharp cannot split pitch ownership either.
    insert(
        &mut app,
        3,
        NoteEntryTarget::Cursor(PlacementTarget::StackIndex {
            surface,
            index: StackIndex(1),
        }),
        "D3",
    )
    .unwrap();
    assert_eq!(&atoms(&app, surface)[2..], original.as_slice());
}
#[test]
fn invalid_tokens_occupied_board_and_missing_targets_leave_no_partial_edit() {
    let mut app = app();
    let target = root_target(&app);
    assert!(insert(&mut app, 1, target.clone(), "C4 nope G4").is_err());
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .nodes()
            .count(),
        0
    );
    insert(&mut app, 2, target.clone(), "E4").unwrap();
    let before = app.world().resource::<MusaicProject>().document.clone();
    assert!(insert(&mut app, 3, target, "C4 G4").is_err());
    assert!(
        insert(
            &mut app,
            4,
            NoteEntryTarget::Expression {
                node: NodeId::new("deleted"),
                after: false
            },
            "C4"
        )
        .is_err()
    );
    assert_eq!(
        app.world().resource::<MusaicProject>().document.graph,
        before.graph
    );
    assert_eq!(
        app.world().resource::<MusaicProject>().document.revision,
        before.revision
    );
    assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
}
#[test]
fn shorthand_accepts_accidentals_and_rests_but_never_guess_missing_octaves() {
    assert_eq!(
        note_entry::parse("c4 F♯5 B♭3 ~").unwrap(),
        note_entry::parse("C4 F#5 Bb3 ~").unwrap()
    );
    for text in [
        "", "C", "C#", "C-1", "C10", "C4,E4", "[C4 E4]", "C4@2", "h4", "D4x",
    ] {
        assert!(note_entry::parse(text).is_err(), "{text}");
    }
    assert_eq!(
        note_entry::parse("~ ~").unwrap(),
        vec![vec![AtomValue::Rest]; 2]
    );
    assert!(note_entry::parse(&"C4 ".repeat(129)).is_err());
}

#[test]
fn continuing_entry_uses_the_committed_anchor_and_one_undo_per_submission() {
    let mut app = app();
    let mut target = root_target(&app);
    let mut expected = Vec::new();
    for (index, text) in ["C4", "F#4 ~", "Bb3 E4 G4 C5 D5 E5 F5 G5 A5 B5", "~"]
        .iter()
        .enumerate()
    {
        let before = app
            .world()
            .resource::<MusaicProject>()
            .document
            .graph
            .clone();
        let receipt = insert_receipt(&mut app, index as u64 + 1, target, text);
        receipt.result.unwrap();
        target = receipt
            .continuation
            .expect("ordinary note entry has a next anchor");
        expected.extend(note_entry::parse(text).unwrap().into_iter().flatten());
        let (_, surface) = sequence(&app);
        assert_eq!(
            atoms(&app, surface)
                .into_iter()
                .map(|(_, atom)| atom)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            app.world().resource::<CommandHistory>().undo_len(),
            index + 1
        );
        let after = app
            .world()
            .resource::<MusaicProject>()
            .document
            .graph
            .clone();
        send(&mut app, EditorCommand::Undo);
        let restored = &app.world().resource::<MusaicProject>().document.graph;
        // Undo intentionally does not recycle IDs: compare authored nodes and
        // their positions, not the monotonic allocation counters.
        assert_eq!(restored.nodes().count(), before.nodes().count());
        for node in before.nodes() {
            assert_eq!(restored.node(&node.id), Some(node));
            assert_eq!(restored.location_of(&node.id), before.location_of(&node.id));
        }
        send(&mut app, EditorCommand::Redo);
        assert_eq!(
            app.world().resource::<MusaicProject>().document.graph,
            after
        );
    }
    let bytes = export_project_bytes(app.world().resource::<MusaicProject>()).unwrap();
    assert_eq!(
        import_project_bytes(&bytes, None).unwrap().document.graph,
        app.world().resource::<MusaicProject>().document.graph
    );
    let before = app.world().resource::<MusaicProject>().document.clone();
    let receipt = insert_receipt(&mut app, 99, target, "C4 invalid");
    assert!(receipt.result.is_err());
    assert!(receipt.continuation.is_none());
    assert_eq!(
        app.world().resource::<MusaicProject>().document.graph,
        before.graph
    );
}

#[test]
fn continuing_in_the_middle_keeps_order_and_rejects_a_deleted_anchor() {
    let mut app = app();
    let target = root_target(&app);
    insert(&mut app, 1, target, "C4 G4").unwrap();
    let (_, surface) = sequence(&app);
    let original = atoms(&app, surface);
    let receipt = insert_receipt(
        &mut app,
        2,
        NoteEntryTarget::Expression {
            node: original[2].0.clone(),
            after: false,
        },
        "D4",
    );
    receipt.result.unwrap();
    let receipt = insert_receipt(&mut app, 3, receipt.continuation.unwrap(), "E4");
    receipt.result.unwrap();
    let target = receipt.continuation.unwrap();
    assert_eq!(
        atoms(&app, surface)
            .into_iter()
            .map(|(_, atom)| atom)
            .collect::<Vec<_>>(),
        note_entry::parse("C4 D4 E4 G4")
            .unwrap()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    );
    send(&mut app, EditorCommand::Undo);
    let before = app
        .world()
        .resource::<MusaicProject>()
        .document
        .graph
        .clone();
    assert!(insert(&mut app, 4, target, "F4").is_err());
    assert_eq!(
        app.world().resource::<MusaicProject>().document.graph,
        before
    );
}

#[test]
fn entry_in_a_layer_creates_a_sequence_instead_of_a_chord() {
    use musaic::domain::document::{ContainerKind, TileSpawnKind};
    let mut app = app();
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
                slot: BoardSlot::new(0, 0),
            },
            tile: TileSpawnKind::Container {
                kind: ContainerKind::Parallel,
            },
        },
    );
    let (_, layer) = sequence(&app);
    let receipt = insert_receipt(&mut app, 1, NoteEntryTarget::End(layer), "C4 E4 ~");
    receipt.result.unwrap();
    let next = receipt.continuation.unwrap();
    let project = app.world().resource::<MusaicProject>();
    let children = project.document.graph.nodes_on_surface(layer);
    assert_eq!(children.len(), 1, "the sequence is one layer member");
    let DocumentNodeKind::Container(child) = &children[0].1.kind else {
        panic!("expected nested sequence")
    };
    assert_eq!(child.kind, ContainerKind::Sequence);
    assert_eq!(atoms(&app, child.local_surface).len(), 5);
    let nested_surface = child.local_surface;
    let authored =
        export_document_program(&app.world().resource::<MusaicProject>().document).unwrap();
    tessera::prelude::TesseraCompiler::new()
        .compile_authored(&authored)
        .unwrap();
    insert(&mut app, 2, next, "G4").unwrap();
    assert_eq!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .nodes_on_surface(layer)
            .len(),
        1,
        "continuing must not create another parallel branch"
    );
    assert_eq!(atoms(&app, nested_surface).len(), 7);
    send(&mut app, EditorCommand::Undo);
    assert_eq!(atoms(&app, nested_surface).len(), 5);
    send(&mut app, EditorCommand::Undo);
    assert!(
        app.world()
            .resource::<MusaicProject>()
            .document
            .graph
            .nodes_on_surface(layer)
            .is_empty()
    );
}

#[test]
fn entered_notes_produce_three_quarter_cycle_events_and_a_rest() {
    use cadence::prelude::{
        BuiltInSynthSource, CadenceCompiler, ControlKey, ControlValue, Intent, Span, Time,
    };
    use musaic::domain::document::TileSpawnKind;
    let mut app = app();
    let target = root_target(&app);
    insert(&mut app, 1, target, "C4 E4 G4 ~").unwrap();
    let (sequence, _) = sequence(&app);
    let root = app
        .world()
        .resource::<MusaicProject>()
        .document
        .root_surface;
    let mut route = vec![sequence];
    for (slot, tile) in [
        (
            BoardSlot::new(1, -3),
            TileSpawnKind::sound(musaic::domain::instrument::InstrumentDefinition::default()),
        ),
        (
            BoardSlot::new(2, -3),
            TileSpawnKind::Output {
                name: "Melody".into(),
            },
        ),
    ] {
        send(
            &mut app,
            EditorCommand::PlaceTile {
                target: PlacementTarget::BoardSlot {
                    surface: root,
                    slot,
                },
                tile,
            },
        );
        route.push(
            app.world()
                .resource::<MusaicProject>()
                .document
                .graph
                .nodes_on_surface(root)
                .into_iter()
                .find(|(loc, _)| loc.address == PlacementAddress::BoardSlot(slot))
                .unwrap()
                .1
                .id
                .clone(),
        );
    }
    for pair in route.windows(2) {
        send(
            &mut app,
            EditorCommand::ConnectTiles {
                from: pair[0].clone(),
                to: pair[1].clone(),
            },
        );
    }
    let authored =
        export_document_program(&app.world().resource::<MusaicProject>().document).unwrap();
    let ir = tessera::prelude::TesseraCompiler::new()
        .compile_authored(&authored)
        .unwrap()
        .ir;
    let (scores, diagnostics) =
        musaic::application::pipeline::lowering::lower_tessera_ir_with_sounds(
            &ir,
            &std::collections::BTreeMap::from([(
                route[1].clone(),
                Intent::synth(BuiltInSynthSource::Sine),
            )]),
        );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let prepared = cadence::prelude::PreparedScore::new(scores[&route[2]].clone()).unwrap();
    let report = CadenceCompiler::new()
        .preview(&prepared, &Span::new(Time::ZERO, Time::ONE).unwrap())
        .unwrap();
    let events: Vec<_> = report
        .starts()
        .map(|event| {
            (
                event.projected().whole(),
                event
                    .projected()
                    .controls()
                    .get(&ControlKey::Pitch)
                    .cloned(),
            )
        })
        .collect();
    assert_eq!(events.len(), 3, "the fourth step is a rest");
    for (i, (span, pitch)) in events.into_iter().enumerate() {
        assert_eq!(
            span,
            Span::new(Time::new(i as i64, 4), Time::new(i as i64 + 1, 4)).unwrap()
        );
        assert_eq!(pitch, Some(ControlValue::Scalar([60.0, 64.0, 67.0][i])));
    }
}
