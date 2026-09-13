//! Section durations preserve each child clock instead of squeezing a song into one cycle.
use tessera::prelude::*;

fn modifier(value: AtomModifier) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Modifier(value))
}
fn source(speed: i64) -> TesseraProgram {
    let mut program = TesseraProgram::default();
    let theme = ContainerId::new("theme");
    let song = ContainerId::new("song");
    program.containers.insert(
        theme.clone(),
        Container::new(
            ContainerKind::Alternate,
            vec![
                ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("c"))),
                ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("d"))),
            ],
        ),
    );
    program.containers.insert(
        song.clone(),
        Container::new(
            ContainerKind::Arrangement,
            vec![
                ContainerSurfaceTile::NestedContainer(theme.clone()),
                modifier(AtomModifier::Elongate(Rational::from_integer(3))),
                ContainerSurfaceTile::NestedContainer(theme),
                modifier(AtomModifier::Fast(Rational::from_integer(speed))),
                modifier(AtomModifier::Elongate(Rational::from_integer(2))),
            ],
        ),
    );
    program.root_nodes.insert(
        NodeId::new("song"),
        RootSurfaceNodeKind::Container { container: song },
    );
    program.root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    program.relations.push(RootRelation::FlowsTo {
        from: StreamSource::node(NodeId::new("song")),
        to: StreamTarget::OutputInput {
            node: NodeId::new("out"),
            endpoint: InputEndpoint::GroupMember {
                group: PortGroupId::new("inputs"),
                member: PortMemberId::new("main"),
            },
        },
    });
    program
}
fn events(speed: i64, start: i64) -> Vec<(Rational, String)> {
    let saved = serde_json::to_vec(&source(speed)).unwrap();
    let program = serde_json::from_slice(&saved).unwrap();
    let compiled = TesseraCompiler::new()
        .compile_authored(&authored(&program))
        .unwrap();
    compiled.ir.outputs[0]
        .root
        .query(CycleSpan::new(
            CycleTime(Rational::from_integer(start)),
            CycleDuration(Rational::from_integer(5)),
        ))
        .events
        .iter()
        .filter_map(|event| match &event.value {
            EventValue::Note { value, .. } => Some((
                event.span.start.0 - Rational::from_integer(start),
                value.clone(),
            )),
            _ => None,
        })
        .collect()
}
#[test]
fn children_run_for_cycle_durations_reset_locally_and_respond_to_fast_at_late_seeks() {
    let slow = vec![(0, "c"), (1, "d"), (2, "c"), (3, "c"), (4, "d")]
        .into_iter()
        .map(|(time, note)| (Rational::from_integer(time), note.into()))
        .collect::<Vec<_>>();
    assert_eq!(events(1, 0), slow);
    assert_eq!(events(1, 100), slow);
    let fast = vec![
        (0, 1, "c"),
        (1, 1, "d"),
        (2, 1, "c"),
        (3, 1, "c"),
        (7, 2, "d"),
        (4, 1, "c"),
        (9, 2, "d"),
    ]
    .into_iter()
    .map(|(n, d, note)| (Rational::new(n, d), note.into()))
    .collect::<Vec<_>>();
    assert_eq!(events(2, 0), fast);
    assert_eq!(events(2, 100), fast);
}

fn reversed(nested: bool) -> PatternNodeIr {
    let theme = ContainerId::new("theme");
    let mut board = Board::new();
    let mut stack = vec![ContainerSurfaceTile::NestedContainer(theme.clone())];
    if nested {
        stack.push(modifier(AtomModifier::Rev));
    }
    board.at(0, 0).named("source").sequence(stack).unwrap();
    let output_x = if nested {
        1
    } else {
        board.at(1, 0).named("reverse").rev().unwrap();
        2
    };
    board.at(output_x, 0).named("out").output().unwrap();
    let mut source = board.finish();
    source.containers.insert(
        theme,
        Container::new(
            ContainerKind::Sequence,
            vec![
                ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("c"))),
                modifier(AtomModifier::Elongate(Rational::from_integer(2))),
                ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("d"))),
                ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("e"))),
            ],
        ),
    );
    let saved = serde_json::to_vec(&source).unwrap();
    TesseraCompiler::new()
        .compile_authored(&serde_json::from_slice(&saved).unwrap())
        .unwrap()
        .ir
        .outputs
        .remove(0)
        .root
}
#[test]
fn nested_rev_matches_connected_rev_through_serialization_and_direct_seeks() {
    let nested = reversed(true);
    let connected = reversed(false);
    let values = |node: &PatternNodeIr, span| {
        node.query(span)
            .events
            .into_iter()
            .map(|event| (event.span, event.value, event.fields))
            .collect::<Vec<_>>()
    };
    for (start, duration) in [
        (Rational::zero(), Rational::one()),
        (Rational::new(127, 4), Rational::new(7, 4)),
        (Rational::from_integer(101), Rational::from_integer(2)),
    ] {
        let span = CycleSpan::new(CycleTime(start), CycleDuration(duration));
        assert_eq!(values(&nested, span), values(&connected, span));
    }
    let whole = nested.query(CycleSpan::new(
        CycleTime(Rational::zero()),
        CycleDuration(Rational::one()),
    ));
    let pitches = whole
        .events
        .iter()
        .map(|event| match &event.value {
            EventValue::Note { value, .. } => value.as_str(),
            _ => panic!("note"),
        })
        .collect::<Vec<_>>();
    assert_eq!(pitches, ["e", "d", "c"]);
    assert!(whole.events.iter().all(|event| {
        !event
            .fields
            .iter()
            .any(|field| matches!(field, EventField::Reverse(_)))
    }));
}

#[test]
fn replication_changes_section_notes_without_multiplying_explicit_or_default_duration() {
    for explicit_duration in [None, Some(2)] {
        let mut source = source(1);
        let mut stack = vec![
            ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("c"))),
            modifier(AtomModifier::Replicate(4)),
            modifier(AtomModifier::Fast(Rational::from_integer(2))),
        ];
        if let Some(duration) = explicit_duration {
            stack.push(modifier(AtomModifier::Elongate(Rational::from_integer(
                duration,
            ))));
        }
        stack.push(ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(
            "d",
        ))));
        source
            .containers
            .get_mut(&ContainerId::new("song"))
            .unwrap()
            .stack = stack;
        let ir = TesseraCompiler::new()
            .compile_authored(&authored(&source))
            .unwrap()
            .ir
            .outputs
            .remove(0)
            .root;
        let duration = explicit_duration.unwrap_or(1);
        assert_eq!(
            ir.duration(),
            CycleDuration(Rational::from_integer(duration + 1))
        );
        let repeated = ir.query(CycleSpan::new(
            CycleTime(Rational::zero()),
            CycleDuration(Rational::from_integer(duration)),
        ));
        assert_eq!(
            repeated.events.len(),
            (8 * duration) as usize,
            "Repeat and Fast belong to the child clock"
        );
        let next = ir.query(CycleSpan::new(
            CycleTime(Rational::from_integer(duration)),
            CycleDuration(Rational::one()),
        ));
        assert_eq!(
            next.events.len(),
            1,
            "the next section begins after the authored duration"
        );
        assert!(matches!(&next.events[0].value, EventValue::Note { value, .. } if value == "d"));
    }
}

#[test]
fn section_duration_products_report_range_errors_and_reduce_cancelling_factors() {
    for (factors, valid) in [
        ([Rational::from_integer(4_000_000_000); 2], false),
        ([Rational::new(1, 4_000_000_000); 2], false),
        (
            [
                Rational::new(4_000_000_000, 3),
                Rational::new(3, 4_000_000_000),
            ],
            true,
        ),
    ] {
        let mut program = source(1);
        program
            .containers
            .get_mut(&ContainerId::new("song"))
            .unwrap()
            .stack = vec![
            ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("c"))),
            modifier(AtomModifier::Elongate(factors[0])),
            modifier(AtomModifier::Elongate(factors[1])),
        ];
        let result = std::panic::catch_unwind(|| {
            TesseraCompiler::new().compile_authored(&authored(&program))
        })
        .expect("individually valid duration tiles must never panic while compiling");
        if valid {
            assert_eq!(
                result.unwrap().ir.outputs[0].root.duration(),
                CycleDuration(Rational::one())
            );
        } else {
            assert!(
                result
                    .unwrap_err()
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("Arrangement duration product")),
                "normalization accepts each positive factor; its combined duration must report a range error"
            );
        }
    }
}
#[path = "support/compiler.rs"]
mod compiler_support;
use compiler_support::authored;
