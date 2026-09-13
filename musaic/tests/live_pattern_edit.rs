//! Music survives the actual document → Tessera → Cadence → file boundary.
use cadence::prelude::{CadenceCompiler, ControlKey, ControlValue, PreparedScore, Span, Time};
use musaic::{
    adapter::persistence::{export_project_bytes, import_project_bytes},
    application::{
        command::{EditorCommand, execute_command, execute_inverse},
        editor::{EditorAttention, SelectionState},
        pipeline::{lowering::lower_tessera_ir, runtime::TimelineProvenanceStore},
        session::MusaicProject,
    },
    domain::{
        board::BoardSlot,
        document::{
            Accidental, AtomValue, ContainerKind, GraphTilePrototypeId, MusaicDocument, NoteName,
            OperatorValue, PlacementAddress, StackIndex, TileSpawnKind, bind_tiles,
            export_document_program,
        },
    },
};
use proptest::prelude::*;
use tessera::prelude::{NodeId, SpatialSide, TesseraCompiler};

fn pattern(
    rate: i32,
    octave: i8,
    accidental: Accidental,
    groups: &[usize],
) -> (MusaicDocument, NodeId) {
    let mut document = MusaicDocument::new_empty();
    let root = document.root_surface;
    let sequence = document
        .graph
        .insert_tile(
            &mut document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
            TileSpawnKind::Container {
                kind: ContainerKind::Sequence,
            },
        )
        .unwrap();
    let surface = document.graph.container_surface(&sequence).unwrap();
    let owned = [
        vec![AtomValue::Octave(octave)],
        vec![AtomValue::Operator(OperatorValue::At), AtomValue::Number(2)],
        vec![
            AtomValue::Operator(OperatorValue::Multiply),
            AtomValue::Number(3),
        ],
    ];
    let mut atoms = vec![
        AtomValue::NoteName(NoteName::C),
        AtomValue::Accidental(accidental),
    ];
    for group in groups {
        atoms.extend(owned[*group].clone());
    }
    atoms.extend([AtomValue::NoteName(NoteName::E), AtomValue::Octave(octave)]);
    for (index, atom) in atoms.into_iter().enumerate() {
        document
            .graph
            .insert_tile(
                &mut document.surfaces,
                surface,
                PlacementAddress::StackIndex(StackIndex(index)),
                TileSpawnKind::Atom { atom },
            )
            .unwrap();
    }
    let fast = document
        .graph
        .insert_tile(
            &mut document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(5, 0)),
            TileSpawnKind::TrickInstance {
                prototype: GraphTilePrototypeId(1),
            },
        )
        .unwrap();
    let number = document
        .graph
        .insert_tile(
            &mut document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(5, -1)),
            TileSpawnKind::Atom {
                atom: AtomValue::Number(rate),
            },
        )
        .unwrap();
    let output = document
        .graph
        .insert_tile(
            &mut document.surfaces,
            root,
            PlacementAddress::BoardSlot(BoardSlot::new(6, 0)),
            TileSpawnKind::Output {
                name: "main".into(),
            },
        )
        .unwrap();
    let mut program = export_document_program(&document).unwrap();
    bind_tiles(&mut program, &sequence, &fast, SpatialSide::East).unwrap();
    bind_tiles(&mut program, &number, &fast, SpatialSide::South).unwrap();
    bind_tiles(&mut program, &fast, &output, SpatialSide::East).unwrap();
    document.replace_connections_from(&program);
    document.validate().unwrap();
    (document, number)
}

fn music(document: &MusaicDocument, cycle: i64) -> Vec<(Time, Time, i64)> {
    let program = export_document_program(document).expect("export document");
    let report = TesseraCompiler::new()
        .compile_authored(&program)
        .expect("compile authored pattern");
    let (scores, diagnostics) = lower_tessera_ir(&report.ir);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(scores.len(), 1);
    let offset = Time::new(cycle, 1);
    let window = Span::new(offset, offset + Time::ONE).unwrap();
    let prepared = PreparedScore::new(scores.values().next().unwrap().clone()).unwrap();
    let report = CadenceCompiler::new().preview(&prepared, &window).unwrap();
    report
        .starts()
        .map(|event| {
            let projected = event.projected();
            let Some(ControlValue::Scalar(pitch)) = projected.controls().get(&ControlKey::Pitch)
            else {
                panic!("note has no chromatic pitch: {:?}", projected.controls());
            };
            (
                projected.whole().start() - offset,
                projected.whole().end() - offset,
                *pitch as i64,
            )
        })
        .collect()
}

fn expected(rate: i64, octave: i8, accidental: Accidental) -> Vec<(Time, Time, i64)> {
    let pitch = (i64::from(octave) + 1) * 12;
    let shift = match accidental {
        Accidental::Sharp => 1,
        Accidental::Flat => -1,
        Accidental::Natural => 0,
    };
    let mut events = Vec::new();
    for cycle in 0..rate {
        for hit in 0..3 {
            events.push((
                Time::new(cycle * 9 + hit * 2, 9 * rate),
                Time::new(cycle * 9 + (hit + 1) * 2, 9 * rate),
                pitch + shift,
            ));
        }
        events.push((
            Time::new(cycle * 3 + 2, 3 * rate),
            Time::new(cycle + 1, rate),
            pitch + 4,
        ));
    }
    events
}

#[test]
fn fast_value_edit_updates_exact_onsets_and_can_be_undone_and_reopened() {
    let (mut document, number) = pattern(2, 4, Accidental::Sharp, &[0, 1, 2]);
    assert_eq!(music(&document, 0), expected(2, 4, Accidental::Sharp));
    let mut attention = EditorAttention::new(document.root_surface);
    let mut selection = SelectionState::default();
    let command = EditorCommand::SetAtomValue {
        node: number,
        value: AtomValue::Number(3),
    };
    let result = execute_command(
        &mut document,
        &mut attention,
        &mut selection,
        &TimelineProvenanceStore::default(),
        &command,
    )
    .unwrap();
    assert!(result.is_accepted(), "{:?}", result.diagnostics);
    assert_eq!(music(&document, 0), expected(3, 4, Accidental::Sharp));
    assert_eq!(music(&document, 7), expected(3, 4, Accidental::Sharp));
    execute_inverse(
        &mut document,
        &mut attention,
        &mut selection,
        &result.undo.unwrap(),
    )
    .unwrap();
    assert_eq!(music(&document, 0), expected(2, 4, Accidental::Sharp));
    execute_command(
        &mut document,
        &mut attention,
        &mut selection,
        &TimelineProvenanceStore::default(),
        &command,
    )
    .unwrap();
    let mut project = MusaicProject::new_empty();
    project.document = document;
    let reopened = import_project_bytes(
        &export_project_bytes(&project).expect("save nonempty project"),
        None,
    )
    .expect("reopen nonempty project");
    assert_eq!(reopened.document.graph, project.document.graph);
    assert_eq!(
        music(&reopened.document, 7),
        expected(3, 4, Accidental::Sharp)
    );
}

const PERMUTATIONS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

#[test]
fn editor_group_moves_keep_values_attached_and_undo_the_exact_move() {
    use musaic::domain::document::DocumentNodeKind;
    for target_order in PERMUTATIONS {
        let (mut document, _) = pattern(1, 4, Accidental::Sharp, &[0, 1, 2]);
        let owners: Vec<_> = [
            AtomValue::Octave(4),
            AtomValue::Operator(OperatorValue::At),
            AtomValue::Operator(OperatorValue::Multiply),
        ]
        .iter()
        .map(|value| {
            document
                .graph
                .nodes()
                .find(|node| {
                    matches!(&node.kind,
                DocumentNodeKind::Atom(atom) if &atom.atom == value)
                })
                .unwrap()
                .id
                .clone()
        })
        .collect();
        let mut current = [0, 1, 2];
        let mut attention = EditorAttention::new(document.root_surface);
        let mut selection = SelectionState::default();
        for (position, wanted) in target_order.into_iter().enumerate() {
            let mut index = current.iter().position(|g| *g == wanted).unwrap();
            while index > position {
                let before = document.graph.clone();
                let command = EditorCommand::MoveModifierGroup {
                    owner: owners[wanted].clone(),
                    step: -1,
                };
                let result = execute_command(
                    &mut document,
                    &mut attention,
                    &mut selection,
                    &TimelineProvenanceStore::default(),
                    &command,
                )
                .unwrap();
                assert!(result.is_accepted(), "{:?}", result.diagnostics);
                assert_eq!(music(&document, 7), expected(1, 4, Accidental::Sharp));
                execute_inverse(
                    &mut document,
                    &mut attention,
                    &mut selection,
                    &result.undo.unwrap(),
                )
                .unwrap();
                assert_eq!(
                    document.graph, before,
                    "undo restores exact positions and IDs"
                );
                execute_command(
                    &mut document,
                    &mut attention,
                    &mut selection,
                    &TimelineProvenanceStore::default(),
                    &command,
                )
                .unwrap();
                current.swap(index, index - 1);
                index -= 1;
            }
        }
        let mut project = MusaicProject::new_empty();
        project.document = document;
        let reopened =
            import_project_bytes(&export_project_bytes(&project).unwrap(), None).unwrap();
        assert_eq!(reopened.document.graph, project.document.graph);
        assert_eq!(
            music(&reopened.document, 7),
            expected(1, 4, Accidental::Sharp)
        );
    }
}

#[test]
fn fractional_fast_value_survives_editing_and_save_without_rounding() {
    let (mut document, number) = pattern(2, 4, Accidental::Natural, &[0, 1, 2]);
    let mut attention = EditorAttention::new(document.root_surface);
    let result = execute_command(
        &mut document,
        &mut attention,
        &mut SelectionState::default(),
        &TimelineProvenanceStore::default(),
        &EditorCommand::SetAtomValue {
            node: number.clone(),
            value: AtomValue::Ratio(tessera::prelude::Rational::new(1, 2)),
        },
    )
    .unwrap();
    assert!(result.is_accepted());
    let mut project = MusaicProject::new_empty();
    project.document = document;
    let reopened = import_project_bytes(&export_project_bytes(&project).unwrap(), None).unwrap();
    assert_eq!(reopened.document.graph, project.document.graph);
    let events = music(&reopened.document, 0);
    assert_eq!(
        events,
        vec![
            (Time::ZERO, Time::new(4, 9), 60),
            (Time::new(4, 9), Time::new(8, 9), 60),
            (Time::new(8, 9), Time::new(4, 3), 60)
        ]
    );
}

#[test]
fn deleting_several_nested_notes_preserves_alternate_and_undo_restores_every_child() {
    use musaic::{
        application::editor::selection::SelectionMode, domain::document::DocumentNodeKind,
    };
    let mut document = MusaicProject::demo().document;
    let alternate = document
        .graph
        .nodes()
        .find(|node| {
            matches!(&node.kind,
        DocumentNodeKind::Container(container) if container.kind == ContainerKind::Alternating)
        })
        .unwrap()
        .id
        .clone();
    let alternate_surface = document.graph.container_surface(&alternate).unwrap();
    let branch = document
        .graph
        .nodes_on_surface(alternate_surface)
        .into_iter()
        .next()
        .unwrap()
        .1
        .id
        .clone();
    let branch_surface = document.graph.container_surface(&branch).unwrap();
    let children = document
        .graph
        .nodes_on_surface(branch_surface)
        .into_iter()
        .map(|(_, n)| n.id.clone())
        .collect::<Vec<_>>();
    let before = document.graph.clone();
    let mut attention = EditorAttention::new(branch_surface);
    let mut selection = SelectionState::default();
    for child in &children {
        selection.select(child.clone(), SelectionMode::Add);
    }
    let result = execute_command(
        &mut document,
        &mut attention,
        &mut selection,
        &TimelineProvenanceStore::default(),
        &EditorCommand::DeleteSelection,
    )
    .unwrap();
    assert!(result.is_accepted(), "{:?}", result.diagnostics);
    assert!(document.graph.nodes_on_surface(branch_surface).is_empty());
    assert_eq!(
        export_document_program(&document).unwrap().containers
            [&tessera::prelude::ContainerId::new(alternate.0)]
            .kind,
        tessera::prelude::ContainerKind::Alternate
    );
    execute_inverse(
        &mut document,
        &mut attention,
        &mut selection,
        &result.undo.unwrap(),
    )
    .unwrap();
    assert_eq!(document.graph, before);
}

#[test]
fn all_owned_group_orders_preserve_distinct_values_through_project_files() {
    for order in PERMUTATIONS {
        let (document, _) = pattern(1, 4, Accidental::Sharp, &order);
        let mut project = MusaicProject::new_empty();
        project.document = document;
        let reopened =
            import_project_bytes(&export_project_bytes(&project).unwrap(), None).unwrap();
        assert_eq!(
            music(&reopened.document, 7),
            expected(1, 4, Accidental::Sharp),
            "order {order:?}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(36))]
    #[test]
    fn pitch_rate_and_owned_groups_survive_save_and_direct_seek(
        octave in -1i8..=8, rate in 1i32..=6, alteration in 0usize..3, order in 0usize..6,
        cycle in 0i64..16,
    ) {
        let accidental = [Accidental::Flat, Accidental::Natural, Accidental::Sharp][alteration];
        let (document, _) = pattern(rate, octave, accidental, &PERMUTATIONS[order]);
        let mut project = MusaicProject::new_empty();
        project.document = document;
        let reopened = import_project_bytes(&export_project_bytes(&project).unwrap(), None).unwrap();
        prop_assert_eq!(&reopened.document.graph, &project.document.graph);
        prop_assert_eq!(music(&reopened.document, cycle), expected(i64::from(rate), octave, accidental));
    }
}

fn source_preview(
    document: &MusaicDocument,
    cycle: i64,
    snapshot: &mut musaic::application::pipeline::runtime::RuntimePreviewSnapshot,
) -> TimelineProvenanceStore {
    use musaic::application::pipeline::lowering::lower_tessera_ir_with_sources;
    let program = export_document_program(document).unwrap();
    let report = TesseraCompiler::new().compile_authored(&program).unwrap();
    let (scores, diagnostics, sources) =
        lower_tessera_ir_with_sources(&report.ir, &Default::default());
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    snapshot.clear();
    let window = Span::new(Time::new(cycle, 1), Time::new(cycle + 1, 1)).unwrap();
    for (output, score) in scores {
        let prepared = PreparedScore::new(score).unwrap();
        let report = CadenceCompiler::new().preview(&prepared, &window).unwrap();
        snapshot.ingest_report(&output.0, &report);
    }
    let mut provenance = TimelineProvenanceStore::default();
    provenance.rebuild(snapshot, &sources, document);
    provenance
}

#[test]
fn preview_jump_selects_authored_notes_after_owned_groups_reorder_and_save() {
    use musaic::application::{
        editor::{ActiveSpace, FocusTarget, TimelineSourceResolver},
        pipeline::runtime::RuntimePreviewSnapshot,
    };
    use musaic::domain::document::DocumentNodeKind;
    for order in PERMUTATIONS {
        let (document, _) = pattern(3, 4, Accidental::Sharp, &order);
        let mut project = MusaicProject::new_empty();
        project.document = document;
        let mut document = import_project_bytes(&export_project_bytes(&project).unwrap(), None)
            .unwrap()
            .document;
        let mut snapshot = RuntimePreviewSnapshot::default();
        let provenance = source_preview(&document, 7, &mut snapshot);
        assert_eq!(snapshot.starts().count(), 12);
        for event in snapshot.starts() {
            let source = provenance
                .source_for_event(event.id)
                .expect("real compiled note has a source");
            assert!(matches!(document.graph.node(&source.node).unwrap().kind,
                DocumentNodeKind::Atom(ref atom) if matches!(atom.atom, AtomValue::NoteName(_))));
            let mut attention = EditorAttention::new(document.root_surface);
            let mut selection = SelectionState::default();
            let result = execute_command(
                &mut document,
                &mut attention,
                &mut selection,
                &provenance,
                &EditorCommand::JumpToTimelineSource { event: event.id },
            )
            .unwrap();
            assert!(result.is_accepted(), "{:?}", result.diagnostics);
            assert_eq!(attention.active_space, ActiveSpace::Board(source.surface));
            assert_eq!(
                attention.focus,
                FocusTarget::Atom {
                    node: source.node.clone()
                }
            );
            assert!(selection.contains(source.node));
        }
    }
}

#[test]
fn preview_sources_follow_nested_alternate_and_old_preview_ids_are_retired() {
    use musaic::application::{
        editor::TimelineSourceResolver, pipeline::runtime::RuntimePreviewSnapshot,
    };
    use musaic::domain::document::DocumentNodeKind;
    let document = MusaicProject::demo().document;
    let mut snapshot = RuntimePreviewSnapshot::default();
    let first = source_preview(&document, 0, &mut snapshot);
    let old_ids = snapshot.events().map(|event| event.id).collect::<Vec<_>>();
    assert!(!old_ids.is_empty());
    assert!(
        old_ids
            .iter()
            .all(|id| first.source_for_event(*id).is_some())
    );
    let second = source_preview(&document, 1, &mut snapshot);
    assert!(
        old_ids
            .iter()
            .all(|id| snapshot.event(*id).is_none() && second.source_for_event(*id).is_none())
    );
    assert!(snapshot.events().all(|event| {
        let source = second
            .source_for_event(event.id)
            .expect("nested note source");
        source.surface != document.root_surface
            && matches!(document.graph.node(&source.node).unwrap().kind,
            DocumentNodeKind::Atom(ref atom) if matches!(atom.atom, AtomValue::NoteName(_)))
    }));
}
