use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
    infrastructure::{
        audio::SampleBuffer,
        playback::{PlaybackRuntime, PlaybackSettings},
        score::{Score, Time},
        voice::{Intent, Tile, Voice},
    },
    prelude::PreparedScore,
};

#[test]
fn audio_renderer_pipeline_runs_without_panic() {
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(44_100, 256)).expect("renderer");
    let mut runtime = PlaybackRuntime::new(PlaybackSettings::default(), audio);
    runtime.load_sample(
        "a3",
        SampleBuffer::new(44_100, vec![Frame::from_mono(0.5); 256]),
    );
    let voice = Voice::new(
        Time::ONE,
        vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample("a3")).expect("sample tile")],
    )
    .expect("voice");
    runtime
        .play_prepared_score(PreparedScore::new(Score::from(voice)).expect("prepare score"))
        .expect("score");
    let _ = runtime.tick();
    renderer.on_start_processing();
    let mut block = vec![Frame::ZERO; 256];
    renderer.render(&mut block);
    assert!(
        block.iter().any(|frame| *frame != Frame::ZERO),
        "expected audible output after tick + render"
    );
}
