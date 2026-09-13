//! Synthetic host sources exercise the public query and audio planning seams.
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame, SampleBuffer},
    infrastructure::playback::SampleBank,
    prelude::*,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, Clone)]
struct Events {
    rows: Vec<QueryMoment>,
    calls: Arc<AtomicUsize>,
    work: f64,
    extent: Time,
}
impl ScoreQuerySource for Events {
    fn query(&self, window: &TransportSpan) -> Result<Vec<QueryMoment>, ControlModelError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .rows
            .iter()
            .filter(|row| row.moment.span().intersects(window))
            .cloned()
            .collect())
    }
    fn estimated_work(&self, _: f64) -> f64 {
        self.work
    }
    fn extent(&self) -> Time {
        self.extent
    }
}
#[derive(Debug, Clone)]
struct Controls {
    rows: Vec<QueryControl>,
    calls: Arc<AtomicUsize>,
    work: f64,
    extent: Time,
}
impl ControlQuerySource for Controls {
    fn query(&self, window: &TransportSpan) -> Result<Vec<QueryControl>, ControlModelError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .rows
            .iter()
            .filter(|row| row.span.intersects(window))
            .cloned()
            .collect())
    }
    fn estimated_work(&self, _: f64) -> f64 {
        self.work
    }
    fn extent(&self) -> Time {
        self.extent
    }
    fn contains_key(&self, key: &ControlKey) -> bool {
        self.rows.iter().any(|row| &row.key == key)
    }
}
fn span(start: Time, end: Time) -> TransportSpan {
    TransportSpan::new(start, end).unwrap()
}
fn events(sample: bool, end: Time) -> Events {
    Events {
        rows: vec![QueryMoment {
            moment: Moment::new(
                span(Time::ZERO, end),
                if sample {
                    Intent::sample("tone")
                } else {
                    Intent::synth(BuiltInSynthSource::Sine)
                },
            )
            .with_id(71u64)
            .with_value_identity("a4"),
            instance_key: 900,
            controls: vec![
                (ControlKey::Pitch, ControlValue::Scalar(69.0)),
                (ControlKey::Gain, ControlValue::Scalar(0.5)),
                (ControlKey::Gain, ControlValue::Scalar(0.5)),
            ],
        }],
        calls: Arc::default(),
        work: 1.0,
        extent: end,
    }
}
fn query(score: &Score, window: TransportSpan) -> Result<PreviewReport, ControlModelError> {
    let prepared = PreparedScore::new(score.clone()).unwrap();
    CadenceCompiler::new().preview(&prepared, &window)
}

#[test]
fn held_note_recovery_is_bounded_before_widened_control_queries_allocate() {
    // The enclosing slow clock makes the normal lookahead cheap. An interior
    // seek nevertheless needs controls at each note's original source onset.
    // Cover both one excessive recovery and excessive combined recovery work.
    for (control_rate, held_notes) in [(20_000, 1), (200, 100)] {
        let calls = Arc::new(AtomicUsize::new(0));
        let observer = ControlScore::query_source(Controls {
            rows: Vec::new(),
            calls: calls.clone(),
            work: 1.0,
            extent: Time::ONE,
        });
        let dense = ControlScore::time_scale(
            ControlScore::track(
                ControlTrack::new(
                    Time::ONE,
                    vec![
                        ControlTile::spanning(
                            Time::ZERO,
                            Time::ONE,
                            ControlKey::Velocity,
                            ControlValue::Scalar(0.5),
                        )
                        .unwrap(),
                    ],
                )
                .unwrap(),
            ),
            Time::whole_number(control_rate),
        );
        let notes = Score::voice(
            Voice::new(
                Time::ONE,
                vec![
                    Tile::spanning(
                        Time::ZERO,
                        Time::ONE,
                        Intent::synth(BuiltInSynthSource::Sine),
                    )
                    .unwrap();
                    held_notes
                ],
            )
            .unwrap(),
        );
        let score = Score::time_scale(
            Score::with_controls(notes, ControlScore::merge(vec![observer, dense])),
            Time::new(1, 1000),
        );
        PreparedScore::new(score.clone()).unwrap();
        let error = query(
            &score,
            span(Time::whole_number(500), Time::whole_number(501)),
        )
        .expect_err("widened onset queries must be checked before their allocations");
        assert!(matches!(error, ControlModelError::QuerySource(message)
            if message.contains("held-note onset control recovery")));
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "only the small visible control query may run; recovery must be rejected first"
        );
    }
}

fn lifecycle(event: &EvaluatedEvent) -> (i64, TransportSpan) {
    match event.kind() {
        EvaluatedEventKind::StartVoice {
            voice_id,
            voice_whole,
        }
        | EvaluatedEventKind::UpdateVoiceControls {
            voice_id,
            voice_whole,
        } => (voice_id.value(), *voice_whole),
    }
}

#[test]
fn clone_and_arbitrary_query_order_preserve_held_event_identity_and_ordered_controls() {
    for sample in [false, true] {
        let source = events(sample, Time::whole_number(2));
        let calls = source.calls.clone();
        let score = Score::query_source(source);
        let cloned = score.clone();
        assert_eq!(score, cloned);
        let full = query(&score, span(Time::ZERO, Time::whole_number(2))).unwrap();
        let expected = full.starts().next().unwrap();
        assert_eq!(
            expected.projected().controls()[&ControlKey::Gain],
            ControlValue::Scalar(0.25)
        );
        assert_eq!(
            expected.projected().controls()[&ControlKey::Pitch],
            ControlValue::Scalar(69.0)
        );
        assert_eq!(expected.projected().id(), Some(MomentId::new(71)));
        for quarter in [5, 1, 7, 3, 0, 6, 2, 4] {
            let window = span(Time::new(quarter, 4), Time::new(quarter + 1, 4));
            let partial = query(&cloned, window).unwrap();
            assert_eq!(partial.events.len(), 1);
            let actual = &partial.events[0];
            assert_eq!(lifecycle(actual), lifecycle(expected));
            assert_eq!(actual.projected().visible(), window);
            assert_eq!(
                actual.projected().controls(),
                expected.projected().controls()
            );
        }
        assert!(calls.load(Ordering::SeqCst) >= 9);
        assert!(
            query(&score, span(Time::whole_number(-1), Time::ZERO))
                .unwrap()
                .events
                .is_empty()
        );
        assert!(
            query(&score, span(Time::whole_number(2), Time::whole_number(3)))
                .unwrap()
                .events
                .is_empty()
        );
    }
}

fn audio_setup(score: Score) -> (PlaybackRuntime, AudioRenderer) {
    let bank = SampleBank::new();
    bank.load(
        "tone",
        SampleBuffer::new(
            8_000,
            (0..16_000)
                .map(|n| Frame::from_mono((n % 113) as f32 / 200.0 - 0.28))
                .collect::<Vec<_>>(),
        ),
    );
    let (audio, renderer) = AudioRenderer::split(AudioRendererSettings::new(8_000, 256)).unwrap();
    let mut runtime = PlaybackRuntime::with_sample_bank(
        PlaybackSettings {
            cps: Time::ONE,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 64),
        },
        audio,
        bank,
    );
    runtime
        .play_prepared_score(PreparedScore::new(score).unwrap())
        .unwrap();
    (runtime, renderer)
}
fn render(
    score: Score,
    calls: &AtomicUsize,
    start: Time,
    frames: usize,
    block: usize,
) -> Vec<Frame> {
    let (mut runtime, mut renderer) = audio_setup(score);
    if start != Time::ZERO {
        runtime.seek(start).unwrap();
    }
    let mut result = vec![Frame::ZERO; frames];
    for chunk in result.chunks_mut(block) {
        runtime.tick().unwrap();
        let before = calls.load(Ordering::SeqCst);
        renderer.render(chunk);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            before,
            "the audio callback must never query a host source"
        );
    }
    result
}
#[test]
fn held_sample_and_synth_sound_identical_across_callbacks_and_direct_seek() {
    for sample in [false, true] {
        let source = events(sample, Time::whole_number(2));
        let calls = source.calls.clone();
        let score = Score::query_source(source);
        let whole = render(score.clone(), &calls, Time::ZERO, 8_000, 127);
        assert!(whole.iter().any(|frame| frame.left != 0.0));
        assert!(
            whole == render(score.clone(), &calls, Time::ZERO, 8_000, 64),
            "callback sizes must not change PCM"
        );
        let seek = render(score, &calls, Time::new(4_003, 8_000), 2_000, 97);
        let maximum_error = seek
            .iter()
            .zip(&whole[4_003..6_003])
            .map(|(actual, expected)| {
                (actual.left - expected.left)
                    .abs()
                    .max((actual.right - expected.right).abs())
            })
            .fold(0.0_f32, f32::max);
        // Sine phase can differ by floating-point rounding after reconstruction.
        assert!(
            maximum_error < 1e-6,
            "held source phase must survive direct seek; sample={sample}, maximum_error={maximum_error}"
        );
    }
}

#[test]
fn control_sources_release_their_runtime_lane_in_gaps_and_preserve_source_controls() {
    let controls = Controls {
        rows: vec![QueryControl {
            span: span(Time::ZERO, Time::new(1, 2)),
            key: ControlKey::Transpose,
            value: ControlValue::Scalar(12.0),
        }],
        calls: Arc::default(),
        work: 1.0,
        extent: Time::whole_number(2),
    };
    let lane = ControlScore::query_source(controls);
    assert_eq!(lane, lane.clone());
    let score = Score::with_controls(
        Score::query_source(events(false, Time::whole_number(2))),
        lane,
    );
    let full = query(&score, span(Time::ZERO, Time::ONE)).unwrap();
    assert_eq!(full.starts().count(), 1);
    assert_eq!(full.control_updates().count(), 1);
    for (start, transpose) in [(Time::new(1, 4), 12.0), (Time::new(3, 4), 0.0)] {
        let partial = query(&score, span(start, start + Time::new(1, 8))).unwrap();
        assert_eq!(partial.events.len(), 1);
        let event = &partial.events[0];
        assert_eq!(lifecycle(event), lifecycle(&full.events[0]));
        assert_eq!(
            event.projected().controls()[&ControlKey::Gain],
            ControlValue::Scalar(0.25)
        );
        let actual = match &event.projected().controls()[&ControlKey::Transpose] {
            ControlValue::Scalar(value) => *value,
            ControlValue::Semitones(value) => value.eval(start.value()),
            ref other => panic!("unexpected transpose value {other:?}"),
        };
        assert_eq!(actual, transpose);
    }
}

#[test]
fn malformed_occurrence_identity_and_controls_are_rejected_without_losing_authored_unisons() {
    let mut duplicate = events(false, Time::ONE);
    duplicate.rows.push(duplicate.rows[0].clone());
    assert!(matches!(
        query(
            &Score::query_source(duplicate.clone()),
            TransportSpan::CYCLE
        ),
        Err(ControlModelError::QuerySource(_))
    ));
    duplicate.rows[1].instance_key += 1;
    let report = query(&Score::query_source(duplicate), TransportSpan::CYCLE).unwrap();
    assert_eq!(report.events.len(), 2);
    assert_ne!(lifecycle(&report.events[0]), lifecycle(&report.events[1]));
    assert_eq!(
        report.events[0].projected().id(),
        report.events[1].projected().id()
    );
    for control in [
        (ControlKey::Gain, ControlValue::Scalar(f64::NAN)),
        (ControlKey::Pitch, ControlValue::Bool(true)),
    ] {
        let mut invalid = events(false, Time::ONE);
        invalid.rows[0].controls.push(control);
        assert!(query(&Score::query_source(invalid), TransportSpan::CYCLE).is_err());
    }
}

#[test]
fn oversized_declared_work_rejects_before_any_host_query_for_score_and_control_sources() {
    for work in [-1.0, 16_385.0, f64::INFINITY, f64::NAN] {
        let mut source = events(false, Time::ONE);
        source.work = work;
        let calls = source.calls.clone();
        let score = Score::query_source(source);
        assert!(PreparedScore::new(score.clone()).is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let calls = Arc::new(AtomicUsize::new(0));
        let controls = ControlScore::query_source(Controls {
            rows: vec![],
            calls: calls.clone(),
            work,
            extent: Time::ONE,
        });
        let score = Score::with_controls(Score::query_source(events(false, Time::ONE)), controls);
        assert!(PreparedScore::new(score.clone()).is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn finite_source_extent_is_honored_by_concat_shift_and_time_scale() {
    let first = Score::query_source(events(false, Time::new(3, 2)));
    let mut second_source = events(false, Time::new(1, 2));
    second_source.rows[0].moment = second_source.rows[0].moment.clone().with_id(72u64);
    let second = Score::query_source(second_source);
    for (score, expected) in [
        (
            Score::concat(vec![first.clone(), second.clone()]),
            vec![
                span(Time::ZERO, Time::new(3, 2)),
                span(Time::new(3, 2), Time::whole_number(2)),
            ],
        ),
        (
            Score::concat(vec![
                Score::shift(first.clone(), Time::new(1, 4)),
                second.clone(),
            ]),
            vec![
                span(Time::ZERO, Time::new(3, 2)),
                span(Time::new(3, 2), Time::whole_number(2)),
            ],
        ),
        (
            Score::concat(vec![
                Score::time_scale(first, Time::whole_number(3)),
                second,
            ]),
            vec![
                span(Time::ZERO, Time::new(1, 2)),
                span(Time::new(1, 2), Time::ONE),
            ],
        ),
    ] {
        let full = query(&score, span(Time::whole_number(-1), Time::whole_number(4))).unwrap();
        assert_eq!(
            full.events
                .iter()
                .map(|event| event.projected().whole())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(full.events[0].projected().id(), Some(MomentId::new(71)));
        assert_eq!(full.events[1].projected().id(), Some(MomentId::new(72)));
    }
}

#[test]
fn query_moment_enforces_the_same_sample_only_control_rules_as_regular_tracks() {
    for sample in [false, true] {
        let mut source = events(sample, Time::ONE);
        source.rows[0]
            .controls
            .push((ControlKey::SampleBank, ControlValue::Choice("kit".into())));
        let result = query(&Score::query_source(source), TransportSpan::CYCLE);
        if sample {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(ControlModelError::UnsupportedControlForSource { .. })
            ));
        }
    }
}

#[test]
fn finite_control_source_extents_and_transforms_keep_segment_boundaries() {
    let lane = |extent, gain| {
        ControlScore::query_source(Controls {
            rows: vec![QueryControl {
                span: span(Time::ZERO, extent),
                key: ControlKey::Gain,
                value: ControlValue::Scalar(gain),
            }],
            calls: Arc::default(),
            work: 1.0,
            extent,
        })
    };
    let first = lane(Time::new(3, 2), 0.5);
    let second = lane(Time::new(1, 2), 0.25);
    let controls = ControlScore::concat(vec![first.clone(), second.clone()]);
    for (controls, expected) in [
        (
            controls.clone(),
            vec![
                (span(Time::ZERO, Time::new(3, 2)), 0.125),
                (span(Time::new(3, 2), Time::whole_number(2)), 0.0625),
            ],
        ),
        (
            ControlScore::shift(controls, Time::new(1, 4)),
            vec![
                (span(Time::ZERO, Time::new(1, 4)), 0.25),
                (span(Time::new(1, 4), Time::new(7, 4)), 0.125),
                (span(Time::new(7, 4), Time::whole_number(2)), 0.0625),
            ],
        ),
        (
            ControlScore::concat(vec![
                ControlScore::time_scale(first, Time::whole_number(3)),
                second,
            ]),
            vec![
                (span(Time::ZERO, Time::new(1, 2)), 0.125),
                (span(Time::new(1, 2), Time::ONE), 0.0625),
                (span(Time::ONE, Time::whole_number(2)), 0.25),
            ],
        ),
    ] {
        let score = Score::with_controls(
            Score::query_source(events(false, Time::whole_number(2))),
            controls,
        );
        let report = query(&score, span(Time::ZERO, Time::whole_number(2))).unwrap();
        let actual: Vec<_> = report
            .events
            .iter()
            .map(|event| {
                (
                    event.projected().visible(),
                    event.projected().controls()[&ControlKey::Gain].clone(),
                )
            })
            .collect();
        assert_eq!(
            actual,
            expected
                .into_iter()
                .map(|(span, gain)| (span, ControlValue::Scalar(gain)))
                .collect::<Vec<_>>()
        );
    }
}
