//! Musical pitch and modulation checks across projection, scheduling and DSP.
use cadence::{
    RendererCore,
    adapter::{
        audio::{
            Frame, SampleBuffer,
            offline::{OfflineRenderSettings, render_wav},
        },
        sample_bank::SampleLoadOptions,
    },
    application::EvaluatedEventKind,
    domain::{
        control::{ControlKey, ControlTile, ControlTrack, ControlValue, SemitoneControl},
        intent::{BuiltInSynthSource, Intent},
        rational::Time,
        score::{ControlScore, Score},
        signal::Signal,
        span::TransportSpan,
        voice::{Repeat, Tile, Voice},
    },
    infrastructure::{PreparedScore, playback::SampleBank},
};
use std::time::Duration;

fn lane(source: Score, key: ControlKey, values: &[(Time, Time, ControlValue)]) -> Score {
    Score::with_controls(
        source,
        ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                values
                    .iter()
                    .map(|(start, end, value)| {
                        ControlTile::spanning(*start, *end, key.clone(), value.clone()).unwrap()
                    })
                    .collect(),
            )
            .unwrap(),
        ),
    )
}
fn constant(source: Score, key: ControlKey, value: ControlValue) -> Score {
    lane(source, key, &[(Time::ZERO, Time::ONE, value)])
}
fn note(pitch: f64) -> Score {
    let source = Score::from(
        Voice::new(
            Time::ONE,
            vec![
                Tile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    Intent::synth(BuiltInSynthSource::Sine),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .with_repeat(Repeat::Once),
    );
    constant(source, ControlKey::Pitch, ControlValue::Scalar(pitch))
}
fn transpose(source: Score, value: ControlValue) -> Score {
    constant(source, ControlKey::Transpose, value)
}
fn pcm(score: Score, block_frames: usize) -> Vec<i16> {
    pcm_with_bank(score, block_frames, SampleBank::new())
}
fn pcm_with_bank(score: Score, block_frames: usize, samples: SampleBank) -> Vec<i16> {
    let wav = render_wav(
        PreparedScore::new(score).unwrap(),
        samples,
        OfflineRenderSettings {
            block_frames,
            ..OfflineRenderSettings::new(8_000, Duration::from_secs(1), Time::ONE, 8_000)
        },
    )
    .unwrap();
    wav[44..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|stereo| i16::from_le_bytes([stereo[0], stereo[1]]))
        .collect()
}

#[test]
fn sample_transpose_changes_rate_with_or_without_absolute_pitch() {
    let source = Score::from(
        Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample("tone")).unwrap()],
        )
        .unwrap()
        .with_repeat(Repeat::Once),
    );
    let bank = SampleBank::new();
    bank.load_with_options(
        "tone",
        SampleBuffer::new(
            8_000,
            (0..16_000)
                .map(|n| Frame::from_mono((n as f32 * 0.17).sin() * 0.25))
                .collect::<Vec<_>>(),
        ),
        SampleLoadOptions {
            root_pitch: Some(60.0),
            ..SampleLoadOptions::default()
        },
    );
    let octave = transpose(source.clone(), ControlValue::Scalar(12.0));
    let twice_rate = constant(
        source.clone(),
        ControlKey::PlaybackRate,
        ControlValue::Scalar(2.0),
    );
    assert_eq!(
        pcm_with_bank(octave.clone(), 127, bank.clone()),
        pcm_with_bank(twice_rate, 127, bank.clone())
    );
    let pitched_octave = constant(octave, ControlKey::Pitch, ControlValue::Scalar(60.0));
    let absolute = constant(source, ControlKey::Pitch, ControlValue::Scalar(72.0));
    assert_eq!(
        pcm_with_bank(pitched_octave, 127, bank.clone()),
        pcm_with_bank(absolute, 127, bank)
    );
}
fn semitones(value: &ControlValue, cycle: f64) -> f64 {
    match value {
        ControlValue::Scalar(value) => *value,
        ControlValue::Signal(signal) => signal.eval(cycle),
        ControlValue::Semitones(control) => control.eval(cycle),
        other => panic!("unresolved transpose: {other:?}"),
    }
}

#[test]
fn c4_plus_one_semitone_is_exactly_c_sharp4_and_octave_down_is_not_pitch_bend() {
    assert_eq!(
        pcm(transpose(note(60.0), ControlValue::Scalar(1.0)), 127),
        pcm(note(61.0), 127),
    );
    assert_eq!(
        pcm(transpose(note(72.0), ControlValue::Scalar(-12.0)), 127),
        pcm(note(60.0), 127),
    );
    assert_eq!(
        pcm(
            transpose(
                note(60.0),
                ControlValue::Signal(Signal::sine().with_depth(0.0).with_bias(1.0)),
            ),
            127,
        ),
        pcm(note(61.0), 127),
    );
}

#[test]
fn patterned_transpose_preserves_base_pitch_voice_identity_and_onsets() {
    let score = lane(
        transpose(note(60.0), ControlValue::Scalar(12.0)),
        ControlKey::Transpose,
        &[
            (Time::ZERO, Time::new(1, 2), ControlValue::Scalar(0.0)),
            (Time::new(1, 2), Time::ONE, ControlValue::Scalar(1.0)),
        ],
    );
    let events = RendererCore::new(score.clone())
        .evaluate_window(&TransportSpan::new(Time::ZERO, Time::ONE).unwrap())
        .unwrap();
    assert_eq!(events.len(), 2);
    let (voice_whole, voice_id) = match events[0].kind() {
        EvaluatedEventKind::StartVoice {
            voice_whole,
            voice_id,
        } => (*voice_whole, *voice_id),
        other => panic!("unexpected first event: {other:?}"),
    };
    assert_eq!(
        voice_whole,
        TransportSpan::new(Time::ZERO, Time::ONE).unwrap()
    );
    assert_eq!(
        events[0].projected().controls().get(&ControlKey::Pitch),
        Some(&ControlValue::Scalar(60.0))
    );
    assert!(
        matches!(events[1].kind(), EvaluatedEventKind::UpdateVoiceControls { voice_whole: whole, voice_id: id } if *whole == voice_whole && *id == voice_id)
    );
    assert_eq!(events[1].projected().visible().start(), Time::new(1, 2));
    for (event, expected) in events.iter().zip([12.0, 13.0]) {
        assert_eq!(
            semitones(
                &event.projected().controls()[&ControlKey::Transpose],
                event.projected().visible().start().value()
            ),
            expected
        );
    }
    let rendered = pcm(score.clone(), 127);
    assert_eq!(rendered, pcm(score, 17));
    let reference = pcm(note(72.0), 127);
    assert_eq!(&rendered[..4000], &reference[..4000]);
    assert_ne!(&rendered[4000..], &reference[4000..]);
    // The change occurs on frame 4000 without resetting the oscillator's phase.
    assert_eq!(rendered[4000], reference[4000]);
}

#[test]
fn ending_a_transpose_scope_returns_to_inherited_pitch_offset() {
    let scoped = lane(
        transpose(note(60.0), ControlValue::Scalar(12.0)),
        ControlKey::Transpose,
        &[(Time::ZERO, Time::new(1, 2), ControlValue::Scalar(1.0))],
    );
    let explicit = lane(
        note(60.0),
        ControlKey::Transpose,
        &[
            (Time::ZERO, Time::new(1, 2), ControlValue::Scalar(13.0)),
            (Time::new(1, 2), Time::ONE, ControlValue::Scalar(12.0)),
        ],
    );
    assert_eq!(pcm(scoped, 127), pcm(explicit, 127));

    let scoped_without_inherited = lane(
        note(60.0),
        ControlKey::Transpose,
        &[(Time::ZERO, Time::new(1, 2), ControlValue::Scalar(1.0))],
    );
    let explicit_zero = lane(
        note(60.0),
        ControlKey::Transpose,
        &[
            (Time::ZERO, Time::new(1, 2), ControlValue::Scalar(1.0)),
            (Time::new(1, 2), Time::ONE, ControlValue::Scalar(0.0)),
        ],
    );
    assert_eq!(pcm(scoped_without_inherited, 127), pcm(explicit_zero, 127));
}

#[test]
fn ramp_and_sum_of_signals_modulate_the_same_held_note() {
    let ramp = transpose(
        note(60.0),
        ControlValue::Ramp {
            from: 0.0,
            to: 12.0,
        },
    );
    let saw = transpose(
        note(60.0),
        ControlValue::Signal(Signal::saw().with_depth(6.0).with_bias(6.0)),
    );
    let ramp_pcm = pcm(ramp.clone(), 127);
    assert_eq!(ramp_pcm, pcm(ramp.clone(), 17));
    let saw_pcm = pcm(saw, 127);
    assert!(
        ramp_pcm
            .iter()
            .zip(saw_pcm)
            .all(|(a, b)| (i32::from(*a) - i32::from(b)).abs() <= 1)
    );
    assert_ne!(ramp_pcm, pcm(note(60.0), 127));
    let events = RendererCore::new(ramp)
        .evaluate_window(&TransportSpan::new(Time::ZERO, Time::ONE).unwrap())
        .unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].kind(),
        EvaluatedEventKind::StartVoice { .. }
    ));
    let control = &events[0].projected().controls()[&ControlKey::Transpose];
    assert_eq!(semitones(control, 0.0), 0.0);
    assert_eq!(semitones(control, 0.5), 6.0);
    assert_eq!(semitones(control, 1.0), 12.0);

    let a = Signal::sine().with_depth(1.0);
    let b = Signal::saw().with_depth(1.0).with_rate(Time::new(1, 2));
    let nested = transpose(
        transpose(note(60.0), ControlValue::Signal(a)),
        ControlValue::Signal(b),
    );
    let combined = SemitoneControl::signal(a)
        .unwrap()
        .combine(SemitoneControl::signal(b).unwrap())
        .unwrap();
    assert_eq!(
        pcm(nested, 127),
        pcm(
            transpose(note(60.0), ControlValue::Semitones(combined)),
            127
        )
    );
}

#[test]
fn transpose_ranges_and_component_budget_fail_explicitly() {
    assert!(SemitoneControl::constant(-127.0).is_ok());
    assert!(SemitoneControl::constant(127.0).is_ok());
    assert!(SemitoneControl::constant(f64::NAN).is_err());
    assert!(SemitoneControl::constant(128.0).is_err());
    assert!(
        SemitoneControl::signal(Signal::linear_ramp(Time::ZERO, Time::ONE, 0.0, f64::NAN)).is_err()
    );
    assert!(
        SemitoneControl::constant(127.0)
            .unwrap()
            .combine(SemitoneControl::constant(1.0).unwrap())
            .is_err()
    );
    let component = SemitoneControl::signal(Signal::sine()).unwrap();
    let mut combined = SemitoneControl::default();
    for _ in 0..8 {
        combined = combined.combine(component.clone()).unwrap();
    }
    assert!(combined.combine(component).is_err());
}
