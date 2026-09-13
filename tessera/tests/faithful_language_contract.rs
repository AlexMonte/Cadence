use tessera::prelude::*;

fn atom(note: &str) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(note)))
}
fn modifier(value: AtomModifier) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Modifier(value))
}
fn window(cycle: i64) -> CycleSpan {
    CycleSpan::new(
        CycleTime(Rational::from_integer(cycle)),
        CycleDuration(Rational::one()),
    )
}
fn program(stack: Vec<ContainerSurfaceTile>) -> TesseraProgram {
    let mut program = TesseraProgram::default();
    program.containers.insert(
        ContainerId::new("pattern"),
        Container::new(ContainerKind::Sequence, stack),
    );
    program.root_nodes.insert(
        NodeId::new("pattern"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("pattern"),
        },
    );
    program.root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    program.relations.push(RootRelation::FlowsTo {
        from: StreamSource::node(NodeId::new("pattern")),
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
fn labels(events: &[PatternEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| match &event.value {
            EventValue::Note { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn octave_weight_rate_ownership_survives_every_group_order_and_serialization() {
    let groups = [
        ContainerSurfaceTile::Atom(AtomTile::Octave(4)),
        modifier(AtomModifier::Elongate(Rational::from_integer(2))),
        modifier(AtomModifier::Fast(Rational::from_integer(3))),
    ];
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        let mut stack = vec![atom("c")];
        stack.extend(order.into_iter().map(|i| groups[i].clone()));
        stack.push(atom("d"));
        let source = program(stack);
        let source: TesseraProgram =
            serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
        let report = TesseraCompiler::new()
            .compile_authored(&authored(&source))
            .unwrap();
        let events = report.ir.outputs[0].root.query(window(0)).events;
        assert_eq!(labels(&events), vec!["c", "c", "c", "d"]);
        for (index, event) in events[..3].iter().enumerate() {
            assert_eq!(event.span.start.0, Rational::new(index as i64 * 2, 9));
            assert_eq!(event.span.duration.0, Rational::new(2, 9));
            assert!(matches!(
                event.value,
                EventValue::Note {
                    octave: Some(4),
                    ..
                }
            ));
        }
        assert_eq!(events[3].span.start.0, Rational::new(2, 3));
    }
}

#[test]
fn modifier_operands_are_not_taken_as_octaves() {
    let source = program(vec![
        atom("e"),
        ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Fast)),
        ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(2))),
    ]);
    let report = TesseraCompiler::new()
        .compile_authored(&authored(&source))
        .unwrap();
    assert_eq!(report.ir.outputs[0].root.query(window(0)).events.len(), 2);
    let invalid = program(vec![
        atom("e"),
        ContainerSurfaceTile::Atom(AtomTile::Operator(AtomOperatorToken::Fast)),
        ContainerSurfaceTile::Atom(AtomTile::Octave(4)),
        modifier(AtomModifier::Elongate(Rational::from_integer(2))),
    ]);
    assert!(
        TesseraCompiler::new()
            .compile_authored(&authored(&invalid))
            .is_err()
    );
}

#[test]
fn chromatic_pitch_retains_spelling_and_crosses_octave_boundaries() {
    for (letter, accidental, semitone, spelling) in [
        ("c", SignedAccidental::Sharp, 61, "c#"),
        ("d", SignedAccidental::Flat, 61, "db"),
        ("b", SignedAccidental::Sharp, 72, "b#"),
        ("c", SignedAccidental::Flat, 59, "cb"),
    ] {
        let note = NoteAtom::new(letter)
            .with_octave(4)
            .with_accidental(accidental);
        assert_eq!(note.semitone(4), semitone);
        let source = program(vec![ContainerSurfaceTile::Atom(AtomTile::Note(note))]);
        let report = TesseraCompiler::new()
            .compile_authored(&authored(&source))
            .unwrap();
        assert_eq!(
            labels(&report.ir.outputs[0].root.query(window(5)).events),
            vec![spelling]
        );
    }
}

#[test]
fn nested_alternate_and_gain_survive_eight_cycles_direct_seek_and_roundtrip() {
    let mut source = program(vec![
        ContainerSurfaceTile::NestedContainer(ContainerId::new("choices")),
        atom("g"),
    ]);
    source.containers.insert(
        ContainerId::new("choices"),
        Container::new(ContainerKind::Alternate, vec![atom("c"), atom("d")]),
    );
    source.root_nodes.insert(
        NodeId::new("gain"),
        RootSurfaceNodeKind::Transform(TransformNode::new(TransformKind::Gain)),
    );
    source.relations.clear();
    source.relations.push(RootRelation::FlowsTo {
        from: StreamSource::node(NodeId::new("pattern")),
        to: StreamTarget::TransformInput {
            node: NodeId::new("gain"),
            endpoint: InputEndpoint::Socket(InputPort::new("main")),
        },
    });
    source.relations.push(RootRelation::FlowsTo {
        from: StreamSource::node(NodeId::new("gain")),
        to: StreamTarget::OutputInput {
            node: NodeId::new("out"),
            endpoint: InputEndpoint::GroupMember {
                group: PortGroupId::new("inputs"),
                member: PortMemberId::new("main"),
            },
        },
    });
    let source: TesseraProgram =
        serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
    let compiled = TesseraCompiler::new()
        .compile_authored(&authored(&source))
        .unwrap();
    let root = &compiled.ir.outputs[0].root;
    let all = root.query(CycleSpan::new(
        CycleTime(Rational::zero()),
        CycleDuration(Rational::from_integer(8)),
    ));
    assert_eq!(all.events.len(), 16);
    for cycle in 0..8 {
        let events = root.query(window(cycle)).events;
        assert_eq!(
            labels(&events),
            vec![if cycle % 2 == 0 { "c" } else { "d" }, "g"]
        );
        assert_eq!(events[0].span.start.0, Rational::from_integer(cycle));
        assert_eq!(
            events[1].span.start.0,
            Rational::from_integer(cycle) + Rational::new(1, 2)
        );
        assert!(events.iter().all(|event| {
            event
                .fields
                .iter()
                .any(|field| matches!(field, EventField::Gain(_)))
        }));
        assert_eq!(
            events,
            all.events
                .iter()
                .filter(|event| event.span.start.0 >= Rational::from_integer(cycle)
                    && event.span.start.0 < Rational::from_integer(cycle + 1))
                .cloned()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn nested_pattern_rates_requery_future_cycles_and_slower_notes_hold() {
    let mut fast = program(vec![
        ContainerSurfaceTile::NestedContainer(ContainerId::new("choices")),
        modifier(AtomModifier::Fast(Rational::from_integer(2))),
    ]);
    fast.containers.insert(
        ContainerId::new("choices"),
        Container::new(ContainerKind::Alternate, vec![atom("c"), atom("d")]),
    );
    let fast_ir = TesseraCompiler::new()
        .compile_authored(&authored(&fast))
        .unwrap();
    for cycle in 0..8 {
        assert_eq!(
            labels(&fast_ir.ir.outputs[0].root.query(window(cycle)).events),
            vec!["c", "d"]
        );
    }
    let slow = program(vec![
        atom("c"),
        modifier(AtomModifier::Slow(Rational::from_integer(2))),
    ]);
    let slow_ir = TesseraCompiler::new()
        .compile_authored(&authored(&slow))
        .unwrap();
    let all = slow_ir.ir.outputs[0].root.query(CycleSpan::new(
        CycleTime(Rational::zero()),
        CycleDuration(Rational::from_integer(8)),
    ));
    // A one-child sequence must not split a held slow note at each cycle boundary.
    assert_eq!(all.events.len(), 4);
    for (i, event) in all.events.iter().enumerate() {
        assert_eq!(event.span.start.0, Rational::from_integer(i as i64 * 2));
        assert_eq!(event.span.duration.0, Rational::from_integer(2));
    }
}

#[test]
fn root_numeric_operand_drives_fast_and_authored_board_roundtrips() {
    let mut board = Board::new();
    board
        .at(0, 1)
        .named("pattern")
        .sequence(vec![atom("c"), atom("d")])
        .unwrap();
    board
        .at(1, 1)
        .named("fast")
        .transform(TransformKind::Fast)
        .unwrap();
    let literal = board
        .at(1, 0)
        .named("rate")
        .scalar(Rational::from_integer(3))
        .unwrap();
    board
        .bind_output_side(
            &literal,
            OutputEndpoint::Socket(OutputPort::new("out")),
            SpatialSide::South,
        )
        .unwrap();
    board.at(2, 1).named("out").output().unwrap();
    let json = serde_json::to_string(board.authored()).unwrap();
    let reopened: AuthoredTesseraProgram = serde_json::from_str(&json).unwrap();
    assert_eq!(&reopened, board.authored());
    let compiled = TesseraCompiler::new().compile_authored(&reopened).unwrap();
    assert_eq!(
        labels(&compiled.ir.outputs[0].root.query(window(7)).events),
        vec!["c", "d", "c", "d", "c", "d"]
    );
}

#[test]
fn slow_note_inside_a_sequence_has_stable_whole_span_when_seeking() {
    let source = program(vec![
        atom("c"),
        modifier(AtomModifier::Slow(Rational::from_integer(2))),
        atom("d"),
    ]);
    let compiled = TesseraCompiler::new()
        .compile_authored(&authored(&source))
        .unwrap();
    let root = &compiled.ir.outputs[0].root;
    let first = root.query(window(0));
    let second = root.query(window(1));
    let c_first = first
        .events
        .iter()
        .find(|e| labels(std::slice::from_ref(e)) == ["c"])
        .unwrap();
    let c_second = second
        .events
        .iter()
        .find(|e| labels(std::slice::from_ref(e)) == ["c"])
        .unwrap();
    assert_eq!(c_first.span, c_second.span);
    assert_eq!(c_first.span.start.0, Rational::zero());
    assert_eq!(c_first.span.end().0, Rational::new(3, 2));
    let all = root.query(CycleSpan::new(
        CycleTime(Rational::zero()),
        CycleDuration(Rational::from_integer(2)),
    ));
    assert_eq!(labels(&all.events), vec!["c", "d", "d"]);
}

#[test]
fn octave_tiles_preserve_explicit_pitch_ownership() {
    let explicit = program(
        SequenceStack::new()
            .note("e")
            .elongate(2)
            .octave(4)
            .fast(3)
            .build(),
    );
    let compiled = TesseraCompiler::new()
        .compile_authored(&authored(&explicit))
        .unwrap();
    let events = compiled.ir.outputs[0].root.query(window(0)).events;
    assert_eq!(events.len(), 3);
    assert!(events.iter().all(|event| matches!(
        event.value,
        EventValue::Note {
            octave: Some(4),
            ..
        }
    )));
}
#[path = "support/compiler.rs"]
mod compiler_support;
use compiler_support::authored;
