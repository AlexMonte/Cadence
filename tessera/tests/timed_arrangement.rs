use tessera::prelude::*;

fn r(value: i64) -> Rational {
    Rational::from_integer(value)
}
fn span(start: Rational, duration: Rational) -> CycleSpan {
    CycleSpan::new(CycleTime(start), CycleDuration(duration))
}
fn note(label: &str, gain: Rational) -> PatternNodeIr {
    let mut event = PatternEvent::new(
        span(r(0), r(1)),
        EventValue::Note {
            value: label.into(),
            octave: Some(4),
        },
    );
    event
        .fields
        .push(EventField::Gain(FieldValue::rational(gain)));
    PatternNodeIr::cycle_event_stream(PatternStream::new(vec![event]))
}
fn alternate() -> PatternNodeIr {
    PatternNodeIr::cycle_route(vec![
        note("c", Rational::new(1, 4)),
        note("d", Rational::new(3, 4)),
    ])
}
fn labels(node: &PatternNodeIr, start: i64, duration: i64) -> Vec<String> {
    node.query(span(r(start), r(duration)))
        .events
        .iter()
        .map(|event| match &event.value {
            EventValue::Note { value, .. } => value.clone(),
            _ => panic!("note expected"),
        })
        .collect()
}

#[test]
fn odd_duration_repeats_reset_alternate_at_each_occurrence_and_whole_loop() {
    let arranged = PatternNodeIr::arrange(vec![
        TimedPatternIr::new(r(3), 2, alternate()),
        TimedPatternIr::new(r(1), 1, note("e", r(1))),
    ]);
    assert_eq!(arranged.duration().0, r(7));
    let expected = ["c", "d", "c", "c", "d", "c", "e"];
    for cycle in 0..28 {
        let events = arranged.query(span(r(cycle), r(1))).events;
        assert_eq!(events.len(), 1);
        let EventValue::Note { value, .. } = &events[0].value else {
            panic!("note")
        };
        assert_eq!(value, expected[cycle as usize % expected.len()]);
        let gain = match value.as_str() {
            "c" => Rational::new(1, 4),
            "d" => Rational::new(3, 4),
            _ => r(1),
        };
        assert!(
            events[0]
                .fields
                .contains(&EventField::Gain(FieldValue::rational(gain)))
        );
        assert_eq!(events[0].span, span(r(cycle), r(1)));
    }
    let encoded = serde_json::to_string(&arranged).unwrap();
    let decoded: PatternNodeIr = serde_json::from_str(&encoded).unwrap();
    assert_eq!(labels(&decoded, 0, 28), labels(&arranged, 0, 28));
    assert_eq!(
        arranged.flatten().events.len(),
        7,
        "bounded flatten covers one arrangement period"
    );
}

#[test]
fn occurrence_bounds_clip_held_patterns_without_stretching_or_losing_seek_identity() {
    let arranged = PatternNodeIr::arrange(vec![TimedPatternIr::new(
        r(3),
        2,
        PatternNodeIr::time_scale(alternate(), r(2)),
    )]);
    let whole = arranged.query(span(r(0), r(6))).events;
    assert_eq!(
        whole.iter().map(|event| event.span).collect::<Vec<_>>(),
        vec![
            span(r(0), r(2)),
            span(r(2), r(1)),
            span(r(3), r(2)),
            span(r(5), r(1))
        ]
    );
    for quarter in 0..24 {
        let query = span(Rational::new(quarter, 4), Rational::new(1, 4));
        let visible = whole
            .iter()
            .filter(|event| event.span.start < query.end() && query.start < event.span.end())
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            arranged.query(query).events,
            visible,
            "partial seeks must recover the same clipped whole event at quarter {quarter}"
        );
    }
}

#[test]
fn nested_arrangements_and_field_mapping_preserve_occurrence_local_clocks() {
    let inner = PatternNodeIr::arrange(vec![TimedPatternIr::new(r(3), 1, alternate())]);
    let outer = PatternNodeIr::arrange(vec![TimedPatternIr::new(r(5), 2, inner)]);
    assert_eq!(
        labels(&outer, 0, 10),
        ["c", "d", "c", "c", "d", "c", "d", "c", "c", "d"]
    );
    let mapped = outer.map_event_leaves(&mut |stream, periodic| {
        let mut stream = stream.clone();
        for event in &mut stream.events {
            event.fields.push(EventField::Fit(FieldValue::bool(true)));
        }
        if periodic {
            PatternNodeIr::cycle_event_stream(stream)
        } else {
            PatternNodeIr::event_stream(stream)
        }
    });
    assert_eq!(mapped.duration(), outer.duration());
    for cycle in 0..20 {
        let events = mapped.query(span(r(cycle), r(1))).events;
        assert!(events.iter().all(|event| {
            event
                .fields
                .contains(&EventField::Fit(FieldValue::bool(true)))
        }));
        assert_eq!(labels(&mapped, cycle, 1), labels(&outer, cycle, 1));
    }
}

#[test]
fn bounded_query_skips_billions_of_repeats_and_rejects_invalid_serialized_timing() {
    let valid = TimedPatternIr::new(r(1), u32::MAX, note("c", r(1)));
    assert!(valid.validate().is_ok());
    let overflow = vec![
        TimedPatternIr::new(r(i64::MAX), 1, note("c", r(1))),
        TimedPatternIr::new(r(1), 1, note("d", r(1))),
    ];
    assert!(TimedPatternIr::total_duration(&overflow).is_err());
    assert!(
        PatternNodeIr::arrange(overflow)
            .query(span(r(0), r(1)))
            .events
            .is_empty()
    );
    let arranged = PatternNodeIr::arrange(vec![valid]);
    assert_eq!(labels(&arranged, i64::from(u32::MAX) - 1, 1), ["c"]);
    for (duration, repeats) in [
        (r(0), 1),
        (r(-1), 1),
        (r(1), 0),
        (
            Rational {
                numerator: 1,
                denominator: 0,
            },
            1,
        ),
        (r(i64::MAX), 2),
    ] {
        let segment = TimedPatternIr::new(duration, repeats, note("c", r(1)));
        assert!(segment.validate().is_err());
        let encoded = serde_json::to_string(&PatternNodeIr::arrange(vec![segment])).unwrap();
        let malformed: PatternNodeIr = serde_json::from_str(&encoded).unwrap();
        assert!(malformed.query(span(r(0), r(1))).events.is_empty());
    }
}

#[test]
fn fractional_occurrences_loop_exactly_and_preserve_parallel_unisons() {
    let unison = PatternNodeIr::merge(vec![note("c", r(1)), note("c", r(1))]);
    let arranged =
        PatternNodeIr::arrange(vec![TimedPatternIr::new(Rational::new(3, 2), 2, unison)]);
    assert_eq!(arranged.duration().0, r(3));
    let stream = arranged.query(span(r(0), r(3)));
    assert_eq!(
        stream.events.len(),
        8,
        "each unison voice must survive each occurrence"
    );
    let starts = stream
        .events
        .iter()
        .map(|event| event.span.start.0)
        .collect::<Vec<_>>();
    assert_eq!(
        starts,
        [
            r(0),
            r(0),
            r(1),
            r(1),
            Rational::new(3, 2),
            Rational::new(3, 2),
            Rational::new(5, 2),
            Rational::new(5, 2)
        ]
    );
}

#[test]
fn scaling_and_concatenating_arrangements_keep_allocated_silent_tail() {
    let arranged = PatternNodeIr::arrange(vec![
        TimedPatternIr::new(r(1), 1, note("c", r(1))),
        TimedPatternIr::new(
            r(2),
            1,
            PatternNodeIr::cycle_event_stream(PatternStream::default()),
        ),
    ]);
    let slow = PatternNodeIr::time_scale(arranged, r(2));
    assert_eq!(slow.duration().0, r(6));
    let joined = PatternNodeIr::concat(vec![slow, note("d", r(1))]);
    assert_eq!(joined.duration().0, r(7));
    assert_eq!(labels(&joined, 0, 7), ["c", "d"]);
    assert!(joined.query(span(r(5), r(1))).events.is_empty());
    assert_eq!(
        joined.query(span(r(6), r(1))).events[0].span,
        span(r(6), r(1))
    );
}
