//! Velocity signals select a stable note intensity, shared by playback and export.
use cadence::{
    RendererCore,
    adapter::audio::{
        Frame, SampleBuffer,
        offline::{OfflineRenderSettings, render_wav},
    },
    infrastructure::playback::SampleBank,
    prelude::{
        BuiltInSynthSource, ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue,
        Intent, PreparedScore, Repeat, Score, Signal, Span, Tile, Time, TimedScore, Voice,
    },
};
use std::time::Duration;

fn source(intent: Intent, count: i64) -> Score {
    Score::from(
        Voice::new(
            Time::ONE,
            (0..count)
                .map(|i| {
                    Tile::spanning(Time::new(i, count), Time::new(i + 1, count), intent.clone())
                        .unwrap()
                })
                .collect(),
        )
        .unwrap()
        .with_repeat(Repeat::Once),
    )
}
fn velocity(source: Score, signal: Signal) -> Score {
    Score::with_controls(
        source,
        ControlScore::from(ControlTrack::from_signal(ControlKey::Velocity, signal).unwrap()),
    )
}
fn levels(score: &Score, start: Time, end: Time) -> Vec<(Time, f64)> {
    RendererCore::new(score.clone())
        .evaluate_window(&Span::new(start, end).unwrap())
        .unwrap()
        .iter()
        .map(|event| {
            let value = match &event.projected().controls()[&ControlKey::Velocity] {
                ControlValue::Unipolar(value) => value.value(),
                other => panic!("velocity must be resolved at onset: {other:?}"),
            };
            (event.projected().whole().start(), value)
        })
        .collect()
}
fn saw() -> Signal {
    Signal::saw()
        .with_bias(0.9)
        .with_depth(0.1)
        .with_rate(Time::new(4, 1))
}

#[test]
fn saw_samples_each_note_without_modulating_a_held_note() {
    let score = velocity(source(Intent::sample("tone"), 16), saw());
    for (index, (_, value)) in levels(&score, Time::ZERO, Time::ONE).iter().enumerate() {
        assert!((value - (0.8 + (index % 4) as f64 * 0.05)).abs() < 1e-12);
    }
    let held = velocity(source(Intent::sample("tone"), 1), saw());
    assert_eq!(levels(&held, Time::new(3, 4), Time::ONE)[0].1, 0.8);
}

#[test]
fn seeded_random_and_outer_timing_transforms_keep_the_original_intensity() {
    let signal = Signal::random(73)
        .with_bias(0.85)
        .with_depth(0.15)
        .with_rate(Time::new(4, 1));
    let score = velocity(source(Intent::synth(BuiltInSynthSource::Sine), 16), signal);
    let expected = levels(&score, Time::ZERO, Time::ONE);
    assert_eq!(expected.len(), 16);
    assert_eq!(
        expected
            .iter()
            .map(|(_, value)| value.to_bits())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        16
    );
    assert!(
        expected
            .iter()
            .all(|(_, value)| (0.7..=1.0).contains(value))
    );
    for (onset, value) in &expected {
        assert_eq!(*value, signal.eval_at(*onset));
        let seek = *onset + Time::new(1, 32);
        assert_eq!(levels(&score, seek, seek + Time::new(1, 32))[0].1, *value);
    }
    let faster = Score::time_scale(score.clone(), Time::new(2, 1));
    let reversed = Score::reflect_cycle(score.clone());
    let moved = Score::shift(score.clone(), Time::new(7, 3));
    let arranged = Score::arrange(vec![
        TimedScore::new(Score::empty(), Time::new(7, 3), 1).unwrap(),
        TimedScore::new(score, Time::ONE, 2).unwrap(),
    ])
    .unwrap();
    assert_eq!(
        levels(&faster, Time::ZERO, Time::new(1, 2))
            .iter()
            .map(|x| x.1)
            .collect::<Vec<_>>(),
        expected.iter().map(|x| x.1).collect::<Vec<_>>()
    );
    assert_eq!(
        levels(&reversed, Time::ZERO, Time::ONE)
            .iter()
            .map(|x| x.1)
            .collect::<Vec<_>>(),
        expected.iter().rev().map(|x| x.1).collect::<Vec<_>>()
    );
    for moved in [&moved, &arranged] {
        assert_eq!(
            levels(moved, Time::new(7, 3), Time::new(10, 3))
                .iter()
                .map(|x| x.1)
                .collect::<Vec<_>>(),
            expected.iter().map(|x| x.1).collect::<Vec<_>>()
        );
    }
}

#[test]
fn multiple_velocity_owners_multiply_and_seek_recovers_an_ended_control_tile() {
    let score = velocity(source(Intent::sample("tone"), 1), saw());
    let score = Score::with_controls(
        score,
        ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::new(1, 4),
                        ControlKey::Velocity,
                        ControlValue::Scalar(0.5),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        ),
    );
    assert_eq!(levels(&score, Time::ZERO, Time::ONE)[0].1, 0.4);
    assert_eq!(levels(&score, Time::new(3, 4), Time::ONE)[0].1, 0.4);
}

fn pcm(score: Score, block_frames: usize) -> Vec<u8> {
    let bank = SampleBank::new();
    bank.load(
        "tone",
        SampleBuffer::new(8_000, vec![Frame::from_mono(0.5); 8_000]),
    );
    render_wav(
        PreparedScore::new(score).unwrap(),
        bank,
        OfflineRenderSettings {
            block_frames,
            ..OfflineRenderSettings::new(8_000, Duration::from_secs(1), Time::ONE, 8_000)
        },
    )
    .unwrap()
}

#[test]
fn sampled_signal_pcm_matches_explicit_note_levels_for_samples_and_synths() {
    for intent in [
        Intent::sample("tone"),
        Intent::synth(BuiltInSynthSource::Sine),
    ] {
        let base = source(intent, 16);
        let signaled = velocity(base.clone(), saw());
        let explicit = Score::with_controls(
            base,
            ControlScore::from(
                ControlTrack::new(
                    Time::ONE,
                    (0..16)
                        .map(|i| {
                            ControlTile::spanning(
                                Time::new(i, 16),
                                Time::new(i + 1, 16),
                                ControlKey::Velocity,
                                ControlValue::Scalar(saw().eval(i as f64 / 16.0)),
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap(),
            ),
        );
        assert_eq!(pcm(signaled.clone(), 127), pcm(explicit, 127));
        assert_eq!(pcm(signaled.clone(), 127), pcm(signaled, 17));
    }
}

#[test]
fn out_of_range_and_non_finite_velocity_signals_are_rejected() {
    for signal in [
        Signal::saw(),
        Signal::saw().with_bias(0.5).with_depth(0.6),
        Signal::saw().with_bias(f64::NAN),
        Signal::saw().with_depth(f64::INFINITY),
    ] {
        assert!(ControlTrack::from_signal(ControlKey::Velocity, signal).is_err());
    }
}

#[test]
fn direct_live_voice_plans_sample_velocity_at_cycle_zero() {
    use cadence::application::audio::{AudioVoicePlan, VoiceInstanceId};
    use cadence::domain::intent::SampleIntent;
    let controls =
        std::collections::BTreeMap::from([(ControlKey::Velocity, ControlValue::Signal(saw()))]);
    let synth = AudioVoicePlan::from_live_synth(
        VoiceInstanceId::new(1),
        BuiltInSynthSource::Sine,
        &controls,
        None,
        None,
        Duration::from_secs(1),
    )
    .unwrap();
    let sample = AudioVoicePlan::from_live_sample(
        VoiceInstanceId::new(2),
        &SampleIntent::new("tone"),
        &controls,
        None,
        None,
        Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(synth.mix.velocity.value(), 0.8);
    assert_eq!(sample.mix.velocity.value(), 0.8);
    assert!(synth.modulations.is_empty());
    assert!(sample.modulations.is_empty());
}

#[test]
fn fractional_random_uses_normalized_exact_time_and_preserves_stepped_noise() {
    let random = Signal::random(73).with_rate(Time::new(4, 1));
    assert_eq!(
        random.eval_at(Time::new(1, 3)),
        random.eval_at(Time::new(17, 51))
    );
    assert_eq!(
        random.with_phase(Time::new(1, 3)).eval_at(Time::new(1, 3)),
        random.eval_at(Time::new(5, 12))
    );
    let values = (0..16)
        .map(|index| random.eval_at(Time::new(index, 16)))
        .collect::<Vec<_>>();
    assert_eq!(
        values
            .iter()
            .map(|value| value.to_bits())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        16
    );
    assert!(values.iter().all(|value| (-1.0..=1.0).contains(value)));
    assert_ne!(
        values,
        (0..16)
            .map(|index| Signal::random(74)
                .with_rate(Time::new(4, 1))
                .eval_at(Time::new(index, 16)))
            .collect::<Vec<_>>()
    );
    let stepped = Signal::rand(73).with_rate(Time::new(4, 1));
    assert_eq!(
        stepped.eval_at(Time::ZERO),
        stepped.eval_at(Time::new(3, 16))
    );
    assert_ne!(random.eval_at(Time::ZERO), random.eval_at(Time::new(3, 16)));
}
