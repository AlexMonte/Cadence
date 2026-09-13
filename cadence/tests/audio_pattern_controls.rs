//! Held-note DSP changes must agree with projected controls and remain block independent.
use cadence::{
    adapter::audio::{
        Frame, SampleBuffer,
        offline::{OfflineRenderSettings, render_wav},
    },
    infrastructure::playback::SampleBank,
    prelude::*,
};
use std::time::Duration;
const RATE: u32 = 8000;
fn unit(value: f64) -> UnitValue {
    UnitValue::new(value).unwrap()
}
fn source(sample: bool) -> Score {
    let intent = if sample {
        Intent::sample("tone")
    } else {
        Intent::synth(BuiltInSynthSource::Sine)
    };
    let note = Score::from(
        Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::ONE, intent).unwrap()],
        )
        .unwrap()
        .with_repeat(Repeat::Once),
    );
    lane(
        note,
        ControlKey::Gain,
        &[(Time::ZERO, Time::ONE, ControlValue::Scalar(0.2))],
    )
}
fn lane(source: Score, key: ControlKey, values: &[(Time, Time, ControlValue)]) -> Score {
    Score::with_controls(
        source,
        ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                values
                    .iter()
                    .map(|(a, b, v)| ControlTile::spanning(*a, *b, key.clone(), v.clone()).unwrap())
                    .collect(),
            )
            .unwrap(),
        ),
    )
}
fn pcm(score: Score, block_frames: usize) -> Vec<i16> {
    let bank = SampleBank::new();
    bank.load(
        "tone",
        SampleBuffer::new(
            RATE,
            (0..RATE)
                .map(|n| {
                    Frame::from_mono((std::f32::consts::TAU * 440.0 * n as f32 / RATE as f32).sin())
                })
                .collect::<Vec<_>>(),
        ),
    );
    let wav = render_wav(
        PreparedScore::new(score).unwrap(),
        bank,
        OfflineRenderSettings {
            block_frames,
            ..OfflineRenderSettings::new(RATE, Duration::from_secs(1), Time::ONE, RATE as usize)
        },
    )
    .unwrap();
    wav[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect()
}
fn energy(samples: &[i16]) -> f64 {
    samples.iter().map(|n| f64::from(*n).powi(2)).sum()
}
fn step(source: Score, key: ControlKey, a: ControlValue, b: ControlValue) -> Score {
    lane(
        source,
        key,
        &[
            (Time::ZERO, Time::new(1, 2), a),
            (Time::new(1, 2), Time::ONE, b),
        ],
    )
}
#[test]
fn gates_close_and_reopen_without_retriggering_sample_or_synth() {
    for sample in [false, true] {
        let base = pcm(source(sample), 127);
        for initially_open in [false, true] {
            let score = step(
                source(sample),
                ControlKey::Gate,
                ControlValue::Bool(initially_open),
                ControlValue::Bool(!initially_open),
            );
            let audio = pcm(score.clone(), 127);
            assert_eq!(audio, pcm(score, 64));
            let (audible, silent) = if initially_open {
                (1000..7000, 9000..15000)
            } else {
                (9000..15000, 1000..7000)
            };
            assert_eq!(
                audio[audible.clone()],
                base[audible],
                "A gate transition must retain oscillator/sample phase"
            );
            assert_eq!(energy(&audio[silent]), 0.0);
        }
    }
}
#[test]
fn filter_steps_and_gaps_reach_both_sources_and_preserve_other_controls() {
    for sample in [false, true] {
        let runtime_pan = |note| {
            lane(
                note,
                ControlKey::Pan,
                &[
                    (
                        Time::ZERO,
                        Time::new(1, 4),
                        ControlValue::Bipolar(SignedUnitValue::new(0.0).unwrap()),
                    ),
                    (
                        Time::new(1, 4),
                        Time::ONE,
                        ControlValue::Bipolar(SignedUnitValue::new(0.5).unwrap()),
                    ),
                ],
            )
        };
        let base = pcm(runtime_pan(source(sample)), 127);
        for key in [ControlKey::LowPassCutoff, ControlKey::HighPassCutoff] {
            let cutoff = if key == ControlKey::LowPassCutoff {
                20.0
            } else {
                2000.0
            };
            let filtered = lane(
                source(sample),
                key,
                &[(Time::ZERO, Time::new(1, 2), ControlValue::Scalar(cutoff))],
            );
            // A separate runtime boundary must preserve the filter, and the
            // filter's own end must restore bypass rather than a sticky value.
            let score = runtime_pan(filtered);
            let audio = pcm(score.clone(), 127);
            assert_eq!(audio, pcm(score, 64));
            assert!(energy(&audio[2000..7000]) < energy(&base[2000..7000]) / 20.0);
            assert_eq!(audio[10000..15000], base[10000..15000]);
        }
    }
}
#[test]
fn sends_and_compressor_change_mid_note_for_both_sources() {
    let cases = [
        (
            ControlKey::DelaySend,
            ControlValue::Delay(DelaySettings::new(
                unit(0.0),
                Duration::from_millis(50),
                unit(0.0),
                unit(0.0),
            )),
            ControlValue::Delay(DelaySettings::new(
                unit(0.8),
                Duration::from_millis(50),
                unit(0.0),
                unit(0.0),
            )),
        ),
        (
            ControlKey::ReverbSend,
            ControlValue::Reverb(ReverbSettings::new(
                unit(0.0),
                Duration::from_millis(300),
                unit(0.2),
            )),
            ControlValue::Reverb(ReverbSettings::new(
                unit(0.8),
                Duration::from_millis(300),
                unit(0.2),
            )),
        ),
        (
            ControlKey::Compressor,
            ControlValue::Compressor(CompressorSettings::new(
                unit(1.0),
                1.0,
                Duration::from_millis(1),
                Duration::from_millis(1),
            )),
            ControlValue::Compressor(CompressorSettings::new(
                unit(0.02),
                20.0,
                Duration::from_millis(1),
                Duration::from_millis(1),
            )),
        ),
    ];
    for sample in [false, true] {
        let base = pcm(source(sample), 127);
        for (key, a, b) in &cases {
            let score = step(source(sample), key.clone(), a.clone(), b.clone());
            let audio = pcm(score.clone(), 127);
            assert_eq!(audio, pcm(score, 64), "{key:?} buffer independence");
            assert_eq!(audio[1000..7000], base[1000..7000], "{key:?} first half");
            assert_ne!(
                audio[11000..15000],
                base[11000..15000],
                "{key:?} update ignored"
            );
            if *key == ControlKey::Compressor {
                assert!(energy(&audio[11000..15000]) < energy(&base[11000..15000]) / 5.0);
            }
        }
    }
}
#[test]
fn expression_gap_restores_identity_and_nested_inherited_level() {
    for sample in [false, true] {
        for inherited in [None, Some(0.5)] {
            let note = inherited.map_or_else(
                || source(sample),
                |v| {
                    lane(
                        source(sample),
                        ControlKey::Expression,
                        &[(Time::ZERO, Time::ONE, ControlValue::Unipolar(unit(v)))],
                    )
                },
            );
            let base = pcm(note.clone(), 127);
            let score = lane(
                note,
                ControlKey::Expression,
                &[(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlValue::Unipolar(unit(0.25)),
                )],
            );
            let audio = pcm(score.clone(), 127);
            assert_eq!(audio, pcm(score, 64));
            assert!(energy(&audio[1000..7000]) < energy(&base[1000..7000]) / 10.0);
            assert_eq!(audio[10000..15000], base[10000..15000]);
        }
    }
}

#[test]
fn resonance_can_change_without_replacing_cutoff_or_restarting_filter_memory() {
    for sample in [false, true] {
        for (cutoff_key, resonance_key) in [
            (ControlKey::LowPassCutoff, ControlKey::LowPassResonance),
            (ControlKey::HighPassCutoff, ControlKey::HighPassResonance),
        ] {
            let note = lane(
                source(sample),
                cutoff_key,
                &[(Time::ZERO, Time::ONE, ControlValue::Scalar(440.0))],
            );
            let base = pcm(note.clone(), 127);
            let score = step(
                note,
                resonance_key.clone(),
                ControlValue::Unipolar(unit(0.0)),
                ControlValue::Unipolar(unit(0.8)),
            );
            let audio = pcm(score.clone(), 127);
            assert_eq!(audio, pcm(score, 64));
            assert_eq!(audio[1000..7000], base[1000..7000]);
            assert_ne!(
                audio[11000..15000],
                base[11000..15000],
                "{resonance_key:?} must retain its cutoff and change the filter"
            );
        }
    }
}

#[test]
fn lowpass_signal_creates_a_filter_and_survives_unrelated_runtime_updates() {
    for sample in [false, true] {
        let filtered = lane(
            source(sample),
            ControlKey::LowPassCutoff,
            &[(
                Time::ZERO,
                Time::ONE,
                ControlValue::Signal(Signal::sine().with_depth(0.0).with_bias(20.0)),
            )],
        );
        let panned = |note| {
            lane(
                note,
                ControlKey::Pan,
                &[
                    (
                        Time::ZERO,
                        Time::new(1, 4),
                        ControlValue::Bipolar(SignedUnitValue::new(0.0).unwrap()),
                    ),
                    (
                        Time::new(1, 4),
                        Time::ONE,
                        ControlValue::Bipolar(SignedUnitValue::new(0.5).unwrap()),
                    ),
                ],
            )
        };
        let score = panned(filtered);
        let audio = pcm(score.clone(), 127);
        assert_eq!(audio, pcm(score, 64));
        let base = pcm(panned(source(sample)), 127);
        assert!(
            energy(&audio[2000..7000]) < energy(&base[2000..7000]) / 20.0,
            "A cutoff signal must create a filter, sample={sample}"
        );
    }
}

#[test]
fn changing_signals_replaces_bindings_and_scalar_gaps_clear_them() {
    let constant = |value| ControlValue::Signal(Signal::sine().with_depth(0.0).with_bias(value));
    for sample in [false, true] {
        let base = pcm(source(sample), 127);
        let gain = step(
            source(sample),
            ControlKey::Gain,
            constant(0.25),
            constant(0.75),
        );
        let audio = pcm(gain.clone(), 127);
        assert_eq!(audio, pcm(gain, 64));
        assert!(energy(&audio[10000..15000]) > energy(&audio[1000..6000]) * 8.0);
        let gap = lane(
            source(sample),
            ControlKey::Gain,
            &[(Time::ZERO, Time::new(1, 2), constant(0.25))],
        );
        let audio = pcm(gap, 127);
        assert_eq!(audio[10000..15000], base[10000..15000]);
        let filter = step(
            source(sample),
            ControlKey::LowPassCutoff,
            constant(20.0),
            constant(3000.0),
        );
        let audio = pcm(filter.clone(), 127);
        assert_eq!(audio, pcm(filter, 64));
        assert!(energy(&audio[10000..15000]) > energy(&audio[1000..6000]) * 20.0);
    }
}

#[test]
fn a_later_pan_boundary_does_not_restore_an_earlier_signal_slot() {
    let gain = |note, signals| {
        lane(
            note,
            ControlKey::Gain,
            &[
                (
                    Time::ZERO,
                    Time::new(1, 16),
                    if signals {
                        ControlValue::Signal(Signal::sine().with_depth(0.0).with_bias(0.25))
                    } else {
                        ControlValue::Scalar(0.25)
                    },
                ),
                (
                    Time::new(1, 16),
                    Time::ONE,
                    if signals {
                        ControlValue::Signal(Signal::sine().with_depth(0.0).with_bias(0.75))
                    } else {
                        ControlValue::Scalar(0.75)
                    },
                ),
            ],
        )
    };
    let pan = |note| {
        lane(
            note,
            ControlKey::Pan,
            &[
                (
                    Time::ZERO,
                    Time::new(1, 8),
                    ControlValue::Bipolar(SignedUnitValue::new(0.0).unwrap()),
                ),
                (
                    Time::new(1, 8),
                    Time::ONE,
                    ControlValue::Bipolar(SignedUnitValue::new(0.5).unwrap()),
                ),
            ],
        )
    };
    let actual = pcm(pan(gain(source(false), true)), 127);
    let expected = pcm(pan(source(false)), 127);
    for (index, (a, b)) in actual.iter().zip(expected).enumerate() {
        let factor = if index < 1000 { 0.25 } else { 0.75 };
        assert!(
            (f64::from(*a) - f64::from(b) * factor).abs() <= 1.0,
            "stale signal at sample {index}: {a} vs {b} * {factor}"
        );
    }
}
