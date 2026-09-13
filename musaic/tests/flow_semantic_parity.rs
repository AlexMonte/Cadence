//! The language's bounded preview and the audio engine must choose the same music.
use cadence::prelude::{
    BuiltInSynthSource, CadenceCompiler, ControlKey, ControlValue, Intent, PreparedScore, Span,
    Time,
};
use musaic::application::pipeline::lowering::lower_tessera_ir_with_sounds;
use std::collections::BTreeMap;
use tessera::prelude::*;

fn r(n: i64, d: i64) -> Rational {
    Rational::new(n, d)
}
fn note(name: &str, duration: Rational, gain: Rational) -> PatternNodeIr {
    PatternNodeIr::cycle_event_stream(PatternStream::new(vec![
        PatternEvent::new(
            CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(duration)),
            EventValue::Note {
                value: name.into(),
                octave: Some(4),
            },
        )
        .with_fields(vec![EventField::Gain(FieldValue::rational(gain))]),
    ]))
}
type Snapshot = Vec<(Rational, Rational, EventValue, Rational)>;
fn compare(node: PatternNodeIr, start: Rational, duration: Rational) -> Snapshot {
    let language: Snapshot = node
        .query(CycleSpan::new(CycleTime(start), CycleDuration(duration)))
        .events
        .into_iter()
        .map(|event| {
            let gain = event
                .fields
                .iter()
                .find_map(|field| match field {
                    EventField::Gain(FieldValue::Rational { value }) => Some(*value),
                    _ => None,
                })
                .unwrap_or(Rational::one());
            (event.span.start.0, event.span.end().0, event.value, gain)
        })
        .collect();
    let output = NodeId::new("out");
    let (scores, diagnostics) = lower_tessera_ir_with_sounds(
        &PatternIr::new(vec![PatternOutput::new(output.clone(), node)]),
        &BTreeMap::from([(output.clone(), Intent::synth(BuiltInSynthSource::Sine))]),
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let end = start + duration;
    let prepared = PreparedScore::new(scores[&output].clone()).unwrap();
    let report = CadenceCompiler::new()
        .preview(
            &prepared,
            &Span::new(
                Time::new(start.numerator, start.denominator),
                Time::new(end.numerator, end.denominator),
            )
            .unwrap(),
        )
        .unwrap();
    let mut lifecycles = std::collections::BTreeSet::new();
    let audio: Snapshot = report
        .events
        .iter()
        .filter(|event| {
            let id = match event.kind() {
                cadence::application::EvaluatedEventKind::StartVoice { voice_id, .. }
                | cadence::application::EvaluatedEventKind::UpdateVoiceControls {
                    voice_id, ..
                } => voice_id.value(),
            };
            lifecycles.insert(id)
        })
        .map(|event| {
            let event = event.projected();
            let span = event.whole();
            let value = serde_json::from_str(
                event
                    .as_moment()
                    .value_identity()
                    .expect("authored value survives all flow transforms")
                    .as_str(),
            )
            .unwrap();
            let gain = match event.controls().get(&ControlKey::Gain) {
                Some(ControlValue::Scalar(gain)) => {
                    Rational::new((gain * 1_000_000.).round() as i64, 1_000_000)
                }
                _ => Rational::one(),
            };
            (
                Rational::new(span.start().numerator(), span.start().denominator()),
                Rational::new(span.end().numerator(), span.end().denominator()),
                value,
                gain,
            )
        })
        .collect();
    assert_eq!(audio, language);
    language
}

#[test]
fn duplicate_policy_preserves_distinct_pitches_and_honors_winner_after_source_assignment() {
    for key in [
        DeduplicateKeyIr::WholeSpanAndValue,
        DeduplicateKeyIr::StartAndValue,
    ] {
        for winner in [DeduplicateWinnerIr::First, DeduplicateWinnerIr::Last] {
            let result = compare(
                PatternNodeIr::deduplicate(
                    PatternNodeIr::merge(vec![
                        note("c", r(1, 1), r(1, 4)),
                        note("e", r(1, 1), r(1, 2)),
                        note("c", r(1, 1), r(3, 4)),
                    ]),
                    DeduplicatePolicyIr { key, winner },
                ),
                r(7, 1),
                r(1, 1),
            );
            assert_eq!(result.len(), 2);
            let c = result
                .iter()
                .find(|(_, _, value, _)| matches!(value,EventValue::Note{value,..}if value=="c"))
                .unwrap();
            assert_eq!(
                c.3,
                if winner == DeduplicateWinnerIr::First {
                    r(1, 4)
                } else {
                    r(3, 4)
                }
            );
        }
    }
}

#[test]
fn start_and_whole_span_policies_have_distinct_meanings() {
    for (key, count) in [
        (DeduplicateKeyIr::WholeSpanAndValue, 2),
        (DeduplicateKeyIr::StartAndValue, 1),
    ] {
        let result = compare(
            PatternNodeIr::deduplicate(
                PatternNodeIr::merge(vec![
                    note("c", r(1, 2), r(1, 4)),
                    note("c", r(1, 1), r(3, 4)),
                ]),
                DeduplicatePolicyIr {
                    key,
                    winner: DeduplicateWinnerIr::Last,
                },
            ),
            r(0, 1),
            r(1, 1),
        );
        assert_eq!(result.len(), count);
    }
}

#[test]
fn priority_merge_uses_the_authored_value_and_the_requested_conflict_rule() {
    for (conflict, count) in [
        (PriorityConflictIr::SameWholeStartAndValue, 2),
        (PriorityConflictIr::SameWholeSpanAndValue, 3),
        (PriorityConflictIr::WholeSpanOverlap, 1),
    ] {
        let result = compare(
            PatternNodeIr::priority_merge(
                vec![
                    note("c", r(1, 2), r(1, 4)),
                    note("e", r(1, 1), r(1, 2)),
                    note("c", r(1, 1), r(3, 4)),
                ],
                PriorityMergePolicyIr { conflict },
            ),
            r(-2, 1),
            r(1, 1),
        );
        assert_eq!(result.len(), count);
    }
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(64))]
    #[test]
    fn seeded_choice_matches_across_negative_seek_and_nested_timing(seed in proptest::prelude::any::<u64>(),cycle in -24i64..24, first_weight in 1i64..10, second_weight in 1i64..10){
        let choice=PatternNodeIr::weighted_choice(vec![WeightedPatternIr::new(r(first_weight,1),note("c",r(1,1),r(1,4))),WeightedPatternIr::new(r(second_weight,1),note("g",r(1,1),r(3,4)))],seed);
        compare(choice.clone(),r(cycle*3+1,3),r(7,3));
        compare(PatternNodeIr::shift(PatternNodeIr::time_scale(choice,r(2,3)),CycleDuration(r(1,5))),r(cycle,1),r(2,1));
    }
    #[test]
    fn seeded_degrade_matches_after_recompilation_and_negative_seek(seed in proptest::prelude::any::<u64>(),cycle in -24i64..24, probability in 0i64..101){
        let notes=PatternNodeIr::cycle_slots(vec![note("c",r(1,1),r(1,4)),note("e",r(1,1),r(1,2)),note("g",r(1,1),r(3,4))]);
        let node=PatternNodeIr::degrade(notes,r(probability,100),seed);
        let first=compare(node.clone(),r(cycle*3+1,3),r(7,3));
        let second=compare(node,r(cycle*3+1,3),r(7,3));
        proptest::prop_assert_eq!(first,second);
    }
}
