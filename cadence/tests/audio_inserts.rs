//! Stabilization coverage for the existing bounded insert implementation.
use cadence::{
    adapter::audio::{
        Frame, SampleBuffer,
        offline::{OfflineRenderSettings, render_wav},
    },
    infrastructure::playback::SampleBank,
    prelude::*,
};
use std::time::Duration;

fn chain(effects: Vec<InsertEffect>) -> InsertChain {
    InsertChain::new(effects).unwrap()
}
fn note(sample: bool, inserts: InsertChain) -> Score {
    let intent = if sample {
        Intent::Sample(SampleIntent::new("tone").with_inserts(inserts))
    } else {
        Intent::Synth(SynthIntent::new(BuiltInSynthSource::Sine).with_inserts(inserts))
    };
    let voice = Voice::new(
        Time::ONE,
        vec![Tile::spanning(Time::new(1, 8), Time::new(7, 8), intent).unwrap()],
    )
    .unwrap()
    .with_repeat(Repeat::Once);
    controls(
        Score::from(voice),
        &[(ControlKey::Gain, 0.2), (ControlKey::Pitch, 69.0)],
    )
}
fn controls(source: Score, values: &[(ControlKey, f64)]) -> Score {
    Score::with_controls(
        source,
        ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                values
                    .iter()
                    .map(|(key, value)| {
                        ControlTile::spanning(
                            Time::ZERO,
                            Time::ONE,
                            key.clone(),
                            ControlValue::Scalar(*value),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
            .unwrap(),
        ),
    )
}
fn pcm(score: Score, rate: u32, block_frames: usize) -> Vec<i16> {
    let bank = SampleBank::new();
    bank.load(
        "tone",
        SampleBuffer::new(
            rate,
            (0..rate)
                .map(|n| {
                    Frame::from_mono(
                        (std::f64::consts::TAU * 440.0 * f64::from(n) / f64::from(rate)).sin()
                            as f32,
                    )
                })
                .collect::<Vec<_>>(),
        ),
    );
    let wav = render_wav(
        PreparedScore::new(score).unwrap(),
        bank,
        OfflineRenderSettings {
            block_frames,
            ..OfflineRenderSettings::new(rate, Duration::from_secs(1), Time::ONE, rate as usize)
        },
    )
    .unwrap();
    wav[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
        .collect()
}

#[test]
fn empty_and_neutral_chains_are_exact_identity_for_sample_and_synth() {
    for sample in [false, true] {
        let base = pcm(note(sample, InsertChain::default()), 8_000, 127);
        for effects in [
            vec![],
            vec![InsertEffect::drive(0.0, 1.0, 1.0).unwrap()],
            vec![InsertEffect::drive(1.0, 0.0, 1.0).unwrap(); 8],
        ] {
            assert_eq!(base, pcm(note(sample, chain(effects)), 8_000, 127));
        }
    }
}

#[test]
fn filter_drive_order_changes_pcm_without_changing_onsets_or_block_results() {
    let filter = InsertEffect::low_pass(500.0, 0.0).unwrap();
    let drive = InsertEffect::drive(0.8, 1.0, 0.3).unwrap();
    let end_filter = InsertEffect::high_pass(60.0, 0.0).unwrap();
    for sample in [false, true] {
        let score = note(sample, chain(vec![filter, drive, end_filter]));
        let a = pcm(score.clone(), 8_000, 127);
        assert_eq!(a, pcm(score, 8_000, 31));
        let b = pcm(
            note(sample, chain(vec![drive, filter, end_filter])),
            8_000,
            127,
        );
        let difference = a
            .iter()
            .zip(&b)
            .map(|(a, b)| (f64::from(*a) - f64::from(*b)).powi(2))
            .sum::<f64>();
        let power = a.iter().map(|a| f64::from(*a).powi(2)).sum::<f64>();
        assert!(
            difference / power > 0.01,
            "ordered inserts must have an audible effect"
        );
        assert!(a[..2_000].iter().all(|sample| *sample == 0));
        assert!(a[14_000..].iter().all(|sample| *sample == 0));
    }
}

#[test]
fn prepared_filter_matches_existing_filter_at_actual_device_rates() {
    for rate in [8_000, 44_100, 96_000] {
        for sample in [false, true] {
            let existing = controls(
                note(sample, InsertChain::default()),
                &[
                    (ControlKey::LowPassCutoff, 12_000.0),
                    (ControlKey::LowPassResonance, 0.4),
                ],
            );
            let inserted = note(
                sample,
                chain(vec![InsertEffect::low_pass(12_000.0, 0.4).unwrap()]),
            );
            assert_eq!(pcm(existing, rate, 127), pcm(inserted, rate, 127));
        }
    }
}

#[test]
fn limits_reject_invalid_authoring_and_maximum_chain_stays_finite() {
    for value in [f64::NAN, f64::INFINITY, -1.0, 20_001.0] {
        assert!(InsertEffect::low_pass(value, 0.0).is_err());
    }
    for value in [f64::NAN, f64::NEG_INFINITY, -0.01, 1.01] {
        assert!(InsertEffect::high_pass(500.0, value).is_err());
        assert!(InsertEffect::drive(value, 1.0, 1.0).is_err());
        assert!(InsertEffect::drive(1.0, value, 1.0).is_err());
    }
    assert!(InsertEffect::drive(1.0, 1.0, 2.01).is_err());
    let effects = vec![InsertEffect::low_pass(20_000.0, 1.0).unwrap(); 9];
    assert_eq!(InsertChain::new(effects), Err(InsertError::TooManyEffects));
    let effects = (0..8)
        .map(|n| {
            if n % 2 == 0 {
                InsertEffect::high_pass(20.0, 1.0).unwrap()
            } else {
                InsertEffect::drive(1.0, 1.0, 2.0).unwrap()
            }
        })
        .collect();
    // Offline rendering explicitly errors on non-finite DSP before conversion.
    let result = pcm(note(false, chain(effects)), 8_000, 127);
    assert!(result.iter().any(|sample| *sample != 0));
}

#[test]
fn host_reconstruction_preserves_order_parameters_and_sound() {
    let original = chain(vec![
        InsertEffect::low_pass(750.0, 0.2).unwrap(),
        InsertEffect::drive(0.5, 0.75, 0.3).unwrap(),
        InsertEffect::high_pass(75.0, 0.1).unwrap(),
    ]);
    let rebuilt = chain(
        original
            .effects()
            .iter()
            .map(|effect| match effect {
                InsertEffect::LowPass(value) => {
                    InsertEffect::low_pass(value.cutoff_hz(), value.resonance().value())
                }
                InsertEffect::HighPass(value) => {
                    InsertEffect::high_pass(value.cutoff_hz(), value.resonance().value())
                }
                InsertEffect::Drive(value) => {
                    InsertEffect::drive(value.amount(), value.wet(), value.output_gain())
                }
            })
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
    );
    assert_eq!(original, rebuilt);
    assert_eq!(
        pcm(note(false, original), 8_000, 127),
        pcm(note(false, rebuilt), 8_000, 127)
    );
}
