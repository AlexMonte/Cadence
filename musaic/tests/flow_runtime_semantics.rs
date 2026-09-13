//! The authored language and the engine agree over multiple cycles and late seeks.
#[path = "../../tessera/tests/support/flow_cases.rs"]
mod flow_cases;
use cadence::prelude::{
    BuiltInSynthSource, CadenceCompiler, ControlKey, ControlValue, Intent, PreparedScore, Span,
    Time,
};
use std::collections::BTreeMap;
use tessera::prelude::*;
fn time(r: Rational) -> Time {
    Time::new(r.numerator, r.denominator)
}
fn key(event: &PatternEvent) -> Option<(Time, Time, i64)> {
    let EventValue::Note { value, octave } = &event.value else {
        return None;
    };
    let pitch = match value.as_str() {
        "c" => 0,
        "d" => 2,
        "e" => 4,
        "g" => 7,
        "a" => 9,
        "b" => 11,
        _ => panic!("unexpected note"),
    };
    Some((
        time(event.span.start.0),
        time(event.span.end().0),
        (octave.unwrap_or(4) + 1) * 12 + pitch,
    ))
}
#[test]
fn all_authored_flow_policies_match_cadence_over_cycles_and_seek_windows() {
    let mut failures = Vec::new();
    for (kind, policy) in flow_cases::policies() {
        let ir = flow_cases::authored(kind, policy.clone());
        let instruments = ir
            .outputs
            .iter()
            .map(|output| (output.id.clone(), Intent::synth(BuiltInSynthSource::Sine)))
            .collect::<BTreeMap<_, _>>();
        let (scores, diagnostics) =
            musaic::application::pipeline::lowering::lower_tessera_ir_with_sounds(
                &ir,
                &instruments,
            );
        assert!(diagnostics.is_empty(), "{policy:?}: {diagnostics:?}");
        for output in &ir.outputs {
            let prepared = PreparedScore::new(scores[&output.id].clone()).unwrap();
            for window in [
                flow_cases::span(0, 1, 4, 1),
                flow_cases::span(5, 4, 1, 4),
                flow_cases::span(30, 1, 4, 1),
                flow_cases::span(127, 4, 1, 4),
            ] {
                let report = CadenceCompiler::new()
                    .preview(
                        &prepared,
                        &Span::new(time(window.start.0), time(window.end().0)).unwrap(),
                    )
                    .unwrap();
                let mut seen = std::collections::BTreeSet::new();
                let mut actual = report
                    .events
                    .iter()
                    .filter(|event| {
                        let voice = match event.kind() {
                            cadence::application::EvaluatedEventKind::StartVoice {
                                voice_id,
                                ..
                            }
                            | cadence::application::EvaluatedEventKind::UpdateVoiceControls {
                                voice_id,
                                ..
                            } => voice_id.value(),
                        };
                        seen.insert(voice)
                    })
                    .map(|event| event.projected())
                    .map(|e| match e.controls().get(&ControlKey::Pitch) {
                        Some(ControlValue::Scalar(value)) => {
                            (e.whole().start(), e.whole().end(), *value as i64)
                        }
                        other => panic!("missing pitch {other:?}"),
                    })
                    .collect::<Vec<_>>();
                actual.sort();
                let mut expected = output
                    .root
                    .query(window)
                    .events
                    .iter()
                    .filter_map(key)
                    .collect::<Vec<_>>();
                expected.sort();
                if actual != expected {
                    failures.push(format!("{policy:?} output {} window {window:?}: got {actual:?}; expected {expected:?}",output.id.0));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn queried_mask_and_mix_gain_fields_reach_engine_controls() {
    for (kind, policy) in [
        (FlowControlKind::Mask, FlowControlPolicy::MaskScale),
        (FlowControlKind::Mix, FlowControlPolicy::MixWeighted),
        (FlowControlKind::Mix, FlowControlPolicy::MixGainAverage),
        (FlowControlKind::Mix, FlowControlPolicy::MixFieldBlend),
    ] {
        let ir = flow_cases::authored(kind, policy.clone());
        let instruments = ir
            .outputs
            .iter()
            .map(|out| (out.id.clone(), Intent::synth(BuiltInSynthSource::Sine)))
            .collect();
        let (scores, diagnostics) =
            musaic::application::pipeline::lowering::lower_tessera_ir_with_sounds(
                &ir,
                &instruments,
            );
        assert!(diagnostics.is_empty());
        for output in &ir.outputs {
            let prepared = PreparedScore::new(scores[&output.id].clone()).unwrap();
            for cycle in [0, 1, 30, 31] {
                let window = flow_cases::span(cycle, 1, 1, 1);
                let report = CadenceCompiler::new()
                    .preview(
                        &prepared,
                        &Span::new(Time::new(cycle, 1), Time::new(cycle + 1, 1)).unwrap(),
                    )
                    .unwrap();
                for event in output.root.query(window).events {
                    let Some((start, end, pitch)) = key(&event) else {
                        continue;
                    };
                    let gain = event.fields.iter().rev().find_map(|field| match field {
                        EventField::Gain(FieldValue::Rational { value }) => {
                            Some(value.numerator as f64 / value.denominator as f64)
                        }
                        _ => None,
                    });
                    if let Some(gain) = gain {
                        let actual = report
                            .starts()
                            .map(|event| event.projected())
                            .find(|e| {
                                e.whole().start() == start
                                    && e.whole().end() == end
                                    && e.controls().get(&ControlKey::Pitch)
                                        == Some(&ControlValue::Scalar(pitch as f64))
                            })
                            .unwrap();
                        assert_eq!(
                            actual.controls().get(&ControlKey::Gain),
                            Some(&ControlValue::Scalar(gain)),
                            "{policy:?} cycle {cycle}"
                        );
                    }
                }
            }
        }
    }
}

fn lower_node(node: PatternNodeIr) -> cadence::prelude::Score {
    let id = NodeId::new("out");
    let (scores, diagnostics) =
        musaic::application::pipeline::lowering::lower_tessera_ir_with_sounds(
            &PatternIr::new(vec![PatternOutput::new(id.clone(), node)]),
            &BTreeMap::from([(id.clone(), Intent::synth(BuiltInSynthSource::Sine))]),
        );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    scores[&id].clone()
}
fn live_render(
    score: cadence::prelude::Score,
    block: usize,
) -> Vec<cadence::adapter::audio::Frame> {
    use cadence::{
        adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
        infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    };
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8_000, 2048)).unwrap();
    let mut runtime = PlaybackRuntime::new(
        PlaybackSettings {
            cps: Time::ONE,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 64),
        },
        audio,
    );
    runtime
        .play_prepared_score(PreparedScore::new(score).unwrap())
        .unwrap();
    let mut result = vec![Frame::ZERO; 32_000];
    for chunk in result.chunks_mut(block) {
        runtime.tick().unwrap();
        renderer.render(chunk);
    }
    result
}
#[test]
fn recursive_flow_playback_changes_by_cycle_and_is_independent_of_render_block_size() {
    for (kind, policy) in [
        (FlowControlKind::Mix, FlowControlPolicy::MixWeighted),
        (FlowControlKind::Switch, FlowControlPolicy::SwitchCycleIndex),
        (FlowControlKind::Mask, FlowControlPolicy::MaskScale),
    ] {
        let ir = flow_cases::authored(kind, policy.clone());
        let score = lower_node(ir.outputs[0].root.clone());
        let a = live_render(score.clone(), 64);
        let b = live_render(score, 113);
        assert!(
            a.iter().any(|frame| frame.left.abs() > 0.001),
            "{policy:?} must be audible"
        );
        let largest = a
            .iter()
            .zip(&b)
            .map(|(a, b)| (a.left - b.left).abs().max((a.right - b.right).abs()))
            .fold(0f32, f32::max);
        assert!(
            largest < 0.0001,
            "{policy:?} changed by {largest} with block size"
        );
        if matches!(
            policy,
            FlowControlPolicy::MixWeighted | FlowControlPolicy::MaskScale
        ) {
            assert!(
                a[1000..7000].iter().all(|frame| frame.left.abs() < 0.00001),
                "zero control in first cycle is silent"
            );
            assert!(
                a[9000..15000].iter().any(|frame| frame.left.abs() > 0.001),
                "next control cycle is audible"
            );
        }
    }
}
#[test]
fn held_flow_query_preserves_lifecycle_identity_across_split_windows_and_late_seek() {
    let leaf = |name: &str| {
        PatternNodeIr::cycle_event_stream(PatternStream::new(vec![PatternEvent::new(
            flow_cases::span(0, 1, 5, 2),
            EventValue::Note {
                value: name.into(),
                octave: Some(4),
            },
        )]))
    };
    let input = |member: &str, node: PatternNodeIr| FlowInputIr {
        endpoint: InputEndpoint::GroupMember {
            group: PortGroupId::new("candidates"),
            member: PortMemberId::new(member),
        },
        nodes: vec![node],
    };
    let node = PatternNodeIr::FlowProjection {
        control: FlowControlNode::new(FlowControlKind::Switch),
        inputs: vec![input("a", leaf("c")), input("b", leaf("g"))],
        output: OutputEndpoint::Socket(OutputPort::new("out")),
    };
    let prepared = PreparedScore::new(lower_node(node.clone())).unwrap();
    let snapshot = |window: CycleSpan| {
        let report = CadenceCompiler::new()
            .preview(
                &prepared,
                &Span::new(time(window.start.0), time(window.end().0)).unwrap(),
            )
            .unwrap();
        let mut identities = BTreeMap::new();
        for event in &report.events {
            let voice = match event.kind() {
                cadence::application::EvaluatedEventKind::StartVoice { voice_id, .. }
                | cadence::application::EvaluatedEventKind::UpdateVoiceControls {
                    voice_id, ..
                } => voice_id.value(),
            };
            let moment = event.projected();
            let Some(ControlValue::Scalar(pitch)) = moment.controls().get(&ControlKey::Pitch)
            else {
                panic!("pitch")
            };
            identities
                .entry((moment.whole().start(), moment.whole().end(), *pitch as i64))
                .or_insert(voice);
        }
        identities
    };
    for origin in [0, 30] {
        let broad = snapshot(flow_cases::span(origin, 1, 4, 1));
        let window = flow_cases::span(origin * 4 + 7, 4, 1, 4);
        let partial = snapshot(window);
        let expected = node
            .query(window)
            .events
            .iter()
            .filter_map(key)
            .collect::<Vec<_>>();
        assert!(
            expected
                .iter()
                .any(|(start, _, _)| *start < time(window.start.0))
        );
        assert_eq!(partial.len(), expected.len());
        for note in expected {
            assert_eq!(
                partial[&note], broad[&note],
                "same held note must keep its voice identity"
            );
        }
    }
}

#[test]
fn constant_gain_after_mix_does_not_change_the_mix_inputs() {
    let mut program =
        flow_cases::authored_input(FlowControlKind::Mix, FlowControlPolicy::MixGainAverage);
    for (name, gain) in [("a", Rational::new(1, 2)), ("b", Rational::one())] {
        let container = program.containers.get_mut(&ContainerId::new(name)).unwrap();
        let mut stack = Vec::new();
        for tile in container.stack.clone() {
            stack.push(tile);
            stack.push(ContainerSurfaceTile::Atom(AtomTile::Modifier(
                AtomModifier::Gain(gain),
            )));
        }
        container.stack = stack;
    }
    program.root_surface.nodes.insert(
        NodeId::new("gain"),
        RootSurfaceNodeKind::Transform(TransformNode::new(TransformKind::Gain)),
    );
    program.root_surface.nodes.insert(
        NodeId::new("amount"),
        RootSurfaceNodeKind::Scalar(ScalarAtom::integer(2)),
    );
    program
        .root_surface
        .placements
        .insert(NodeId::new("gain"), RootPlacement::unit(100, 0));
    program
        .root_surface
        .placements
        .insert(NodeId::new("amount"), RootPlacement::unit(101, 0));
    for relation in &mut program.root_surface.explicit_relations {
        if let RootRelation::FlowsTo { to, .. } = relation {
            if matches!(to, StreamTarget::OutputInput { .. }) {
                *to = StreamTarget::TransformInput {
                    node: NodeId::new("gain"),
                    endpoint: InputEndpoint::Socket(InputPort::new("main")),
                };
            }
        }
    }
    program.root_surface.explicit_relations.extend([
        RootRelation::FlowsTo {
            from: StreamSource::node(NodeId::new("amount")),
            to: StreamTarget::TransformInput {
                node: NodeId::new("gain"),
                endpoint: InputEndpoint::Socket(InputPort::new("amount")),
            },
        },
        RootRelation::FlowsTo {
            from: StreamSource::node(NodeId::new("gain")),
            to: StreamTarget::OutputInput {
                node: NodeId::new("out0"),
                endpoint: InputEndpoint::GroupMember {
                    group: PortGroupId::new("inputs"),
                    member: PortMemberId::new("main"),
                },
            },
        },
    ]);
    let ir = TesseraCompiler::new()
        .compile_authored(&program)
        .unwrap()
        .ir;
    let score = lower_node(ir.outputs[0].root.clone());
    let prepared = PreparedScore::new(score).unwrap();
    let report = CadenceCompiler::new()
        .preview(&prepared, &Span::new(Time::ZERO, Time::ONE).unwrap())
        .unwrap();
    let gains = report
        .starts()
        .map(|event| {
            event
                .projected()
                .controls()
                .get(&ControlKey::Gain)
                .cloned()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        gains,
        vec![ControlValue::Scalar(0.75), ControlValue::Scalar(1.5)]
    );
}

#[test]
fn very_long_repeating_held_flow_is_rejected_before_expensive_query_preparation() {
    let input = |member: &str, note: &str| FlowInputIr {
        endpoint: InputEndpoint::GroupMember {
            group: PortGroupId::new("candidates"),
            member: PortMemberId::new(member),
        },
        nodes: vec![PatternNodeIr::cycle_event_stream(PatternStream::new(vec![
            PatternEvent::new(
                flow_cases::span(0, 1, 1_000, 1),
                EventValue::Note {
                    value: note.into(),
                    octave: Some(4),
                },
            ),
        ]))],
    };
    let score = lower_node(PatternNodeIr::FlowProjection {
        control: FlowControlNode::new(FlowControlKind::Switch),
        inputs: vec![input("a", "c"), input("b", "g")],
        output: OutputEndpoint::Socket(OutputPort::new("out")),
    });
    // Preparation must reject the excessive projection cost before playback.
    let error = PreparedScore::new(score).unwrap_err();
    assert!(
        error.to_string().contains("preparation budget"),
        "long held flow must fail at the preparation budget"
    );
}

#[test]
fn velocity_signal_onsets_keep_their_clock_inside_recursive_flow_queries() {
    let modulation = ModulationParameters {
        waveform: ModulationWaveform::Random,
        rate: Rational::from_integer(4),
        phase: Rational::new(1, 7),
        minimum: Rational::new(7, 10),
        maximum: Rational::one(),
        seed: 73,
    };
    let leaf = PatternNodeIr::cycle_event_stream(PatternStream::new(
        (0..8)
            .map(|index| {
                PatternEvent::new(
                    flow_cases::span(index, 8, 1, 8),
                    EventValue::Note {
                        value: "c".into(),
                        octave: Some(4),
                    },
                )
                .with_fields(vec![EventField::Velocity(FieldValue::Modulation {
                    value: modulation,
                })])
            })
            .collect(),
    ));
    let signal = cadence::prelude::Signal::random(73)
        .with_rate(Time::new(4, 1))
        .with_phase(Time::new(1, 7))
        .with_bias(0.85)
        .with_depth(0.15);
    let transformed = PatternNodeIr::shift(
        PatternNodeIr::reflect_cycle(PatternNodeIr::time_scale(leaf, Rational::new(1, 2))),
        CycleDuration(Rational::new(7, 3)),
    );
    let root = PatternNodeIr::FlowProjection {
        control: FlowControlNode::new(FlowControlKind::Layer),
        inputs: vec![FlowInputIr {
            endpoint: InputEndpoint::GroupMember {
                group: PortGroupId::new("streams"),
                member: PortMemberId::new("a"),
            },
            nodes: vec![transformed],
        }],
        output: OutputEndpoint::Socket(OutputPort::new("out")),
    };
    let prepared = PreparedScore::new(lower_node(root)).unwrap();
    let window = Span::new(Time::new(7, 3), Time::new(10, 3)).unwrap();
    let report = CadenceCompiler::new().preview(&prepared, &window).unwrap();
    assert_eq!(report.events.len(), 16);
    for event in report.events {
        let onset = event.projected().whole().start();
        let local_start = onset - Time::new(7, 3);
        let source_start = (Time::ONE - local_start - Time::new(1, 16)) * Time::new(2, 1);
        let ControlValue::Unipolar(actual) = event.projected().controls()[&ControlKey::Velocity]
        else {
            panic!("expected onset velocity")
        };
        assert!((actual.value() - signal.eval_at(source_start)).abs() < 1e-12);
        let seek = onset + Time::new(1, 64);
        let partial = CadenceCompiler::new()
            .preview(
                &prepared,
                &Span::new(seek, seek + Time::new(1, 64)).unwrap(),
            )
            .unwrap();
        assert_eq!(partial.events.len(), 1);
        assert_eq!(
            partial.events[0].projected().controls()[&ControlKey::Velocity],
            ControlValue::Unipolar(actual)
        );
    }
}
