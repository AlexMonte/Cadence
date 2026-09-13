//! A preview is an independent, bounded mixer slot, even with a full song.
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame, SampleBuffer},
    infrastructure::playback::{
        ChokeGroup, PlaybackRuntime, PlaybackSettings, PlaybackState, SampleLoadOptions,
    },
    prelude::{
        BuiltInSynthSource, ControlKey, ControlScore, ControlTile, ControlTrack, ControlValue,
        Intent, PreparedScore, Score, Tile, Time, Voice,
    },
};
use std::time::Duration;

const RATE: u32 = 8_000;
fn setup() -> (PlaybackRuntime, AudioRenderer) {
    let (audio, renderer) = AudioRenderer::split(AudioRendererSettings::new(RATE, 512)).unwrap();
    let runtime = PlaybackRuntime::new(
        PlaybackSettings {
            cps: Time::ONE,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 64),
        },
        audio,
    );
    for (name, frame) in [
        ("song", Frame::new(0.001, 0.0)),
        ("positive", Frame::new(0.0, 0.5)),
        ("negative", Frame::new(0.0, -0.5)),
    ] {
        runtime.load_sample_with_options(
            name,
            SampleBuffer::new(RATE, vec![frame; 16000]),
            SampleLoadOptions {
                choke_group: Some(ChokeGroup::Hat),
                ..Default::default()
            },
        );
    }
    (runtime, renderer)
}
fn render(runtime: &mut PlaybackRuntime, renderer: &mut AudioRenderer, count: usize) -> Vec<Frame> {
    let mut output = vec![Frame::ZERO; count];
    for block in output.chunks_mut(61) {
        runtime.tick().unwrap();
        renderer.render(block);
    }
    output
}

#[test]
fn previews_replace_each_other_expire_and_leave_stopped_transport_untouched() {
    let (mut runtime, mut renderer) = setup();
    for _ in 0..64 {
        runtime
            .audition(Intent::sample("positive"), None, Duration::from_millis(100))
            .unwrap();
    }
    runtime
        .audition(Intent::sample("negative"), None, Duration::from_millis(100))
        .unwrap();
    let output = render(&mut runtime, &mut renderer, 1000);
    assert_eq!(runtime.status().state, PlaybackState::Stopped);
    assert_eq!(runtime.status().cycle_position, Time::ZERO);
    assert!(
        (output[100].right + 0.325).abs() < 1.0e-6,
        "only the final audition may sound"
    );
    assert!(output.iter().all(|frame| frame.left == 0.0));
    assert!(output[800..].iter().all(|frame| *frame == Frame::ZERO));
    runtime
        .audition(Intent::sample("positive"), None, Duration::from_secs(2))
        .unwrap();
    assert!(render(&mut runtime, &mut renderer, 100)[80].right > 0.0);
    runtime.stop_audition().unwrap();
    runtime.stop_audition().unwrap();
    assert!(
        render(&mut runtime, &mut renderer, 100)
            .iter()
            .all(|frame| *frame == Frame::ZERO)
    );
    runtime
        .audition(
            Intent::synth(BuiltInSynthSource::Sine),
            Some(60.0),
            Duration::from_secs(1),
        )
        .unwrap();
    assert!(
        render(&mut runtime, &mut renderer, 100)
            .iter()
            .any(|frame| frame.left.abs() > 0.01)
    );
    runtime.panic().unwrap();
    assert!(
        render(&mut runtime, &mut renderer, 100)
            .iter()
            .all(|frame| *frame == Frame::ZERO)
    );
}

#[test]
fn audition_does_not_choke_a_song_sample_or_change_its_transport() {
    let (mut runtime, mut renderer) = setup();
    let score = Score::from(
        Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample("song")).unwrap()],
        )
        .unwrap(),
    );
    runtime
        .play_prepared_score(PreparedScore::new(score).unwrap())
        .unwrap();
    let reference = render(&mut runtime, &mut renderer, 100);
    let before = runtime.status();
    runtime
        .audition(Intent::sample("positive"), None, Duration::from_secs(1))
        .unwrap();
    assert_eq!(runtime.status(), before);
    let mixed = render(&mut runtime, &mut renderer, 1000);
    assert!(
        mixed.iter().all(|frame| frame.left == reference[80].left),
        "audition must not choke the song's hat group"
    );
    assert!(mixed[100].right > 0.0);
    runtime.stop_audition().unwrap();
    let stopped = render(&mut runtime, &mut renderer, 1000);
    assert!(
        stopped
            .iter()
            .all(|frame| frame.left == reference[80].left && frame.right == 0.0)
    );
    assert_eq!(runtime.status().cycle_position, Time::new(21, 80));
    assert_eq!(runtime.status().state, PlaybackState::Playing);
}

#[test]
fn audition_at_full_synth_polyphony_does_not_steal_a_song_voice() {
    fn score() -> Score {
        Score::merge(
            (0..8)
                .map(|index| {
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
                        .unwrap(),
                    );
                    Score::with_controls(
                        source,
                        ControlScore::from(
                            ControlTrack::new(
                                Time::ONE,
                                vec![
                                    ControlTile::spanning(
                                        Time::ZERO,
                                        Time::ONE,
                                        ControlKey::Pitch,
                                        ControlValue::Scalar(48.0 + index as f64),
                                    )
                                    .unwrap(),
                                    ControlTile::spanning(
                                        Time::ZERO,
                                        Time::ONE,
                                        ControlKey::Gain,
                                        ControlValue::Scalar(0.03),
                                    )
                                    .unwrap(),
                                ],
                            )
                            .unwrap(),
                        ),
                    )
                })
                .collect(),
        )
    }
    let (mut baseline, mut baseline_renderer) = setup();
    let (mut auditioned, mut auditioned_renderer) = setup();
    let score = score();
    baseline
        .play_prepared_score(PreparedScore::new(score.clone()).unwrap())
        .unwrap();
    auditioned
        .play_prepared_score(PreparedScore::new(score).unwrap())
        .unwrap();
    render(&mut baseline, &mut baseline_renderer, 200);
    render(&mut auditioned, &mut auditioned_renderer, 200);
    auditioned
        .audition(
            Intent::synth(BuiltInSynthSource::Square),
            Some(84.0),
            Duration::from_millis(100),
        )
        .unwrap();
    let reference = render(&mut baseline, &mut baseline_renderer, 1600);
    let mixed = render(&mut auditioned, &mut auditioned_renderer, 1600);
    assert_ne!(mixed[100], reference[100]);
    assert_eq!(
        &mixed[800..],
        &reference[800..],
        "all original synths must continue with unchanged phase"
    );
}

#[test]
fn invalid_preview_requests_are_errors_and_leave_current_preview_playing() {
    let (mut runtime, mut renderer) = setup();
    runtime
        .audition(Intent::sample("positive"), None, Duration::from_secs(1))
        .unwrap();
    for (intent, pitch, duration) in [
        (Intent::sample("positive"), None, Duration::ZERO),
        (Intent::sample("positive"), None, Duration::from_secs(3)),
        (
            Intent::synth(BuiltInSynthSource::Sine),
            Some(f64::NAN),
            Duration::from_secs(1),
        ),
        (
            Intent::toggle("unsupported", true),
            None,
            Duration::from_secs(1),
        ),
        (Intent::sample("missing"), None, Duration::from_secs(1)),
    ] {
        assert!(runtime.audition(intent, pitch, duration).is_err());
    }
    assert!(render(&mut runtime, &mut renderer, 100)[80].right > 0.0);
}
