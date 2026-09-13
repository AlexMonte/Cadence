//! Sampler meaning verified through scheduled stereo output, not only metadata.
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame, SampleBuffer},
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings, SampleBank, SampleLoadOptions},
    prelude::{
        ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue, Intent, PreparedScore,
        Repeat, SampleIntent, Score, Symbol, Tile, Time, Voice,
    },
};

const RATE: u32 = 8_000;

#[test]
fn internal_sustain_loop_plays_attack_once_at_tuned_pitch_and_survives_seek() {
    use cadence::adapter::sample_bank::SampleLoopRegion;
    let bank = SampleBank::new();
    bank.load_with_options(
        "source",
        SampleBuffer::new(
            RATE,
            [0.8, 0.6, 0.1, 0.2, 0.3, 0.4, 0.9, 0.9]
                .into_iter()
                .map(Frame::from_mono)
                .collect::<Vec<_>>(),
        ),
        SampleLoadOptions {
            root_pitch: Some(60.25),
            sustain_loop: SampleLoopRegion::new(2, 6),
            ..Default::default()
        },
    );
    let score = control(
        sample(SampleIntent::new("source"), Time::ONE),
        ControlKey::Pitch,
        ControlValue::Scalar(60.25),
    );
    let output = render(score.clone(), bank.clone(), Time::ZERO, 800, 61);
    let expected = [0.8, 0.6, 0.1, 0.2, 0.3, 0.4, 0.1, 0.2, 0.3, 0.4];
    for (frame, expected) in output.iter().zip(expected) {
        assert!((frame.left - expected).abs() < 1e-6);
    }
    assert!(
        output[2..].iter().all(|frame| frame.left < 0.5),
        "the attack and post-loop tail must never repeat"
    );
    assert_eq!(
        output[400..],
        render(score.clone(), bank.clone(), Time::new(1, 20), 400, 37)
    );
    let high = render(
        control(
            sample(SampleIntent::new("source"), Time::ONE),
            ControlKey::Pitch,
            ControlValue::Scalar(72.25),
        ),
        bank.clone(),
        Time::ZERO,
        8,
        3,
    );
    for (frame, expected) in high.iter().zip([0.8, 0.1, 0.3, 0.1, 0.3, 0.1, 0.3, 0.1]) {
        assert!(
            (frame.left - expected).abs() < 1e-6,
            "tuned octave changed the sustain coordinates: {frame:?}"
        );
    }
    let reverse = render(
        sample(SampleIntent::new("source").reverse(true), Time::ONE),
        bank,
        Time::ZERO,
        10,
        7,
    );
    for (frame, expected) in reverse
        .iter()
        .zip([0.9, 0.9, 0.4, 0.3, 0.2, 0.1, 0.4, 0.3, 0.2, 0.1])
    {
        assert!((frame.left - expected).abs() < 1e-6);
    }
}
fn control(score: Score, key: ControlKey, value: ControlValue) -> Score {
    Score::with_controls(
        score,
        ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![ControlTile::spanning(Time::ZERO, Time::ONE, key, value).unwrap()],
            )
            .unwrap(),
        ),
    )
}
fn sample(intent: SampleIntent, end: Time) -> Score {
    Score::from(
        Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, end, Intent::Sample(intent)).unwrap()],
        )
        .unwrap()
        .with_repeat(Repeat::Once),
    )
}
fn bank(frames: Vec<Frame>) -> SampleBank {
    let bank = SampleBank::new();
    bank.load("source", SampleBuffer::new(RATE, frames));
    bank
}
fn render(score: Score, bank: SampleBank, start: Time, frames: usize, block: usize) -> Vec<Frame> {
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(RATE, 1024)).unwrap();
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
    if start != Time::ZERO {
        runtime.seek(start).unwrap();
    }
    let mut output = vec![Frame::ZERO; frames];
    for chunk in output.chunks_mut(block) {
        runtime.tick().unwrap();
        renderer.render(chunk);
    }
    output
}

#[test]
fn rearranged_sixteen_slice_break_has_exact_slots_without_neighbor_bleed() {
    let order = [0, 4, 2, 7, 8, 8, 10, 3, 12, 5, 14, 1, 15, 13, 6, 9];
    let source = bank(
        (0..128)
            .map(|index| {
                Frame::new(
                    (index / 8 + 1) as f32 / 32.0,
                    -(index / 8 + 1) as f32 / 32.0,
                )
            })
            .collect(),
    );
    let tiles = order
        .iter()
        .enumerate()
        .map(|(slot, index)| {
            Tile::spanning(
                Time::new(slot as i64, 16),
                Time::new(slot as i64 + 1, 16),
                Intent::Sample(
                    SampleIntent::new("source")
                        .slice(*index, 16)
                        .unwrap()
                        .reverse(slot % 3 == 0),
                ),
            )
            .unwrap()
        })
        .collect();
    let score = control(
        Score::from(Voice::new(Time::ONE, tiles).unwrap()),
        ControlKey::Fit,
        ControlValue::Bool(true),
    );
    let output = render(score, source, Time::ZERO, 8_000, 61);
    for (frame_index, frame) in output.iter().enumerate() {
        let value = (order[frame_index / 500] + 1) as f32 / 32.0;
        assert!(
            (frame.left - value).abs() < 1.0e-6 && (frame.right + value).abs() < 1.0e-6,
            "slice changed or leaked at frame {frame_index}: {frame:?}"
        );
    }
}

#[test]
fn fit_changes_rate_and_pitch_then_explicit_rate_and_transpose_apply() {
    let frames = (0..800)
        .map(|i| Frame::from_mono(i as f32 / 1600.0))
        .collect();
    let source = bank(frames);
    let plain = sample(SampleIntent::new("source"), Time::new(1, 5));
    let fitted = control(plain.clone(), ControlKey::Fit, ControlValue::Bool(true));
    let stretched = render(fitted.clone(), source.clone(), Time::ZERO, 1800, 61);
    assert_eq!(
        &stretched[400..700],
        render(fitted.clone(), source.clone(), Time::new(1, 20), 300, 61)
    );
    assert!((stretched[1200].left - 0.375).abs() < 1.0e-6);
    assert!(stretched[1600..].iter().all(|frame| *frame == Frame::ZERO));
    let double_rate = render(
        control(
            fitted.clone(),
            ControlKey::PlaybackRate,
            ControlValue::Scalar(2.0),
        ),
        source.clone(),
        Time::ZERO,
        1800,
        127,
    );
    assert_eq!(
        double_rate,
        render(plain, source.clone(), Time::ZERO, 1800, 61)
    );
    assert_eq!(
        double_rate,
        render(
            control(fitted, ControlKey::Transpose, ControlValue::Scalar(12.0)),
            source,
            Time::ZERO,
            1800,
            61
        )
    );
}

#[test]
fn patterned_rate_updates_keep_fitting_and_root_pitch_multipliers() {
    let source = SampleBank::new();
    source.load_with_options(
        "source",
        SampleBuffer::new(
            RATE,
            (0..1600)
                .map(|i| Frame::from_mono(i as f32 / 3200.0))
                .collect::<Vec<_>>(),
        ),
        SampleLoadOptions {
            root_pitch: Some(60.0),
            ..Default::default()
        },
    );
    fn rates(score: Score, before: f64, after: f64) -> Score {
        Score::with_controls(
            score,
            ControlScore::from(
                ControlTrack::new(
                    Time::ONE,
                    vec![
                        ControlTile::spanning(
                            Time::ZERO,
                            Time::new(1, 4),
                            ControlKey::PlaybackRate,
                            ControlValue::Scalar(before),
                        )
                        .unwrap(),
                        ControlTile::spanning(
                            Time::new(1, 4),
                            Time::ONE,
                            ControlKey::PlaybackRate,
                            ControlValue::Scalar(after),
                        )
                        .unwrap(),
                    ],
                )
                .unwrap(),
            ),
        )
    }
    let looped = control(
        sample(SampleIntent::new("source"), Time::ONE),
        ControlKey::Loop,
        ControlValue::Bool(true),
    );
    let fitted = control(
        control(looped.clone(), ControlKey::Fit, ControlValue::Bool(true)),
        ControlKey::Pitch,
        ControlValue::Scalar(72.0),
    );
    assert_eq!(
        render(
            rates(fitted, 1.0, 2.0),
            source.clone(),
            Time::ZERO,
            8000,
            61
        ),
        render(rates(looped, 0.4, 0.8), source, Time::ZERO, 8000, 127),
    );
}

#[test]
fn forward_and_reverse_loops_keep_phase_on_direct_seek_and_stop_at_the_gate() {
    let frames = (0..32)
        .map(|i| Frame::new(i as f32 / 64.0, -i as f32 / 128.0))
        .collect();
    let source = bank(frames);
    for reverse in [false, true] {
        let score = control(
            sample(
                SampleIntent::new("source").reverse(reverse).rate(1.5),
                Time::ONE,
            ),
            ControlKey::Loop,
            ControlValue::Bool(true),
        );
        let output = render(score.clone(), source.clone(), Time::ZERO, 8_100, 61);
        assert_eq!(
            output,
            render(score.clone(), source.clone(), Time::ZERO, 8_100, 127)
        );
        assert_eq!(
            &output[1003..1259],
            render(score, source.clone(), Time::new(1003, 8000), 256, 61)
        );
        assert!(output[8000..].iter().all(|frame| *frame == Frame::ZERO));
        assert!(
            output[7900..8000]
                .iter()
                .any(|frame| frame.left.abs() > 0.1)
        );
    }
}

#[test]
fn ordinary_sample_seek_uses_source_time_instead_of_fraction_of_note_slot() {
    let source = bank(
        (0..8000)
            .map(|i| Frame::from_mono(i as f32 / 16000.0))
            .collect(),
    );
    let score = sample(
        SampleIntent::new("source").region(0.25, 0.75),
        Time::new(1, 2),
    );
    let reference = render(score.clone(), source.clone(), Time::ZERO, 4000, 61);
    let sought = render(score, source, Time::new(1, 4), 500, 61);
    assert_eq!(&reference[2000..2500], sought);
    assert!((sought[0].left - 0.25).abs() < 1.0e-6);
}

#[test]
fn bank_selection_does_not_escape_to_another_bank_without_a_pitch() {
    let bank = SampleBank::new();
    for (name, value) in [("first", 0.1), ("wanted", 0.75)] {
        bank.load_with_options(
            "source",
            SampleBuffer::new(RATE, vec![Frame::from_mono(value); 2000]),
            SampleLoadOptions {
                bank: Some(Symbol::from(name)),
                ..Default::default()
            },
        );
    }
    let score = control(
        sample(SampleIntent::new("source"), Time::new(1, 4)),
        ControlKey::SampleBank,
        ControlValue::Choice(Symbol::from("wanted")),
    );
    assert!(
        render(score, bank, Time::ZERO, 1000, 61)
            .iter()
            .all(|frame| frame.left == 0.75)
    );
}

#[test]
fn slice_selection_and_invalid_fit_bounds_fail_explicitly() {
    assert!(SampleIntent::new("source").slice(0, 0).is_err());
    assert!(SampleIntent::new("source").slice(16, 16).is_err());
    assert!(SampleIntent::new("source").slice(0, 65_537).is_err());
    let slice = SampleIntent::new("source")
        .region(0.25, 0.75)
        .slice(1, 4)
        .unwrap();
    assert_eq!((slice.start, slice.end), (0.375, 0.5));
    let (audio, _) = AudioRenderer::split(AudioRendererSettings::new(RATE, 256)).unwrap();
    let mut runtime = PlaybackRuntime::with_sample_bank(
        PlaybackSettings::default(),
        audio,
        bank(vec![Frame::ZERO; 16]),
    );
    runtime
        .play_prepared_score(
            PreparedScore::new(sample(SampleIntent::new("source").rate(1.0e100), Time::ONE))
                .unwrap(),
        )
        .unwrap();
    let error = runtime.tick().unwrap_err();
    assert!(
        error.to_string().contains("invalid sample playback"),
        "{error}"
    );
    let invalid_region = control(
        control(
            sample(SampleIntent::new("source"), Time::ONE),
            ControlKey::PlaybackStart,
            ControlValue::Scalar(0.75),
        ),
        ControlKey::PlaybackEnd,
        ControlValue::Scalar(0.25),
    );
    runtime
        .play_prepared_score(PreparedScore::new(invalid_region).unwrap())
        .unwrap();
    let error = runtime.tick().unwrap_err();
    assert!(
        error.to_string().contains("sample region must satisfy"),
        "{error}"
    );
}
