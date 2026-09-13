//! Rendered oscillator quality, reproducible noise, and preset meaning.
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    prelude::{
        BuiltInSynthSource, ControlKey, ControlMap, ControlScore, ControlTile, ControlTrack,
        ControlValue, Intent, PreparedScore, Score, SynthPreset, Tile, Time, Voice,
    },
};
use std::f64::consts::TAU;

fn score(intent: Intent, pitch: f64, end: Time, mut controls: ControlMap) -> Score {
    controls.insert(ControlKey::Pitch, ControlValue::Scalar(pitch));
    Score::with_controls(
        Score::from(
            Voice::new(
                Time::whole_number(4),
                vec![Tile::spanning(Time::ZERO, end, intent).unwrap()],
            )
            .unwrap(),
        ),
        ControlScore::from(
            ControlTrack::new(
                Time::whole_number(4),
                controls
                    .into_iter()
                    .map(|(key, value)| ControlTile::spanning(Time::ZERO, end, key, value).unwrap())
                    .collect(),
            )
            .unwrap(),
        ),
    )
}

fn setup(score: Score, rate: u32) -> (PlaybackRuntime, AudioRenderer) {
    let (audio, renderer) = AudioRenderer::split(AudioRendererSettings::new(rate, 256)).unwrap();
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
    (runtime, renderer)
}

fn render(score: Score, rate: u32, count: usize, block: usize) -> Vec<Frame> {
    let (mut runtime, mut renderer) = setup(score, rate);
    render_with(&mut runtime, &mut renderer, count, block)
}

fn render_with(
    runtime: &mut PlaybackRuntime,
    renderer: &mut AudioRenderer,
    count: usize,
    block: usize,
) -> Vec<Frame> {
    let mut frames = vec![Frame::ZERO; count];
    for block in frames.chunks_mut(block) {
        runtime.tick().unwrap();
        renderer.render(block);
    }
    frames
}

fn bin_power(samples: &[f64], bin: usize) -> f64 {
    let (real, imag) =
        samples
            .iter()
            .enumerate()
            .fold((0.0, 0.0), |(real, imag), (index, sample)| {
                let phase = TAU * bin as f64 * index as f64 / samples.len() as f64;
                (real + sample * phase.cos(), imag + sample * phase.sin())
            });
    2.0 * (real * real + imag * imag) / (samples.len() * samples.len()) as f64
}

fn alias_power(samples: &[f64], fundamental: usize) -> f64 {
    let total = samples.iter().map(|sample| sample * sample).sum::<f64>() / samples.len() as f64;
    let harmonics = (fundamental..samples.len() / 2)
        .step_by(fundamental)
        .map(|bin| bin_power(samples, bin))
        .sum::<f64>();
    (total - harmonics).max(0.0)
}

#[test]
fn corrected_waveforms_reduce_nonharmonic_energy_without_losing_note_pitch() {
    const COUNT: usize = 8_192;
    const BIN: usize = 521;
    for rate in [44_100, 48_000, 96_000] {
        let frequency = f64::from(rate) * BIN as f64 / COUNT as f64;
        let pitch = 69.0 + 12.0 * (frequency / 440.0).log2();
        for source in [
            BuiltInSynthSource::Saw,
            BuiltInSynthSource::Square,
            BuiltInSynthSource::Triangle,
        ] {
            let actual = render(
                score(Intent::synth(source), pitch, Time::ONE, ControlMap::new()),
                rate,
                COUNT,
                61,
            );
            let actual: Vec<_> = actual.iter().map(|frame| f64::from(frame.left)).collect();
            let naive: Vec<_> = (0..COUNT)
                .map(|index| {
                    let phase = (index as f64 * BIN as f64 / COUNT as f64).fract();
                    match source {
                        BuiltInSynthSource::Saw => 2.0 * phase - 1.0,
                        BuiltInSynthSource::Square => {
                            if phase < 0.5 {
                                1.0
                            } else {
                                -1.0
                            }
                        }
                        BuiltInSynthSource::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                        _ => unreachable!(),
                    }
                })
                .collect();
            let corrected_alias = alias_power(&actual, BIN);
            let naive_alias = alias_power(&naive, BIN);
            assert!(
                corrected_alias < naive_alias * 0.08,
                "{source:?} at {rate}: corrected {corrected_alias}, naive {naive_alias}"
            );
            assert!(
                bin_power(&actual, BIN) > bin_power(&naive, BIN) * 0.9,
                "alias reduction must retain the requested fundamental"
            );
            assert!(bin_power(&actual, BIN) > bin_power(&actual, BIN + 1) * 1_000.0);
        }
    }
}

#[test]
fn oscillator_edges_are_continuous_and_out_of_band_notes_stay_silent() {
    for source in [
        BuiltInSynthSource::Sine,
        BuiltInSynthSource::Saw,
        BuiltInSynthSource::Square,
        BuiltInSynthSource::Triangle,
    ] {
        let frames = render(
            score(Intent::synth(source), 69.0, Time::ONE, ControlMap::new()),
            48_000,
            4_800,
            127,
        );
        assert!(
            frames
                .iter()
                .all(|frame| frame.left.is_finite() && frame.left.abs() <= 1.001)
        );
        let largest_step = frames
            .windows(2)
            .map(|frames| (frames[1].left - frames[0].left).abs())
            .fold(0.0_f32, f32::max);
        assert!(
            largest_step <= 1.51,
            "{source:?} still has a full discontinuity: {largest_step}"
        );
        let above_nyquist = render(
            score(Intent::synth(source), 127.0, Time::ONE, ControlMap::new()),
            8_000,
            512,
            61,
        );
        assert!(above_nyquist.iter().all(|frame| *frame == Frame::ZERO));
    }
}

#[test]
fn noise_is_repeatable_across_blocks_new_scores_and_direct_seeks() {
    let make_score = |pitch| {
        score(
            Intent::synth(BuiltInSynthSource::Noise),
            pitch,
            Time::ONE,
            ControlMap::new(),
        )
    };
    let baseline = render(make_score(60.0), 8_000, 4_096, 61);
    assert_eq!(
        baseline,
        render(make_score(90.0), 8_000, 4_096, 257),
        "unpitched noise must not depend on pitch, identities or block size"
    );
    let (mut runtime, mut renderer) = setup(make_score(60.0), 8_000);
    runtime.seek(Time::new(1_033, 8_000)).unwrap();
    assert_eq!(
        render_with(&mut runtime, &mut renderer, 512, 127),
        baseline[1_033..1_545]
    );
    let values: Vec<_> = baseline.iter().map(|frame| f64::from(frame.left)).collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let power = values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64;
    let correlation =
        values.windows(2).map(|pair| pair[0] * pair[1]).sum::<f64>() / values.len() as f64;
    assert!(mean.abs() < 0.03 && (0.30..0.37).contains(&power));
    assert!(
        correlation.abs() < 0.03,
        "noise must not repeat a short tonal cycle"
    );
}

#[test]
fn presets_reload_as_explicit_sound_defaults_and_authored_controls_win() {
    for preset in [SynthPreset::Bass, SynthPreset::Pad, SynthPreset::Percussion] {
        let end = Time::new(1, 2);
        let defaults = preset.controls();
        let named = render(
            score(Intent::synth_preset(preset), 48.0, end, ControlMap::new()),
            8_000,
            8_000,
            61,
        );
        let explicit = render(
            score(Intent::synth(preset.source()), 48.0, end, defaults.clone()),
            8_000,
            8_000,
            257,
        );
        assert_eq!(
            named, explicit,
            "{preset:?} must have reconstructable, inspectable meaning"
        );
        assert!(named.iter().any(|frame| frame.left.abs() > 0.02));
        assert!(named[7_300..].iter().all(|frame| frame.left.abs() < 1.0e-6));
        assert_eq!(
            named[0],
            Frame::ZERO,
            "presets must start with a shaped attack"
        );
        let authored = [
            (ControlKey::Attack, ControlValue::Scalar(0.025)),
            (ControlKey::Gain, ControlValue::Scalar(0.1)),
            (ControlKey::LowPassCutoff, ControlValue::Scalar(1_200.0)),
        ]
        .into_iter()
        .collect::<ControlMap>();
        let mut merged = defaults;
        merged.extend(authored.clone());
        assert_eq!(
            render(
                score(Intent::synth_preset(preset), 72.0, end, authored),
                8_000,
                8_000,
                61
            ),
            render(
                score(Intent::synth(preset.source()), 72.0, end, merged),
                8_000,
                8_000,
                127
            ),
        );
    }
}

#[test]
fn pad_has_an_audible_release_and_noise_percussion_decays_before_the_slot_ends() {
    let pad = render(
        score(
            Intent::synth_preset(SynthPreset::Pad),
            60.0,
            Time::new(1, 2),
            ControlMap::new(),
        ),
        8_000,
        8_000,
        61,
    );
    assert!(
        pad[4_400..5_200]
            .iter()
            .any(|frame| frame.left.abs() > 0.02)
    );
    assert!(pad[7_300..].iter().all(|frame| frame.left.abs() < 1.0e-6));
    let percussion = render(
        score(
            Intent::synth_preset(SynthPreset::Percussion),
            60.0,
            Time::ONE,
            ControlMap::new(),
        ),
        8_000,
        4_000,
        61,
    );
    let energy = |frames: &[Frame]| {
        frames
            .iter()
            .map(|frame| frame.left * frame.left)
            .sum::<f32>()
            / frames.len() as f32
    };
    assert!(energy(&percussion[80..400]) > 0.001);
    assert!(energy(&percussion[1_200..2_000]) < 1.0e-10);
}

#[test]
fn changing_note_controls_keeps_preset_defaults_and_audition_keeps_its_envelope() {
    let transpose = ControlScore::from(
        ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::Transpose,
                    ControlValue::Scalar(0.0),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::Transpose,
                    ControlValue::Scalar(12.0),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    );
    let named = Score::with_controls(
        score(
            Intent::synth_preset(SynthPreset::Pad),
            60.0,
            Time::ONE,
            ControlMap::new(),
        ),
        transpose.clone(),
    );
    let explicit = Score::with_controls(
        score(
            Intent::synth(SynthPreset::Pad.source()),
            60.0,
            Time::ONE,
            SynthPreset::Pad.controls(),
        ),
        transpose,
    );
    assert_eq!(
        render(named, 8_000, 8_000, 61),
        render(explicit, 8_000, 8_000, 127)
    );

    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8_000, 64)).unwrap();
    let mut runtime = PlaybackRuntime::new(PlaybackSettings::default(), audio);
    runtime
        .audition(
            Intent::synth_preset(SynthPreset::Pad),
            Some(60.0),
            std::time::Duration::from_secs(1),
        )
        .unwrap();
    let preview = render_with(&mut runtime, &mut renderer, 8_128, 61);
    let reference = render(
        score(
            Intent::synth_preset(SynthPreset::Pad),
            60.0,
            Time::new(3, 5),
            ControlMap::new(),
        ),
        8_000,
        8_128,
        127,
    );
    assert!(
        preview
            .iter()
            .zip(reference)
            .all(|(preview, reference)| (preview.left - reference.left * 0.65).abs() < 1.0e-6)
    );
    assert!(preview[8_000..].iter().all(|frame| *frame == Frame::ZERO));
}
