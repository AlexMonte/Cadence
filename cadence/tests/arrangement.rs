//! Timed source clocks, authored identity, clipping and bounded queries.
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
    application::EvaluatedEventKind,
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    prelude::*,
};

fn span(start: Time, end: Time) -> TransportSpan {
    TransportSpan::new(start, end).unwrap()
}
fn note(name: &str, id: u64) -> Score {
    Score::from(
        Voice::new(
            Time::ONE,
            vec![
                Tile::spanning(Time::ZERO, Time::ONE, Intent::sample(name))
                    .unwrap()
                    .with_id(id),
            ],
        )
        .unwrap(),
    )
}
fn query(score: &Score, start: Time, end: Time) -> PreviewReport {
    let prepared = PreparedScore::new(score.clone()).unwrap();
    CadenceCompiler::new()
        .preview(&prepared, &span(start, end))
        .unwrap()
}
fn labels(report: &PreviewReport) -> Vec<String> {
    report
        .starts()
        .map(|event| match event.projected().intent() {
            Intent::Sample(sample) => sample.sample_id.clone(),
            _ => "synth".into(),
        })
        .collect()
}

#[test]
fn odd_period_arrangement_restarts_nested_alternate_in_every_use_and_loop() {
    let pattern = Score::cycle_slots(vec![
        Score::cycle_route(vec![note("c", 11), note("d", 12)]),
        note("e", 13),
    ]);
    let arrangement = Score::arrange(vec![
        TimedScore::new(pattern.clone(), Time::whole_number(3), 1).unwrap(),
        TimedScore::new(note("g", 14), Time::ONE, 1).unwrap(),
        TimedScore::new(pattern, Time::whole_number(3), 1).unwrap(),
    ])
    .unwrap();
    let expected = [
        "c", "e", "d", "e", "c", "e", "g", "c", "e", "d", "e", "c", "e",
    ];
    assert_eq!(
        labels(&query(&arrangement, Time::ZERO, Time::whole_number(14))),
        expected.repeat(2)
    );
    let full = query(&arrangement, Time::ZERO, Time::whole_number(14));
    for start in 0..56 {
        let window = span(Time::new(start, 4), Time::new(start + 1, 4));
        let partial = query(&arrangement, window.start(), window.end());
        for actual in partial.starts() {
            let original = full
                .starts()
                .find(|original| original.kind() == actual.kind())
                .expect("query slices must retain lifecycle identity");
            assert_eq!(original.projected().id(), actual.projected().id());
            assert_eq!(original.projected().whole(), actual.projected().whole());
            assert_eq!(
                actual.projected().visible(),
                original
                    .projected()
                    .visible()
                    .intersection(&window)
                    .unwrap()
            );
        }
        assert_eq!(
            partial.starts().count(),
            full.starts()
                .filter(|event| event.projected().visible().intersects(&window))
                .count()
        );
    }
    let later_c = query(&arrangement, Time::whole_number(7), Time::new(15, 2));
    assert_eq!(
        later_c.starts().next().unwrap().projected().id(),
        Some(MomentId::new(11)),
        "authored provenance must not become an occurrence ID"
    );
}

#[test]
fn repeats_clip_slow_whole_spans_and_nested_arrangements_restart() {
    let source = Score::time_scale(
        Score::cycle_route(vec![note("c", 1), note("d", 2)]),
        Time::new(1, 2),
    );
    let repeated = Score::arrange(vec![
        TimedScore::new(source, Time::whole_number(3), 2).unwrap(),
    ])
    .unwrap();
    let report = query(&repeated, Time::ZERO, Time::whole_number(6));
    assert_eq!(labels(&report), ["c", "d", "c", "d"]);
    let expected = [(0, 2), (2, 3), (3, 5), (5, 6)];
    for (event, (start, end)) in report.starts().zip(expected) {
        let expected = span(Time::whole_number(start), Time::whole_number(end));
        assert_eq!(event.projected().whole(), expected);
        assert!(
            matches!(event.kind(), EvaluatedEventKind::StartVoice { voice_whole, .. } if *voice_whole == expected)
        );
    }
    let nested =
        Score::arrange(vec![TimedScore::new(repeated, Time::new(3, 2), 2).unwrap()]).unwrap();
    let report = query(&nested, Time::ZERO, Time::whole_number(3));
    assert_eq!(labels(&report), ["c", "c"]);
    assert_eq!(
        report
            .starts()
            .map(|event| event.projected().whole())
            .collect::<Vec<_>>(),
        vec![
            span(Time::ZERO, Time::new(3, 2)),
            span(Time::new(3, 2), Time::whole_number(3))
        ]
    );
}

#[test]
fn control_only_arrangements_reset_note_values_and_slice_ramps_at_occurrence_bounds() {
    let track = |pitch| {
        ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::ONE,
                        ControlKey::Pitch,
                        ControlValue::Scalar(pitch),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        )
    };
    let control_pattern = ControlScore::cycle_route(vec![track(60.0), track(62.0)]);
    let controls = ControlScore::arrange(vec![
        TimedControlScore::new(control_pattern, Time::whole_number(3), 2).unwrap(),
    ])
    .unwrap();
    let song = Score::with_controls(note("tone", 1), controls);
    let report = query(&song, Time::ZERO, Time::whole_number(7));
    let pitches: Vec<_> = report
        .starts()
        .map(|event| event.projected().controls()[&ControlKey::Pitch].clone())
        .collect();
    assert_eq!(
        pitches,
        [60., 62., 60., 60., 62., 60., 60.].map(ControlValue::Scalar)
    );

    let ramp = ControlScore::from(
        ControlTrack::new(
            Time::whole_number(2),
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::whole_number(2),
                    ControlKey::Gain,
                    ControlValue::Ramp { from: 0.0, to: 1.0 },
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let ramp =
        ControlScore::arrange(vec![TimedControlScore::new(ramp, Time::ONE, 2).unwrap()]).unwrap();
    let report = query(
        &Score::with_controls(note("tone", 1), ramp),
        Time::ONE,
        Time::whole_number(2),
    );
    assert_eq!(
        report.starts().next().unwrap().projected().controls()[&ControlKey::Gain],
        ControlValue::Ramp { from: 0.0, to: 0.5 }
    );
}

#[test]
fn parallel_authored_unisons_survive_and_updates_keep_each_occurrence_identity() {
    let held = |id| {
        Score::from(
            Voice::new(
                Time::whole_number(2),
                vec![
                    Tile::spanning(
                        Time::ZERO,
                        Time::whole_number(2),
                        Intent::synth(BuiltInSynthSource::Sine),
                    )
                    .unwrap()
                    .with_id(id),
                ],
            )
            .unwrap(),
        )
    };
    let controls = ControlScore::from(
        ControlTrack::new(
            Time::whole_number(2),
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    ControlKey::Transpose,
                    ControlValue::Scalar(0.0),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::ONE,
                    Time::whole_number(2),
                    ControlKey::Transpose,
                    ControlValue::Scalar(12.0),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let pattern = Score::with_controls(Score::merge(vec![held(1), held(2)]), controls);
    let arrangement =
        Score::arrange(vec![TimedScore::new(pattern, Time::new(3, 2), 2).unwrap()]).unwrap();
    let report = query(&arrangement, Time::ZERO, Time::whole_number(3));
    assert_eq!(report.starts().count(), 4);
    assert_eq!(report.control_updates().count(), 4);
    let starts: std::collections::BTreeSet<_> = report
        .starts()
        .map(|event| match event.kind() {
            EvaluatedEventKind::StartVoice { voice_id, .. } => *voice_id,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(starts.len(), 4);
    for update in report.control_updates() {
        assert!(
            matches!(update.kind(), EvaluatedEventKind::UpdateVoiceControls { voice_id, .. } if starts.contains(voice_id))
        );
    }
    let partial = query(&arrangement, Time::new(11, 4), Time::whole_number(3));
    assert!(
        partial
            .starts()
            .all(|event| report.starts().any(|full| full.kind() == event.kind()))
    );
}

#[test]
fn rendered_noise_and_signal_phase_repeat_exactly_without_stretching() {
    let noise = Score::from(
        Voice::new(
            Time::whole_number(2),
            vec![
                Tile::spanning(
                    Time::ZERO,
                    Time::whole_number(2),
                    Intent::synth(BuiltInSynthSource::Noise),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let gain = Signal::sine()
        .with_rate(Time::new(1, 2))
        .with_bias(0.5)
        .with_depth(0.2);
    let noise = Score::with_controls(
        noise,
        ControlScore::from(
            ControlTrack::new(
                Time::whole_number(2),
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::whole_number(2),
                        ControlKey::Gain,
                        ControlValue::Signal(gain),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        ),
    );
    let arranged =
        Score::arrange(vec![TimedScore::new(noise, Time::new(3, 2), 2).unwrap()]).unwrap();
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8_000, 256)).unwrap();
    let mut runtime = PlaybackRuntime::new(
        PlaybackSettings {
            cps: Time::ONE,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 64),
        },
        audio,
    );
    runtime
        .play_prepared_score(PreparedScore::new(arranged).unwrap())
        .unwrap();
    let mut frames = vec![Frame::ZERO; 24_000];
    for block in frames.chunks_mut(61) {
        runtime.tick().unwrap();
        renderer.render(block);
    }
    assert!(
        frames[..12_000]
            .iter()
            .zip(&frames[12_000..])
            .all(|(first, second)| (first.left - second.left).abs() < 1.0e-6)
    );
    runtime.seek(Time::new(7, 4)).unwrap();
    let mut sought = vec![Frame::ZERO; 400];
    for block in sought.chunks_mut(61) {
        runtime.tick().unwrap();
        renderer.render(block);
    }
    assert!(
        sought[100..]
            .iter()
            .zip(&frames[14_100..14_400])
            .all(|(sought, reference)| (sought.left - reference.left).abs() < 1.0e-6)
    );
}

#[test]
fn invalid_or_excessive_arrangements_fail_preparation() {
    assert!(TimedScore::new(note("c", 1), Time::ZERO, 1).is_err());
    assert!(TimedScore::new(note("c", 1), Time::ONE, 0).is_err());
    assert!(TimedScore::new(note("c", 1), Time::whole_number(1_000_000), u32::MAX).is_err());
    assert!(Score::arrange(Vec::new()).is_err());
    let tiny = TimedScore::new(note("c", 1), Time::new(1, 1_000_000), 1_000_000).unwrap();
    let dense = Score::arrange(vec![tiny]).unwrap();
    assert!(PreparedScore::new(dense.clone()).is_err());
    let control = ControlScore::from(
        ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    ControlKey::Gain,
                    ControlValue::Scalar(0.5),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let control = ControlScore::arrange(vec![
        TimedControlScore::new(control, Time::new(1, 1_000_000), 1_000_000).unwrap(),
    ])
    .unwrap();
    let with_controls = Score::with_controls(note("c", 1), control);
    assert!(PreparedScore::new(with_controls).is_err());
    assert!(Time::new(i64::MAX, i64::MAX - 1) > Time::ONE);
}
