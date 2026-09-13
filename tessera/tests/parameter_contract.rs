use tessera::prelude::*;
fn r(value: i64) -> Rational {
    Rational::from_integer(value)
}
fn note(value: &str) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Note(NoteAtom::new(value)))
}
fn modify(value: AtomModifier) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Modifier(value))
}
fn span(cycle: i64) -> CycleSpan {
    CycleSpan::new(CycleTime(r(cycle)), CycleDuration(r(1)))
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
fn transformed(kind: TransformKind, aux: Container) -> TesseraProgram {
    let mut source = fixture(vec![note("c"), note("d")]);
    source.containers.insert(ContainerId::new("aux"), aux);
    source.root_nodes.insert(
        NodeId::new("aux"),
        RootSurfaceNodeKind::Container {
            container: ContainerId::new("aux"),
        },
    );
    source.root_nodes.insert(
        NodeId::new("effect"),
        RootSurfaceNodeKind::Transform(TransformNode::new(kind)),
    );
    source.relations = vec![
        RootRelation::FlowsTo {
            from: StreamSource::node(NodeId::new("pattern")),
            to: StreamTarget::TransformInput {
                node: NodeId::new("effect"),
                endpoint: InputEndpoint::Socket(InputPort::new("main")),
            },
        },
        RootRelation::FlowsTo {
            from: StreamSource::node(NodeId::new("aux")),
            to: StreamTarget::TransformInput {
                node: NodeId::new("effect"),
                endpoint: InputEndpoint::Socket(InputPort::new("amount")),
            },
        },
        RootRelation::FlowsTo {
            from: StreamSource::node(NodeId::new("effect")),
            to: StreamTarget::OutputInput {
                node: NodeId::new("out"),
                endpoint: InputEndpoint::GroupMember {
                    group: PortGroupId::new("inputs"),
                    member: PortMemberId::new("main"),
                },
            },
        },
    ];
    source
}
fn scalar(value: Rational) -> ContainerSurfaceTile {
    ContainerSurfaceTile::Atom(AtomTile::Scalar(ScalarAtom { value }))
}

#[test]
fn catalog_defaults_satisfy_declared_domains_and_sound_contracts() {
    for key in ParameterKey::ALL {
        let spec = key.spec();
        if let Some(value) = &spec.default {
            assert!(spec.validate(value).is_ok(), "{key:?}: {value:?}");
        }
        if let Some((low, high)) = spec.editor_range {
            assert!(low <= high);
        }
    }
    assert_eq!(ParameterKey::Attack.spec().unit, ParameterUnit::Seconds);
    assert_eq!(ParameterKey::Legato.spec().timing, ParameterTiming::Onset);
    assert_eq!(ParameterKey::Sustain.spec().unit, ParameterUnit::UnitLevel);
    assert_eq!(
        ParameterKey::SampleBank.spec().source,
        ParameterSource::SampleOnly
    );
    assert_eq!(
        ParameterKey::Transpose.spec().host_control_name,
        Some("transpose")
    );
    assert_eq!(
        ParameterKey::Transpose.spec().timing,
        ParameterTiming::ContinuousRuntime
    );
    assert_eq!(ParameterKey::Transpose.spec().merge, ParameterMerge::Add);
}

#[test]
fn note_length_and_envelope_level_do_not_move_the_next_onset() {
    let held = fixture(vec![
        note("c"),
        modify(AtomModifier::Legato(r(3))),
        modify(AtomModifier::Sustain(Rational::new(1, 4))),
        note("d"),
    ]);
    let held = TesseraCompiler::new()
        .compile_authored(&authored(&held))
        .unwrap()
        .ir;
    let json = serde_json::to_string(&held).unwrap();
    let held: PatternIr = serde_json::from_str(&json).unwrap();
    let events = held.outputs[0].root.query(span(0)).events;
    assert_eq!(events[0].span.duration.0, Rational::new(1, 2));
    assert_eq!(events[1].span.start.0, Rational::new(1, 2));
    assert!(
        events[0]
            .fields
            .contains(&EventField::Legato(FieldValue::rational(r(3))))
    );
    assert!(
        events[0]
            .fields
            .contains(&EventField::Sustain(FieldValue::rational(Rational::new(
                1, 4
            ))))
    );
    let weighted = fixture(vec![
        note("c"),
        modify(AtomModifier::Elongate(r(3))),
        note("d"),
    ]);
    let weighted = TesseraCompiler::new()
        .compile_authored(&authored(&weighted))
        .unwrap();
    assert_eq!(
        weighted.ir.outputs[0].root.query(span(0)).events[1]
            .span
            .start
            .0,
        Rational::new(3, 4)
    );
}

#[test]
fn patterned_note_length_and_cutoff_keep_alternating_values_on_seek() {
    for (kind, key) in [
        (TransformKind::Legato, "legato"),
        (TransformKind::LowPassCutoff, "lowpass_cutoff"),
        (TransformKind::SampleVariant, "sample_variant"),
    ] {
        let source = transformed(
            kind,
            Container::new(ContainerKind::Alternate, vec![scalar(r(2)), scalar(r(3))]),
        );
        let ir = TesseraCompiler::new()
            .compile_authored(&authored(&source))
            .unwrap()
            .ir;
        for cycle in 0..8 {
            let events = ir.outputs[0].root.query(span(cycle)).events;
            let notes = events
                .iter()
                .filter(|event| matches!(event.value, EventValue::Note { .. }))
                .collect::<Vec<_>>();
            assert_eq!(notes.len(), 2);
            assert_eq!(notes[0].span.start.0, r(cycle));
            assert_eq!(notes[1].span.start.0, r(cycle) + Rational::new(1, 2));
            let control = events.iter().find(|event| event.value.is_scalar()).unwrap();
            assert_eq!(
                control.value,
                EventValue::Scalar {
                    value: r(if cycle % 2 == 0 { 2 } else { 3 })
                }
            );
            assert!(
                control
                    .fields
                    .iter()
                    .any(|field| matches!(field,EventField::Custom {key: name,..} if name==key))
            );
        }
    }
}

#[test]
fn gate_uses_boolean_values_and_does_not_respace_events() {
    let source = transformed(
        TransformKind::Gate,
        Container::new(ContainerKind::Alternate, vec![scalar(r(1)), scalar(r(0))]),
    );
    let ir = TesseraCompiler::new()
        .compile_authored(&authored(&source))
        .unwrap()
        .ir;
    for cycle in 0..4 {
        let events = ir.outputs[0].root.query(span(cycle)).events;
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event.value, EventValue::Note { .. }))
                .count(),
            2
        );
        assert!(events.iter().any(|event| event.fields.iter().any(|field| matches!(field,EventField::Custom {key,value:FieldValue::Bool {value}} if key=="gate" && *value==(cycle%2==0)))));
    }
}

#[test]
fn all_pattern_branches_are_validated_before_publishing() {
    for (kind, bad) in [
        (TransformKind::Legato, r(0)),
        (TransformKind::Velocity, r(2)),
        (TransformKind::ClipLength, r(-1)),
        (TransformKind::PostGain, r(-1)),
        (TransformKind::PitchBend, r(2)),
        (TransformKind::Expression, r(2)),
        (TransformKind::Decay, r(-1)),
        (TransformKind::Release, r(-1)),
        (TransformKind::Pan, r(2)),
        (TransformKind::HighPassCutoff, r(0)),
        (TransformKind::HighPassResonance, r(2)),
        (TransformKind::LowPassCutoff, r(-1)),
        (TransformKind::Sustain, r(2)),
        (TransformKind::LowPassResonance, r(2)),
        (TransformKind::SampleVariant, Rational::new(1, 2)),
        (TransformKind::Gate, r(2)),
        (TransformKind::Transpose, r(128)),
    ] {
        let source = transformed(
            kind,
            Container::new(ContainerKind::Alternate, vec![scalar(r(1)), scalar(bad)]),
        );
        let diagnostics = TesseraCompiler::new()
            .compile_authored(&authored(&source))
            .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == DiagnosticKind::InvalidTransformArgument),
            "{kind:?}"
        );
    }
}

#[test]
fn sample_bank_and_variant_are_typed_selection_fields() {
    let source = fixture(vec![
        note("c"),
        modify(AtomModifier::SampleBank("breaks".into())),
        modify(AtomModifier::SampleVariant(2)),
        modify(AtomModifier::LowPassCutoff(r(1200))),
        modify(AtomModifier::Gate(true)),
    ]);
    let ir = TesseraCompiler::new()
        .compile_authored(&authored(&source))
        .unwrap()
        .ir;
    let events = ir.outputs[0].root.query(span(4)).events;
    assert_eq!(events.len(), 1);
    assert!(matches!(&events[0].value,EventValue::Note {value,..} if value=="c"));
    assert!(
        events[0]
            .fields
            .contains(&EventField::SampleBank(FieldValue::symbol("breaks")))
    );
    assert!(
        events[0]
            .fields
            .contains(&EventField::SampleVariant(FieldValue::rational(r(2))))
    );
    assert!(
        !events[0]
            .fields
            .iter()
            .any(|field| matches!(field, EventField::Select(_)))
    );
    let bad = fixture(vec![note("c"), modify(AtomModifier::SampleBank("".into()))]);
    assert!(
        TesseraCompiler::new()
            .compile_authored(&authored(&bad))
            .is_err()
    );
}

#[test]
fn typed_fields_roundtrip() {
    for field in [
        EventField::Gate(FieldValue::bool(true)),
        EventField::Legato(FieldValue::rational(r(3))),
        EventField::Sustain(FieldValue::rational(Rational::new(1, 4))),
        EventField::SampleBank(FieldValue::symbol("breaks")),
        EventField::SampleVariant(FieldValue::rational(r(2))),
        EventField::LowPassCutoff(FieldValue::rational(r(1200))),
    ] {
        let json = serde_json::to_string(&field).unwrap();
        assert_eq!(serde_json::from_str::<EventField>(&json).unwrap(), field);
    }
}

#[test]
fn sampler_groups_preserve_both_slice_operands_and_recursive_pattern_cycles() {
    let alternate = ContainerId::new("alternating_slices");
    let mut source = fixture(vec![ContainerSurfaceTile::NestedContainer(
        alternate.clone(),
    )]);
    source.containers.insert(
        alternate.clone(),
        Container::new(
            ContainerKind::Alternate,
            vec![
                note("c"),
                modify(AtomModifier::Slice {
                    index: 3,
                    count: 16,
                }),
                modify(AtomModifier::Fit(true)),
                modify(AtomModifier::Loop(true)),
                modify(AtomModifier::PlaybackRate(Rational::new(3, 2))),
                note("d"),
                modify(AtomModifier::Reverse(true)),
                modify(AtomModifier::Slice {
                    index: 15,
                    count: 16,
                }),
                modify(AtomModifier::PlaybackEnd(Rational::new(3, 4))),
                modify(AtomModifier::PlaybackStart(Rational::new(1, 4))),
            ],
        ),
    );
    let report = TesseraCompiler::new()
        .compile_authored(&authored(&source))
        .unwrap();
    let ir = &report.ir.outputs[0].root;
    let pattern = TesseraCompiler::new()
        .compile_normalized_container_ir(&report.normalized, &ContainerId::new("pattern"))
        .unwrap();
    for cycle in 0..8 {
        let events = ir.query(span(cycle)).events;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].span.start.0, r(cycle));
        assert_eq!(
            events[0].span.duration.0,
            r(1),
            "sample playback settings do not change sequence weight"
        );
        assert!(
            events[0]
                .fields
                .contains(&EventField::Slice(FieldValue::Slice {
                    index: if cycle % 2 == 0 { 3 } else { 15 },
                    count: 16
                }))
        );
        let named_events = pattern.query(span(cycle)).events;
        assert_eq!(named_events[0].value, events[0].value);
        assert_eq!(named_events[0].fields, events[0].fields);
        if cycle % 2 == 0 {
            assert!(
                events[0]
                    .fields
                    .contains(&EventField::Fit(FieldValue::bool(true)))
            );
            assert!(
                events[0]
                    .fields
                    .contains(&EventField::Loop(FieldValue::bool(true)))
            );
            assert!(
                events[0]
                    .fields
                    .contains(&EventField::PlaybackRate(FieldValue::rational(
                        Rational::new(3, 2)
                    )))
            );
        } else {
            assert!(
                events[0]
                    .fields
                    .contains(&EventField::Reverse(FieldValue::bool(true)))
            );
        }
    }
    let encoded = serde_json::to_string(&report.ir).unwrap();
    let decoded: PatternIr = serde_json::from_str(&encoded).unwrap();
    assert_eq!(report.ir, decoded);
    assert!(
        TesseraCompiler::new()
            .compile_normalized_container_ir(&report.normalized, &ContainerId::new("missing"))
            .is_err()
    );
}

#[test]
fn slice_and_sample_parameter_domains_reject_invalid_owned_values() {
    for (index, count) in [(0, 0), (16, 16), (0, 65_537), (u32::MAX, 16)] {
        let modifier = AtomModifier::Slice { index, count };
        assert!(
            TesseraCompiler::new()
                .compile_authored(&authored(&fixture(vec![note("c"), modify(modifier)])))
                .is_err()
        );
    }
    for (index, count) in [(0, 1), (15, 16), (65_535, 65_536)] {
        let modifier = AtomModifier::Slice { index, count };
        let value = modifier.parameter_value().unwrap();
        assert_eq!(value, FieldValue::Slice { index, count });
        assert_eq!(modifier.with_parameter_value(value).unwrap(), modifier);
        assert!(
            modifier
                .with_parameter_value(FieldValue::rational(r(2)))
                .is_err(),
            "a lone scalar cannot replace the pair"
        );
    }
    assert!(
        ParameterKey::PlaybackRate
            .spec()
            .validate(&FieldValue::rational(r(65_536)))
            .is_ok()
    );
    for rate in [0, -1, 65_537] {
        assert!(
            ParameterKey::PlaybackRate
                .spec()
                .validate(&FieldValue::rational(r(rate)))
                .is_err()
        );
    }
}

#[test]
fn envelope_highpass_and_pan_patterns_keep_branch_values_and_note_timing() {
    for (kind, low, high) in [
        (
            TransformKind::Decay,
            Rational::new(1, 4),
            Rational::new(3, 4),
        ),
        (
            TransformKind::Release,
            Rational::new(1, 4),
            Rational::new(3, 4),
        ),
        (TransformKind::Pan, r(-1), r(1)),
        (TransformKind::HighPassCutoff, r(120), r(1200)),
        (
            TransformKind::HighPassResonance,
            Rational::new(1, 4),
            Rational::new(3, 4),
        ),
    ] {
        let source = transformed(
            kind,
            Container::new(ContainerKind::Alternate, vec![scalar(low), scalar(high)]),
        );
        let ir = TesseraCompiler::new()
            .compile_authored(&authored(&source))
            .unwrap()
            .ir;
        let encoded = serde_json::to_string(&ir).unwrap();
        let ir: PatternIr = serde_json::from_str(&encoded).unwrap();
        for cycle in [0, 1, 4, 5, 1001] {
            let events = ir.outputs[0].root.query(span(cycle)).events;
            let notes = events
                .iter()
                .filter(|event| matches!(event.value, EventValue::Note { .. }))
                .collect::<Vec<_>>();
            assert_eq!(notes.len(), 2, "{kind:?}");
            assert_eq!(notes[0].span.start.0, r(cycle));
            assert_eq!(notes[1].span.start.0, r(cycle) + Rational::new(1, 2));
            let control = events.iter().find(|event| event.value.is_scalar()).unwrap();
            let expected = if cycle % 2 == 0 { low } else { high };
            assert_eq!(
                control.value,
                EventValue::Scalar { value: expected },
                "{kind:?}"
            );
            assert!(control.fields.iter().any(|field| matches!(field, EventField::Custom { key, .. } if key == kind.parameter_key().unwrap().spec().host_control_name.unwrap())));
        }
    }
}

#[test]
fn structured_effect_operands_validate_the_whole_group_before_compilation() {
    for modifier in [
        AtomModifier::Delay(DelayParameters::default()),
        AtomModifier::Reverb(ReverbParameters::default()),
        AtomModifier::Compressor(CompressorParameters::default()),
    ] {
        assert_eq!(
            modifier
                .with_parameter_value(modifier.parameter_value().unwrap())
                .unwrap(),
            modifier
        );
        assert!(
            modifier
                .with_parameter_value(FieldValue::rational(r(1)))
                .is_err()
        );
        let source = fixture(vec![note("c"), modify(modifier)]);
        let encoded = serde_json::to_string(&source).unwrap();
        let source: TesseraProgram = serde_json::from_str(&encoded).unwrap();
        TesseraCompiler::new()
            .compile_authored(&authored(&source))
            .unwrap();
    }
    for modifier in [
        AtomModifier::Delay(DelayParameters {
            time: r(0),
            ..Default::default()
        }),
        AtomModifier::Delay(DelayParameters {
            time: r(3),
            ..Default::default()
        }),
        AtomModifier::Reverb(ReverbParameters {
            amount: r(2),
            ..Default::default()
        }),
        AtomModifier::Compressor(CompressorParameters {
            attack: r(0),
            ..Default::default()
        }),
    ] {
        assert!(
            TesseraCompiler::new()
                .compile_authored(&authored(&fixture(vec![note("c"), modify(modifier)])))
                .is_err()
        );
    }
}
#[path = "support/compiler.rs"]
mod compiler_support;
use compiler_support::authored;
