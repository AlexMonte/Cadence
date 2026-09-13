//! Mixer pan and nested gain verified through scheduled stereo PCM rendering.
use std::time::Duration;

use cadence::{
    adapter::audio::{
        AudioRenderer, AudioRendererSettings, Frame, SampleBuffer,
        offline::{OfflineRenderSettings, render_wav},
    },
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings, SampleBank},
    prelude::*,
};

const RATE: u32 = 8_000;

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
fn bipolar(value: f64) -> ControlValue {
    ControlValue::Bipolar(SignedUnitValue::new(value).unwrap())
}
fn pan(source: Score, value: f64) -> Score {
    constant(source, ControlKey::Pan, bipolar(value))
}
fn source(intent: Intent) -> Score {
    let score = Score::from(
        Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::new(1, 8), Time::new(7, 8), intent).unwrap()],
        )
        .unwrap()
        .with_repeat(Repeat::Once),
    );
    constant(score, ControlKey::Gain, ControlValue::Scalar(0.2))
}
fn synth() -> Score {
    source(Intent::synth(BuiltInSynthSource::Sine))
}
fn bank() -> SampleBank {
    let bank = SampleBank::new();
    bank.load(
        "tone",
        SampleBuffer::new(
            RATE,
            (0..RATE)
                .map(|n| {
                    Frame::from_mono((std::f32::consts::TAU * 250.0 * n as f32 / RATE as f32).sin())
                })
                .collect::<Vec<_>>(),
        ),
    );
    bank.load(
        "stereo",
        SampleBuffer::new(
            RATE,
            (0..RATE)
                .map(|n| {
                    let phase = std::f32::consts::TAU * n as f32 / RATE as f32;
                    Frame::new((phase * 250.0).sin(), (phase * 170.0).cos())
                })
                .collect::<Vec<_>>(),
        ),
    );
    bank
}
fn pcm(score: Score, block_frames: usize) -> Vec<[i16; 2]> {
    let wav = render_wav(
        PreparedScore::new(score).unwrap(),
        bank(),
        OfflineRenderSettings {
            block_frames,
            ..OfflineRenderSettings::new(RATE, Duration::from_secs(1), Time::ONE, RATE as usize)
        },
    )
    .unwrap();
    wav[44..]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|bytes| {
            [
                i16::from_le_bytes([bytes[0], bytes[1]]),
                i16::from_le_bytes([bytes[2], bytes[3]]),
            ]
        })
        .collect()
}
fn close(actual: [i16; 2], expected: [f64; 2], frame: usize) {
    for channel in 0..2 {
        assert!(
            (f64::from(actual[channel]) - expected[channel]).abs() <= 2.0,
            "frame {frame}, channel {channel}: {actual:?} != {expected:?}"
        );
    }
}
fn balance(value: [i16; 2], offset: f64) -> [f64; 2] {
    let offset = offset.clamp(-1.0, 1.0);
    [
        f64::from(value[0]) * (1.0 - offset).sqrt(),
        f64::from(value[1]) * (1.0 + offset).sqrt(),
    ]
}

#[test]
fn center_is_exact_and_hard_pan_preserves_power_without_distance_loss() {
    for score in [
        synth(),
        source(Intent::sample("tone")),
        source(Intent::sample("stereo")),
    ] {
        let center = pcm(score.clone(), 127);
        assert_eq!(center, pcm(pan(score.clone(), 0.0), 127));
        for side in [-1.0, 1.0] {
            let hard = pcm(pan(score.clone(), side), 127);
            for (frame, (actual, original)) in hard.iter().zip(&center).enumerate() {
                close(*actual, balance(*original, side), frame);
            }
            let power = |audio: &[[i16; 2]]| {
                audio
                    .iter()
                    .flatten()
                    .map(|s| f64::from(*s).powi(2))
                    .sum::<f64>()
            };
            assert!((power(&hard) / power(&center) - 1.0).abs() < 0.001);
            let silent = if side < 0.0 { 1 } else { 0 };
            assert!(hard.iter().all(|frame| frame[silent] == 0));
        }
    }
}

#[test]
fn constant_output_controls_preserve_changing_note_pan_and_post_gain() {
    let source = lane(
        synth(),
        ControlKey::Pan,
        &[
            (Time::ZERO, Time::new(1, 2), bipolar(0.25)),
            (Time::new(1, 2), Time::ONE, bipolar(-0.25)),
        ],
    );
    let source = lane(
        source,
        ControlKey::PostGain,
        &[
            (Time::ZERO, Time::new(1, 2), ControlValue::Scalar(0.4)),
            (Time::new(1, 2), Time::ONE, ControlValue::Scalar(0.6)),
        ],
    );
    let score = pan(
        constant(source, ControlKey::PostGain, ControlValue::Scalar(0.5)),
        0.5,
    );
    let actual = pcm(score.clone(), 127);
    assert_eq!(actual, pcm(score, 31));
    for (frame, (actual, original)) in actual.iter().zip(pcm(synth(), 127)).enumerate() {
        let (offset, factor) = if frame < 4_000 {
            (0.75, 0.2)
        } else {
            (0.25, 0.3)
        };
        close(
            *actual,
            balance(original, offset).map(|sample| sample * factor),
            frame,
        );
    }
}

#[test]
fn nested_pan_sums_before_clamping_and_does_not_depend_on_group_order() {
    let reference = pcm(pan(synth(), 0.75), 127);
    for offsets in [
        [0.75, 0.75, -0.75],
        [-0.75, 0.75, 0.75],
        [0.75, -0.75, 0.75],
    ] {
        let nested = offsets.into_iter().fold(synth(), pan);
        assert_eq!(pcm(nested, 127), reference);
    }
    assert_eq!(
        pcm(pan(pan(synth(), 1.0), 1.0), 127),
        pcm(pan(synth(), 1.0), 127)
    );
}

fn timed_pan() -> Score {
    lane(
        pan(synth(), 0.25),
        ControlKey::Pan,
        &[
            (Time::ZERO, Time::new(1, 2), bipolar(-1.0)),
            (Time::new(1, 2), Time::new(3, 4), bipolar(0.75)),
        ],
    )
}

#[test]
fn timed_pan_preserves_phase_onsets_nested_offset_and_block_partition() {
    let original = pcm(synth(), 127);
    let actual = pcm(timed_pan(), 127);
    assert_eq!(actual, pcm(timed_pan(), 31));
    assert_eq!(actual, pcm(timed_pan(), 500));
    for (frame, (actual, original)) in actual.iter().zip(original).enumerate() {
        let offset = if frame < 4_000 {
            -0.75
        } else if frame < 6_000 {
            1.0
        } else {
            0.25
        };
        close(*actual, balance(original, offset), frame);
    }
    assert!(actual[..1_000].iter().flatten().all(|sample| *sample == 0));
    assert!(actual[7_000..].iter().flatten().all(|sample| *sample == 0));
}

#[test]
fn outer_post_gain_changes_keep_inner_factor_and_restore_it_after_lane_ends() {
    let original = pcm(synth(), 127);
    let score = lane(
        constant(synth(), ControlKey::PostGain, ControlValue::Scalar(0.4)),
        ControlKey::PostGain,
        &[
            (Time::ZERO, Time::new(1, 2), ControlValue::Scalar(0.5)),
            (Time::new(1, 2), Time::new(3, 4), ControlValue::Scalar(0.75)),
        ],
    );
    let actual = pcm(score.clone(), 127);
    assert_eq!(actual, pcm(score.clone(), 31));
    assert_eq!(actual, pcm(score, 500));
    for (frame, (actual, original)) in actual.iter().zip(original).enumerate() {
        let factor = if frame < 4_000 {
            0.2
        } else if frame < 6_000 {
            0.3
        } else {
            0.4
        };
        close(
            *actual,
            original.map(|sample| f64::from(sample) * factor),
            frame,
        );
    }
}

fn render_from(score: Score, start: Time, frames: usize) -> Vec<Frame> {
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(RATE, 1024)).unwrap();
    let mut runtime = PlaybackRuntime::new(
        PlaybackSettings {
            cps: Time::ONE,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 64),
        },
        audio,
    );
    runtime
        .play_prepared_score(PreparedScore::new(score).unwrap())
        .unwrap();
    runtime.seek(start).unwrap();
    let mut result = vec![Frame::ZERO; frames];
    for block in result.chunks_mut(31) {
        runtime.tick().unwrap();
        renderer.render(block);
    }
    result
}

#[test]
fn direct_seek_restores_pan_inside_a_lane_and_after_its_end() {
    let full = render_from(timed_pan(), Time::ZERO, 8_000);
    for (start, offset) in [(Time::new(5, 8), 5_000), (Time::new(3, 4), 6_000)] {
        let sought = render_from(timed_pan(), start, 500);
        for (a, b) in sought.iter().zip(&full[offset..]) {
            assert!((a.left - b.left).abs() < 1e-5 && (a.right - b.right).abs() < 1e-5);
        }
    }
}

#[test]
fn pan_requires_finite_bipolar_authored_values() {
    for value in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1.00001,
        1.00001,
    ] {
        assert!(SignedUnitValue::new(value).is_none());
        assert!(
            ControlTile::spanning(
                Time::ZERO,
                Time::ONE,
                ControlKey::Pan,
                ControlValue::Scalar(value)
            )
            .is_err()
        );
    }
    for value in [-1.0, 0.0, 1.0] {
        assert!(
            ControlTile::spanning(Time::ZERO, Time::ONE, ControlKey::Pan, bipolar(value)).is_ok()
        );
    }
    assert!(
        ControlValue::Bool(true)
            .validate_for(&ControlKey::Pan)
            .is_err()
    );
    assert!(
        ControlValue::Ramp {
            from: -1.0,
            to: 1.0
        }
        .validate_for(&ControlKey::Pan)
        .is_err()
    );
}
