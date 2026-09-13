//! Cyclic source phase is defined on both sides of zero; finite playback is not.
use cadence::prelude::*;

fn span(start: Time, end: Time) -> TransportSpan {
    TransportSpan::new(start, end).unwrap()
}
fn voice(period: Time, repeat: Repeat) -> Score {
    Voice::new(
        period,
        vec![
            Tile::spanning(Time::ZERO, period, Intent::sample("tone"))
                .unwrap()
                .with_id(17u64),
        ],
    )
    .unwrap()
    .with_repeat(repeat)
    .into()
}
fn controls(period: Time, repeat: Repeat) -> ControlScore {
    ControlTrack::new(
        period,
        vec![
            ControlTile::spanning(
                Time::ZERO,
                period,
                ControlKey::Gain,
                ControlValue::Scalar(0.25),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_repeat(repeat)
    .into()
}
fn query(score: &Score, window: TransportSpan) -> PreviewReport {
    let prepared = PreparedScore::new(score.clone()).unwrap();
    CadenceCompiler::new().preview(&prepared, &window).unwrap()
}
fn lifecycle(event: &EvaluatedEvent) -> (i64, TransportSpan) {
    match event.kind() {
        EvaluatedEventKind::StartVoice {
            voice_whole,
            voice_id,
        }
        | EvaluatedEventKind::UpdateVoiceControls {
            voice_whole,
            voice_id,
        } => (voice_id.value(), *voice_whole),
    }
}
fn snapshot(
    report: &PreviewReport,
    offset: Time,
) -> Vec<(TransportSpan, TransportSpan, ControlMap)> {
    report
        .events
        .iter()
        .map(|event| {
            let event = event.projected();
            (
                event.whole().translate(Time::ZERO - offset),
                event.visible().translate(Time::ZERO - offset),
                event.controls().clone(),
            )
        })
        .collect()
}

#[test]
fn negative_query_keeps_whole_spans_controls_and_stable_partial_query_identity() {
    let score = Score::with_controls(
        voice(Time::ONE, Repeat::Forever),
        controls(Time::ONE, Repeat::Forever),
    );
    let whole_window = span(Time::new(-5, 4), Time::new(1, 4));
    let full = query(&score, whole_window);
    let expected = [
        span(Time::whole_number(-2), Time::whole_number(-1)),
        span(Time::whole_number(-1), Time::ZERO),
        span(Time::ZERO, Time::ONE),
    ];
    assert_eq!(
        full.events
            .iter()
            .map(|event| event.projected().whole())
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        query(&voice(Time::ONE, Repeat::Forever), whole_window)
            .starts()
            .count(),
        3
    );
    for event in &full.events {
        assert_eq!(
            event.projected().controls().get(&ControlKey::Gain),
            Some(&ControlValue::Scalar(0.25))
        );
        assert_eq!(event.projected().id(), Some(MomentId::new(17)));
    }
    for quarter in -5..1 {
        let window = span(Time::new(quarter, 4), Time::new(quarter + 1, 4));
        let partial = query(&score, window);
        assert_eq!(partial.events.len(), 1);
        let actual = &partial.events[0];
        let original = full
            .events
            .iter()
            .find(|event| lifecycle(event) == lifecycle(actual))
            .expect("direct negative seeks retain lifecycle identity");
        assert_eq!(actual.projected().whole(), original.projected().whole());
        assert_eq!(actual.projected().visible(), window);
    }
}

#[test]
fn cyclic_notes_and_controls_are_periodic_across_negative_fractional_boundaries() {
    // Check translation invariance over a bounded exhaustive grid, without a
    // new test dependency. Rational periods exercise mathematical floor at
    // negative fractional boundaries, not truncation toward zero.
    for period in [
        Time::new(1, 2),
        Time::ONE,
        Time::new(3, 2),
        Time::whole_number(2),
    ] {
        let score = Score::with_controls(
            voice(period, Repeat::Forever),
            controls(period, Repeat::Forever),
        );
        let base_window = span(period * Time::new(1, 4), period * Time::new(9, 4));
        let baseline = snapshot(&query(&score, base_window), Time::ZERO);
        assert_eq!(baseline.len(), 3);
        for repetition in -16..=16 {
            let offset = period * Time::whole_number(repetition);
            assert_eq!(
                snapshot(&query(&score, base_window.translate(offset)), offset),
                baseline,
                "period={period:?}, repetition={repetition}"
            );
        }
    }
}

#[test]
fn shifting_a_cycle_preserves_the_previous_occurrence_at_transport_zero() {
    let source = Score::with_controls(
        voice(Time::ONE, Repeat::Forever),
        controls(Time::ONE, Repeat::Forever),
    );
    let offset = Time::new(1, 4);
    let score = Score::shift(source.clone(), offset);
    let window = span(Time::ZERO, offset);
    let report = query(&score, window);
    let start = report
        .events
        .first()
        .expect("shifted periodic audio continues through cycle zero");
    assert_eq!(start.projected().whole(), span(Time::new(-3, 4), offset));
    assert_eq!(start.projected().visible(), window);
    assert_eq!(
        start.projected().controls().get(&ControlKey::Gain),
        Some(&ControlValue::Scalar(0.25))
    );
    assert_eq!(
        snapshot(&report, offset),
        snapshot(
            &query(&source, window.translate(Time::ZERO - offset)),
            Time::ZERO
        )
    );
    let finite = Score::shift(voice(Time::ONE, Repeat::Once), offset);
    assert_eq!(query(&finite, window).starts().count(), 0);
}

#[test]
fn finite_repetition_policies_still_begin_at_zero_and_keep_their_limits() {
    for (repeat, expected) in [
        (Repeat::Once, vec![span(Time::ZERO, Time::ONE)]),
        (Repeat::Count(0), vec![]),
        (
            Repeat::Count(2),
            vec![
                span(Time::ZERO, Time::ONE),
                span(Time::ONE, Time::whole_number(2)),
            ],
        ),
        (
            Repeat::Until(Time::new(3, 2)),
            vec![
                span(Time::ZERO, Time::ONE),
                span(Time::ONE, Time::new(3, 2)),
            ],
        ),
        (Repeat::Until(Time::ZERO), vec![]),
        (Repeat::Until(Time::whole_number(-1)), vec![]),
    ] {
        let window = span(Time::whole_number(-2), Time::whole_number(3));
        let report = query(&voice(Time::ONE, repeat), window);
        assert_eq!(
            report
                .starts()
                .map(|event| event.projected().whole())
                .collect::<Vec<_>>(),
            expected,
            "{repeat:?}"
        );
        let controlled = Score::with_controls(
            voice(Time::ONE, Repeat::Forever),
            controls(Time::ONE, repeat),
        );
        let before = query(&controlled, span(Time::whole_number(-2), Time::ZERO));
        assert_eq!(before.starts().count(), 2);
        assert!(
            before
                .starts()
                .all(|event| !event.projected().controls().contains_key(&ControlKey::Gain)),
            "finite control track must not repeat backwards: {repeat:?}"
        );
    }
}
