//! Authored flow policies retain recursive source clocks and whole-event decisions.
use tessera::prelude::*;
#[path = "support/flow_cases.rs"]
mod flow_cases;
use flow_cases::{authored, policies, span};
fn input(group: &str, member: &str) -> InputEndpoint {
    InputEndpoint::GroupMember {
        group: PortGroupId::new(group),
        member: PortMemberId::new(member),
    }
}
fn visible(events: &[PatternEvent], window: CycleSpan) -> Vec<PatternEvent> {
    events
        .iter()
        .filter(|e| e.span.start < window.end() && window.start < e.span.end())
        .cloned()
        .collect()
}
#[test]
fn all_authored_flow_policies_are_consistent_across_full_partial_and_far_seek_queries() {
    for (kind, policy) in policies() {
        let ir = authored(kind, policy.clone());
        let serialized = serde_json::to_vec(&ir).unwrap();
        let restored: PatternIr = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(ir, restored);
        for output in ir.outputs {
            let broad = output.root.query(span(0, 1, 8, 1));
            for window in [
                span(1, 4, 1, 4),
                span(5, 4, 1, 4),
                span(13, 4, 1, 4),
                span(21, 4, 1, 4),
            ] {
                assert_eq!(
                    output.root.query(window).events,
                    visible(&broad.events, window),
                    "{policy:?}: {window:?}"
                );
            }
            let far = output.root.query(span(30, 1, 4, 1));
            let window = span(127, 4, 1, 4);
            assert_eq!(
                output.root.query(window).events,
                visible(&far.events, window),
                "far {policy:?}"
            );
        }
    }
}
fn notes(node: &PatternNodeIr, cycle: i64) -> Vec<String> {
    node.query(span(cycle, 1, 1, 1))
        .events
        .into_iter()
        .filter_map(|e| match e.value {
            EventValue::Note { value, .. } => Some(value),
            _ => None,
        })
        .collect()
}
#[test]
fn alternating_inputs_and_control_values_reach_the_selected_cycle() {
    use FlowControlKind as K;
    use FlowControlPolicy as P;
    for (kind, policy) in [
        (K::Switch, P::SwitchCycleIndex),
        (K::Choice, P::ChoiceCycle),
        (K::Switch, P::SwitchControlValue),
        (K::Choice, P::ChoiceWeighted),
    ] {
        let ir = authored(kind, policy.clone());
        let root = &ir.outputs[0].root;
        assert_ne!(
            notes(root, 0),
            notes(root, 1),
            "{policy:?} was frozen at compilation"
        );
        assert_eq!(notes(root, 30), notes(root, 0));
        assert_eq!(notes(root, 31), notes(root, 1));
    }
    for policy in [P::SwitchSeededRandom, P::ChoiceSeededRandom] {
        let kind = if policy == P::SwitchSeededRandom {
            K::Switch
        } else {
            K::Choice
        };
        let ir = authored(kind, policy);
        let choices = (0..24)
            .map(|cycle| notes(&ir.outputs[0].root, cycle))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            choices.len() > 2,
            "random choices must change with the cycle and remain repeatable"
        );
    }
    let ir = authored(K::Route, P::RouteByControlValue);
    assert!(!notes(&ir.outputs[0].root, 0).is_empty());
    assert!(
        ir.outputs[0]
            .root
            .query(span(1, 1, 1, 10))
            .events
            .iter()
            .all(|e| e.span.start.0 < Rational::one()),
        "cycle-one new onsets belong to the second branch"
    );
    assert!(!notes(&ir.outputs[1].root, 1).is_empty());
}
#[test]
fn field_blends_masks_and_splits_retain_fast_and_held_inputs() {
    use FlowControlKind as K;
    use FlowControlPolicy as P;
    for (kind, policy) in [
        (K::Mix, P::MixWeighted),
        (K::Mask, P::MaskScale),
        (K::Mask, P::MaskGate),
        (K::Mask, P::MaskInvertGate),
        (K::Split, P::SplitByIndexModulo),
        (K::Route, P::RouteByIndexModulo),
    ] {
        let ir = authored(kind, policy.clone());
        for output in ir.outputs {
            let fast = PatternNodeIr::time_scale(output.root.clone(), Rational::new(1, 3));
            let all = fast.query(span(0, 1, 4, 1));
            let window = span(13, 12, 1, 12);
            assert_eq!(
                fast.query(window).events,
                visible(&all.events, window),
                "Fast {policy:?}"
            );
        }
    }
    let ir = authored(K::Mask, P::MaskScale);
    let root = &ir.outputs[0].root;
    for cycle in [0, 1, 30, 31] {
        let events = root.query(span(cycle, 1, 1, 1)).events;
        let own = events
            .iter()
            .filter(|e| e.span.start.0 >= Rational::from_integer(cycle));
        for e in own {
            assert!(e.fields.contains(&EventField::Gain(FieldValue::rational(
                Rational::from_integer(cycle % 2)
            ))));
        }
    }
}

#[test]
fn whole_held_notes_keep_onset_routing_when_a_seek_starts_mid_note() {
    let leaf = |note: &str| {
        PatternNodeIr::cycle_event_stream(PatternStream::new(vec![PatternEvent::new(
            span(0, 1, 5, 2),
            EventValue::Note {
                value: note.into(),
                octave: Some(4),
            },
        )]))
    };
    for policy in [
        FlowControlPolicy::SwitchCycleIndex,
        FlowControlPolicy::SwitchSeededRandom,
    ] {
        let node = PatternNodeIr::FlowProjection {
            control: FlowControlNode::new(FlowControlKind::Switch).with_policy(policy.clone()),
            inputs: vec![
                FlowInputIr {
                    endpoint: input("candidates", "a"),
                    nodes: vec![leaf("c")],
                },
                FlowInputIr {
                    endpoint: input("candidates", "b"),
                    nodes: vec![leaf("g")],
                },
            ],
            output: OutputEndpoint::Socket(OutputPort::new("out")),
        };
        let broad = node.query(span(0, 1, 8, 1));
        for window in [span(7, 4, 1, 4), span(15, 4, 1, 4), span(23, 4, 1, 4)] {
            let partial = node.query(window);
            assert!(
                partial
                    .events
                    .iter()
                    .any(|event| event.span.start < window.start),
                "a real held note is present"
            );
            assert_eq!(
                partial.events,
                visible(&broad.events, window),
                "held {policy:?}"
            );
        }
        let far = node.query(span(30, 1, 4, 1));
        let window = span(127, 4, 1, 4);
        assert_eq!(node.query(window).events, visible(&far.events, window));
    }
}

#[test]
fn merge_priority_keeps_priority_instead_of_silently_layering() {
    let ir = authored(FlowControlKind::Merge, FlowControlPolicy::MergePriority);
    assert_eq!(notes(&ir.outputs[0].root, 0), vec!["c"]);
    assert_eq!(notes(&ir.outputs[0].root, 1), vec!["d"]);
}

#[test]
fn custom_flow_output_shape_and_recursive_operands_survive_mapping() {
    let mut control = FlowControlNode::new(FlowControlKind::Switch);
    control.signature.output_sockets[0].shape = StreamShape::ScalarPattern;
    control.signature.input_groups[0].shape = StreamShape::ScalarPattern;
    let scalar = |value| {
        PatternNodeIr::scalar_stream(ScalarStream::new(vec![ScalarEvent::new(
            span(0, 1, 1, 1),
            Rational::from_integer(value),
        )]))
    };
    let node = PatternNodeIr::FlowProjection {
        control,
        inputs: vec![
            FlowInputIr {
                endpoint: input("candidates", "a"),
                nodes: vec![scalar(2)],
            },
            FlowInputIr {
                endpoint: input("candidates", "b"),
                nodes: vec![scalar(3)],
            },
        ],
        output: OutputEndpoint::Socket(OutputPort::new("out")),
    };
    assert_eq!(node.shape(), PatternStreamShape::Scalar);
    assert_eq!(node.duration(), CycleDuration(Rational::one()));
    let mapped = node.map_event_leaves(&mut |stream, periodic| {
        let mut stream = stream.clone();
        for event in &mut stream.events {
            if let EventValue::Scalar { value } = &mut event.value {
                *value = *value * Rational::from_integer(2);
            }
        }
        if periodic {
            PatternNodeIr::cycle_event_stream(stream)
        } else {
            PatternNodeIr::event_stream(stream)
        }
    });
    assert_eq!(mapped.shape(), PatternStreamShape::Scalar);
    assert_eq!(
        mapped.query(span(30, 1, 1, 1)).events[0].value,
        EventValue::Scalar {
            value: Rational::from_integer(4)
        }
    );
    assert_eq!(
        mapped.query(span(31, 1, 1, 1)).events[0].value,
        EventValue::Scalar {
            value: Rational::from_integer(6)
        }
    );
}
