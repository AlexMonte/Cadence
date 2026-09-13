//! Legato extends rendered sound without moving the next authored onset.
//! Separate stereo sides distinguish an overlapping tail from the next voice.

use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame, SampleBuffer},
    domain::space::{Point3, SpatialMotion},
    infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    prelude::{
        BuiltInSynthSource, CadenceCompiler, ControlKey, ControlScore, ControlTile, ControlTrack,
        ControlValue, Coord, Intent, PreparedScore, Score, Span, Tile, Time, Voice,
    },
};

const RATE: u32 = 8_000;
const NEXT_ONSET_FRAME: usize = 1_200; // 3/20 cycle, with one cycle per second.

fn voice(intent: Intent, start: Time, end: Time, side: i64, legato: f64) -> Score {
    let tile = Tile::spanning(start, end, intent)
        .unwrap()
        .with_position(SpatialMotion::Static(Point3::new(
            Coord::new(side, 1),
            Coord::ZERO,
            Coord::ZERO,
        )));
    let source = Score::from(Voice::new(Time::ONE, vec![tile]).unwrap());
    let controls = ControlTrack::new(
        Time::ONE,
        vec![
            ControlTile::spanning(
                Time::ZERO,
                Time::ONE,
                ControlKey::Legato,
                ControlValue::Scalar(legato),
            )
            .unwrap(),
            ControlTile::spanning(
                Time::ZERO,
                Time::ONE,
                ControlKey::Gain,
                ControlValue::Scalar(0.2),
            )
            .unwrap(),
            ControlTile::spanning(
                Time::ZERO,
                Time::ONE,
                ControlKey::Release,
                ControlValue::Scalar(0.0),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    Score::with_controls(source, ControlScore::track(controls))
}

fn pattern(intent: Intent, legato: f64) -> Score {
    Score::merge(vec![
        voice(intent.clone(), Time::ZERO, Time::new(1, 10), -1, legato),
        voice(intent, Time::new(3, 20), Time::new(1, 4), 1, 1.0),
    ])
}

fn render(score: Score) -> Vec<Frame> {
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(RATE, 128)).unwrap();
    let mut runtime = PlaybackRuntime::new(
        PlaybackSettings {
            cps: Time::ONE,
            look_ahead: Time::new(1, 2),
            step: Time::new(1, 32),
        },
        audio,
    );
    // One full second keeps sample exhaustion outside both 100ms and 200ms gates.
    runtime.load_sample(
        "long_tone",
        SampleBuffer::new(
            RATE,
            (0..RATE)
                .map(|index| {
                    let phase = std::f64::consts::TAU * 220.0 * f64::from(index) / f64::from(RATE);
                    Frame::from_mono(phase.sin() as f32 * 0.5)
                })
                .collect::<Vec<_>>(),
        ),
    );
    runtime
        .play_prepared_score(PreparedScore::new(score).unwrap())
        .unwrap();
    let mut frames = vec![Frame::ZERO; 2_400];
    for block in frames.chunks_mut(64) {
        runtime.tick().unwrap();
        renderer.render(block);
    }
    frames
}

fn mean_square_left(frames: &[Frame]) -> f64 {
    frames
        .iter()
        .map(|frame| f64::from(frame.left).powi(2))
        .sum::<f64>()
        / frames.len() as f64
}

fn assert_legato_sound_and_onsets(intent: Intent) {
    let normal_score = pattern(intent.clone(), 1.0);
    let extended_score = pattern(intent, 2.0);
    let window = Span::new(Time::ZERO, Time::ONE).unwrap();
    let compiler = CadenceCompiler::new();
    for score in [&normal_score, &extended_score] {
        let prepared = PreparedScore::new(score.clone()).unwrap();
        let report = compiler.preview(&prepared, &window).unwrap();
        let mut spans = report
            .starts()
            .map(|event| {
                let moment = event.projected();
                (moment.whole().start(), moment.whole().end())
            })
            .collect::<Vec<_>>();
        spans.sort();
        assert_eq!(
            spans,
            vec![
                (Time::ZERO, Time::new(1, 10)),
                (Time::new(3, 20), Time::new(1, 4))
            ]
        );
    }
    let normal = render(normal_score);
    let extended = render(extended_score);
    assert!(
        mean_square_left(&normal[1_000..1_150]) < 1.0e-12,
        "normal gate has ended before the next onset"
    );
    assert!(
        mean_square_left(&extended[1_000..1_150]) > 1.0e-5,
        "Legato2 must sound beyond the authored 100ms span"
    );
    assert!(
        mean_square_left(&extended[1_300..1_500]) > 1.0e-5,
        "extended voice must overlap the following voice"
    );
    assert!(
        mean_square_left(&extended[1_800..1_950]) < 1.0e-12,
        "the extended200ms gate must still end"
    );

    for output in [&normal, &extended] {
        assert!(
            output[..NEXT_ONSET_FRAME]
                .iter()
                .all(|frame| frame.right.abs() < 1.0e-7)
        );
        let first = output
            .iter()
            .position(|frame| frame.right.abs() > 1.0e-5)
            .expect("next voice is audible");
        assert!(
            (NEXT_ONSET_FRAME..=NEXT_ONSET_FRAME + 2).contains(&first),
            "next voice began at frame {first}"
        );
    }
    let difference = normal
        .iter()
        .zip(&extended)
        .map(|(a, b)| (a.right - b.right).abs())
        .fold(0.0f32, f32::max);
    assert!(
        difference < 1.0e-6,
        "Legato on the left voice changed the right voice: {difference}"
    );
}

#[test]
fn synth_legato_extends_sound_and_keeps_the_next_note_on_time() {
    assert_legato_sound_and_onsets(Intent::synth(BuiltInSynthSource::Sine));
}

#[test]
fn long_sample_legato_extends_sound_and_keeps_the_next_note_on_time() {
    assert_legato_sound_and_onsets(Intent::sample("long_tone"));
}
