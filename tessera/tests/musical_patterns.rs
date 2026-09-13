use tessera::prelude::*;

fn r(value: i64) -> Rational {
    Rational::from_integer(value)
}
fn scalar(value: i64) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom::integer(value)))
}
fn modify(value: AtomModifier) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Modifier(value))
}
fn fixture(stack: Vec<ContainerSurfaceTile>) -> TesseraProgram {
    let mut source = TesseraProgram::default();
    source.containers.insert(
        ContainerId::new("pattern"),
        Container::new(ContainerKind::Sequence, stack),
    );
    source.root_nodes.insert(
        NodeId::new("pattern"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("pattern"),
        },
    );
    source.root_nodes.insert(
        NodeId::new("out"),
        RootSurfaceNodeKind::Output(OutputNode::default()),
    );
    source.relations.push(RootRelation::FlowsTo {
        from: StreamSource::node(NodeId::new("pattern")),
        to: StreamTarget::OutputInput {
            node: NodeId::new("out"),
            endpoint: InputEndpoint::GroupMember {
                group: PortGroupId::new("inputs"),
                member: PortMemberId::new("main"),
            },
        },
    });
    source
}
fn compile(source: &TesseraProgram) -> PatternNodeIr {
    let saved = serde_json::to_vec(source).unwrap();
    let source = serde_json::from_slice(&saved).unwrap();
    TesseraCompiler::new()
        .compile_authored(&authored(&source))
        .unwrap()
        .ir
        .outputs
        .remove(0)
        .root
}
fn span(cycle: i64) -> CycleSpan {
    CycleSpan::new(CycleTime(r(cycle)), CycleDuration(r(1)))
}

#[test]
fn scale_degrees_cross_octaves_in_both_directions_and_keep_chords() {
    let scale = ScaleParameters {
        root: 62,
        mode: ScaleMode::Major,
    };
    for (degree, pitch) in [
        (-8, 49),
        (-7, 50),
        (-2, 59),
        (-1, 61),
        (0, 62),
        (2, 66),
        (4, 69),
        (6, 73),
        (7, 74),
        (14, 86),
    ] {
        assert_eq!(scale.pitch(r(degree)), Ok(pitch));
        assert_eq!(scale.note(r(degree)).unwrap().semitone(0), i64::from(pitch));
    }
    let mut source = fixture(vec![
        ContainerSurfaceTile::NestedContainer(ContainerId::new("chord")),
        modify(AtomModifier::Scale(scale)),
    ]);
    source.containers.insert(
        ContainerId::new("chord"),
        Container::new(
            ContainerKind::Layer,
            vec![scalar(-1), scalar(0), scalar(2), scalar(4)],
        ),
    );
    let node = compile(&source);
    let events = node.query(span(31)).events;
    assert_eq!(events.len(), 4);
    assert_eq!(
        events
            .iter()
            .map(|event| event.value.clone())
            .collect::<Vec<_>>(),
        vec![
            EventValue::Note {
                value: "c#".into(),
                octave: Some(4)
            },
            EventValue::Note {
                value: "d".into(),
                octave: Some(4)
            },
            EventValue::Note {
                value: "f#".into(),
                octave: Some(4)
            },
            EventValue::Note {
                value: "a".into(),
                octave: Some(4)
            },
        ]
    );
    assert!(
        events
            .iter()
            .all(|event| event.span == span(31) && event.source.is_some())
    );
}

#[test]
fn scale_rejects_fractional_and_out_of_range_degrees_without_panicking() {
    let scale = ScaleParameters {
        root: 62,
        mode: ScaleMode::Major,
    };
    for value in [
        Rational::new(1, 2),
        r(i64::MAX),
        Rational {
            numerator: i64::MIN,
            denominator: 1,
        },
    ] {
        assert!(scale.pitch(value).is_err());
        let source = fixture(vec![
            ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom { value })),
            modify(AtomModifier::Scale(scale)),
        ]);
        assert!(
            TesseraCompiler::new()
                .compile_authored(&authored(&source))
                .is_err()
        );
    }
    assert!(ScaleParameters { root: 128, ..scale }.validate().is_err());
    for mode in ScaleMode::ALL {
        let scale = ScaleParameters { mode, ..scale };
        assert_eq!(scale.pitch(r(mode.intervals().len() as i64)).unwrap(), 74);
        assert_eq!(
            scale.pitch(r(-(mode.intervals().len() as i64))).unwrap(),
            50
        );
    }
}

#[test]
fn independent_euclidean_inputs_match_fixed_rhythms_when_seeking_and_speeding_up() {
    let pattern = EuclidPatternParameters {
        pulses: vec![3, 1],
        steps: vec![8, 7, 6],
        rotations: vec![0, 2],
    };
    assert_eq!(pattern.period(), Ok(6));
    let note = || ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new("c")));
    let dynamic = compile(&fixture(vec![
        note(),
        modify(AtomModifier::EuclidPattern(pattern.clone())),
    ]));
    for cycle in -7_i64..=20 {
        let fixed = compile(&fixture(vec![
            note(),
            modify(AtomModifier::EuclidRot {
                pulses: pattern.pulses[cycle.rem_euclid(2) as usize],
                steps: pattern.steps[cycle.rem_euclid(3) as usize],
                rotation: pattern.rotations[cycle.rem_euclid(2) as usize],
            }),
        ]));
        assert_eq!(
            dynamic.query(span(cycle)),
            fixed.query(span(cycle)),
            "cycle {cycle}"
        );
    }
    let source = fixture(vec![
        note(),
        modify(AtomModifier::EuclidPattern(Default::default())),
        modify(AtomModifier::Fast(r(2))),
    ]);
    let events = compile(&source).query(span(0)).events;
    assert_eq!(
        events
            .iter()
            .map(|event| event.span.start.0)
            .collect::<Vec<_>>(),
        vec![
            r(0),
            Rational::new(3, 16),
            Rational::new(3, 8),
            Rational::new(5, 8)
        ]
    );
}

#[test]
fn euclidean_inputs_reject_unbounded_or_invalid_combined_patterns() {
    for pattern in [
        EuclidPatternParameters {
            pulses: vec![],
            ..Default::default()
        },
        EuclidPatternParameters {
            steps: vec![0],
            ..Default::default()
        },
        EuclidPatternParameters {
            steps: vec![1025],
            ..Default::default()
        },
        EuclidPatternParameters {
            pulses: vec![9],
            ..Default::default()
        },
        EuclidPatternParameters {
            pulses: vec![1; 11],
            steps: vec![8; 13],
            rotations: vec![0],
        },
    ] {
        assert!(pattern.validate().is_err());
        assert!(
            tessera::domain::stack::validate_modifier(&AtomModifier::EuclidPattern(pattern))
                .is_err()
        );
    }
}
#[path = "support/compiler.rs"]
mod compiler_support;
use compiler_support::authored;
