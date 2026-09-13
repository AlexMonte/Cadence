//! A saved sound has the same envelope/sends during live and scheduled playback.
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame, SampleBuffer},
    application::audio::AudioVoicePlan,
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    prelude::*,
};
use std::time::Duration;

fn settings() -> SoundDefaults {
    SoundDefaults::default()
        .with_envelope(EnvelopeDefaults::new(
            Duration::from_millis(80),
            Duration::from_millis(100),
            UnitValue::new(0.4).unwrap(),
            Duration::from_millis(150),
        ))
        .with_gain(0.25)
        .unwrap()
        .with_delay(DelaySettings::new(
            UnitValue::new(0.2).unwrap(),
            Duration::from_millis(90),
            UnitValue::new(0.3).unwrap(),
            UnitValue::new(0.5).unwrap(),
        ))
        .with_compressor(CompressorSettings::new(
            UnitValue::new(0.5).unwrap(),
            4.0,
            Duration::from_millis(10),
            Duration::from_millis(100),
        ))
        .with_reverb(ReverbSettings::new(
            UnitValue::new(0.15).unwrap(),
            Duration::from_millis(900),
            UnitValue::new(0.4).unwrap(),
        ))
}
fn intent(sample: bool, defaults: SoundDefaults) -> Intent {
    if sample {
        Intent::Sample(SampleIntent::new("tone").with_sound_defaults(defaults))
    } else {
        Intent::Synth(SynthIntent::preset(SynthPreset::Bass).with_sound_defaults(defaults))
    }
}

#[test]
fn caller_controls_override_sound_defaults_for_both_live_sources() {
    for sample in [true, false] {
        let intent = intent(sample, settings());
        let controls = [
            (ControlKey::Attack, ControlValue::Scalar(0.01)),
            (ControlKey::Gain, ControlValue::Scalar(0.5)),
        ]
        .into_iter()
        .collect();
        let id = cadence::application::audio::VoiceInstanceId::new(17);
        let plan = match intent {
            Intent::Sample(sample) => AudioVoicePlan::from_live_sample(
                id,
                &sample,
                &controls,
                None,
                None,
                Duration::from_secs(1),
            ),
            Intent::Synth(synth) => AudioVoicePlan::from_live_synth_intent(
                id,
                &synth,
                &controls,
                None,
                None,
                Duration::from_secs(1),
            ),
            _ => unreachable!(),
        }
        .unwrap();
        assert_eq!(plan.envelope.attack, Duration::from_millis(10));
        assert_eq!(plan.envelope.decay, Duration::from_millis(100));
        assert_eq!(plan.envelope.sustain_level.value(), 0.4);
        assert_eq!(plan.envelope.release, Duration::from_millis(150));
        assert_eq!(plan.mix.gain, 0.5);
        assert_eq!(plan.dynamics.compressor.unwrap().ratio(), 4.0);
        assert_eq!(plan.sends.delay.unwrap().time(), Duration::from_millis(90));
        assert_eq!(
            plan.sends.reverb.unwrap().decay(),
            Duration::from_millis(900)
        );
    }
}

fn render(intent: Intent, audition: bool) -> Vec<Frame> {
    let (handle, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8_000, 512)).unwrap();
    let mut runtime = PlaybackRuntime::new(
        PlaybackSettings {
            cps: Time::ONE,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 64),
        },
        handle,
    );
    runtime.load_sample(
        "tone",
        SampleBuffer::new(8_000, vec![Frame::from_mono(0.3); 8_000]),
    );
    if audition {
        runtime
            .audition(intent, Some(60.0), Duration::from_secs(1))
            .unwrap();
    } else {
        let voice = Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::ONE, intent).unwrap()],
        )
        .unwrap()
        .with_repeat(Repeat::Once);
        runtime
            .play_prepared_score(
                PreparedScore::new(Score::with_controls(
                    Score::from(voice),
                    ControlScore::from(
                        ControlTrack::new(
                            Time::ONE,
                            vec![
                                ControlTile::spanning(
                                    Time::ZERO,
                                    Time::ONE,
                                    ControlKey::Pitch,
                                    ControlValue::Scalar(60.0),
                                )
                                .unwrap(),
                            ],
                        )
                        .unwrap(),
                    ),
                ))
                .unwrap(),
            )
            .unwrap();
    }
    let mut frames = vec![Frame::ZERO; 4_000];
    for block in frames.chunks_mut(61) {
        runtime.tick().unwrap();
        renderer.render(block);
    }
    frames
}
#[test]
fn saved_sound_changes_scheduled_audio_and_audition_for_sample_and_synth() {
    for sample in [true, false] {
        for audition in [true, false] {
            let base = render(intent(sample, SoundDefaults::default()), audition);
            let changed = render(intent(sample, settings()), audition);
            assert_ne!(base, changed);
            assert!(changed.iter().any(|frame| frame.left.abs() > 0.0001));
            assert!(
                changed
                    .iter()
                    .all(|frame| frame.left.is_finite() && frame.right.is_finite())
            );
            assert_eq!(changed, render(intent(sample, settings()), audition));
        }
    }
}

#[test]
fn sound_gain_rejects_nonfinite_and_negative_values() {
    for gain in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.1] {
        assert!(SoundDefaults::default().with_gain(gain).is_err());
    }
}
