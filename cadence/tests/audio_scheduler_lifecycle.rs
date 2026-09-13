//! Expiring scheduler records must not change audible onsets or held tails.
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame, SampleBuffer},
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    prelude::{Intent, PreparedScore, Score, Tile, Time, Voice, WeightedScore},
};

fn runtime(cps: Time) -> (PlaybackRuntime, AudioRenderer) {
    let (audio, renderer) = AudioRenderer::split(AudioRendererSettings::new(8_000, 256)).unwrap();
    let runtime = PlaybackRuntime::new(
        PlaybackSettings {
            cps,
            look_ahead: Time::new(1, 4),
            step: Time::new(1, 32),
        },
        audio,
    );
    runtime.load_sample(
        "pulse",
        SampleBuffer::new(8_000, vec![Frame::from_mono(0.25); 32_000]),
    );
    runtime.load_sample(
        "silent",
        SampleBuffer::new(8_000, vec![Frame::ZERO; 32_000]),
    );
    (runtime, renderer)
}
fn render(
    runtime: &mut PlaybackRuntime,
    renderer: &mut AudioRenderer,
    frames: usize,
) -> Vec<Frame> {
    let mut output = vec![Frame::ZERO; frames];
    for block in output.chunks_mut(61) {
        runtime.tick().unwrap();
        renderer.render(block);
    }
    output
}

#[test]
fn long_repetition_seek_stop_and_resume_keep_exact_audio_onsets() {
    let score = Score::from(
        Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::new(1, 8), Intent::sample("pulse")).unwrap()],
        )
        .unwrap(),
    );
    let (mut runtime, mut renderer) = runtime(Time::whole_number(8));
    runtime
        .play_prepared_score(PreparedScore::new(score).unwrap())
        .unwrap();
    // At eight cycles/sec each cycle is exactly 1000 frames, with a 125-frame gate.
    let output = render(&mut runtime, &mut renderer, 128_000);
    for (index, frame) in output.iter().enumerate() {
        let expected = if index % 1000 < 125 {
            Frame::from_mono(0.25)
        } else {
            Frame::ZERO
        };
        assert_eq!(
            *frame, expected,
            "wrong onset, duplicate voice or tail at frame {index}"
        );
    }
    runtime.seek(Time::new(127, 2)).unwrap();
    let sought = render(&mut runtime, &mut renderer, 1000);
    assert!(sought[..500].iter().all(|frame| *frame == Frame::ZERO));
    assert_eq!(&sought[500..625], &output[..125]);
    assert!(sought[625..].iter().all(|frame| *frame == Frame::ZERO));
    runtime.stop().unwrap();
    assert!(
        render(&mut runtime, &mut renderer, 80)
            .iter()
            .all(|frame| *frame == Frame::ZERO)
    );
    runtime.resume().unwrap();
    assert_eq!(render(&mut runtime, &mut renderer, 1000), output[..1000]);
}

#[test]
fn slow_note_across_hidden_slots_sounds_once_and_continues_until_its_end() {
    fn one(name: &str) -> Score {
        Score::from(
            Voice::new(
                Time::ONE,
                vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample(name)).unwrap()],
            )
            .unwrap(),
        )
    }
    let score = Score::weighted_cycle_slots(vec![
        WeightedScore::new(Score::time_scale(one("pulse"), Time::new(1, 2)), Time::ONE),
        WeightedScore::new(one("silent"), Time::ONE),
    ]);
    let (mut runtime, mut renderer) = runtime(Time::ONE);
    runtime
        .play_prepared_score(PreparedScore::new(score).unwrap())
        .unwrap();
    let output = render(&mut runtime, &mut renderer, 32_000);
    for (index, frame) in output.iter().enumerate() {
        // A slow note spans 1.5 cycles in this alternating half-cycle layout.
        let expected = if index % 16_000 < 12_000 {
            Frame::from_mono(0.25)
        } else {
            Frame::ZERO
        };
        assert_eq!(
            *frame, expected,
            "slow note stopped or restarted at frame {index}"
        );
    }
}
