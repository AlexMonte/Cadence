use std::time::Duration;

use cadence::infrastructure::{
    CadenceCompiler, PreviewReport,
    audio::{
        AudioRenderer, AudioRendererSettings, AudioTrigger, AudioTriggerResolver, Frame,
        SampleBuffer, audio_trigger_channel,
    },
    playback::{PlaybackRuntime, PlaybackSettings, PlaybackState, SampleBank},
    projection::{ControlKey, ControlValue, TransportSpan},
    render::RendererCore,
    score::{ControlScore, ControlTile, ControlTrack, Score, Time, UnitValue},
    voice::{BuiltInSynthSource, Intent, Tile, Voice},
};
use cadence::{
    application::audio::{AudioVoicePlan, VoiceInstanceId},
    domain::control::ControlMap,
};
use cadence::{
    application::{EvaluatedEvent, EvaluatedEventKind},
    domain::control::SignedUnitValue,
};

fn sample_voice(sample: &str) -> Voice {
    Voice::new(
        Time::ONE,
        vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample(sample)).unwrap()],
    )
    .unwrap()
}

fn synth_voice(source: BuiltInSynthSource) -> Voice {
    Voice::new(
        Time::ONE,
        vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::synth(source)).unwrap()],
    )
    .unwrap()
}

fn mixed_score() -> Score {
    Score::merge(vec![
        Score::from(sample_voice("bd")),
        Score::from(synth_voice(BuiltInSynthSource::Sine)),
    ])
}

fn controlled_sample_score() -> Score {
    let controls = ControlTrack::new(
        Time::ONE,
        vec![
            ControlTile::spanning(
                Time::ZERO,
                Time::ONE,
                ControlKey::Gain,
                ControlValue::Ramp { from: 1.0, to: 0.0 },
            )
            .unwrap(),
        ],
    )
    .unwrap();

    Score::with_controls(
        Score::from(sample_voice("pad")),
        ControlScore::track(controls),
    )
}

fn sample_buffer() -> SampleBuffer {
    SampleBuffer::new(
        8_000,
        vec![
            Frame::from_mono(0.0),
            Frame::from_mono(0.5),
            Frame::from_mono(-0.25),
            Frame::from_mono(0.25),
        ],
    )
}

fn render_is_audible(renderer: &mut AudioRenderer, frames: usize) -> bool {
    renderer.on_start_processing();
    let mut out = vec![Frame::ZERO; frames];
    renderer.render(&mut out);
    out.iter().any(|frame| *frame != Frame::ZERO)
}

#[test]
fn host_can_route_loaded_sample_triggers_through_infrastructure_audio() {
    let bank = SampleBank::new();
    bank.load("bd", sample_buffer());

    let (sender, receiver) = audio_trigger_channel();
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8_000, 64)).unwrap();
    let mut resolver = AudioTriggerResolver::new(receiver, bank, audio);

    sender
        .send(AudioTrigger::StartVoice(
            AudioVoicePlan::from_live_sample(
                VoiceInstanceId::next_live(),
                &cadence::domain::intent::SampleIntent::new("bd"),
                &ControlMap::new(),
                None,
                None,
                Duration::ZERO,
            )
            .unwrap(),
        ))
        .unwrap();

    assert!(resolver.tick());
    assert!(render_is_audible(&mut renderer, 32));
}

#[test]
fn host_can_route_synth_triggers_without_sample_manifest_loading() {
    let bank = SampleBank::new();

    let (sender, receiver) = audio_trigger_channel();
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8_000, 64)).unwrap();
    let mut resolver = AudioTriggerResolver::new(receiver, bank, audio);

    sender
        .send(AudioTrigger::StartVoice(
            AudioVoicePlan::from_live_synth(
                VoiceInstanceId::next_live(),
                BuiltInSynthSource::Triangle,
                &ControlMap::from([(ControlKey::Pitch, ControlValue::Scalar(60.0))]),
                Some(UnitValue::new(1.0).unwrap()),
                None,
                Duration::ZERO,
            )
            .unwrap(),
        ))
        .unwrap();

    assert!(resolver.tick());
    assert!(render_is_audible(&mut renderer, 64));
}

#[test]
fn host_can_route_mixed_audio_triggers_through_infrastructure_audio() {
    let bank = SampleBank::new();
    bank.load("bd", sample_buffer());

    let (sender, receiver) = audio_trigger_channel();
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8_000, 64)).unwrap();
    let mut resolver = AudioTriggerResolver::new(receiver, bank, audio);

    sender
        .send(AudioTrigger::StartVoice(
            AudioVoicePlan::from_live_sample(
                VoiceInstanceId::next_live(),
                &cadence::domain::intent::SampleIntent::new("bd"),
                &ControlMap::new(),
                None,
                None,
                Duration::ZERO,
            )
            .unwrap(),
        ))
        .unwrap();
    sender
        .send(AudioTrigger::StartVoice(
            AudioVoicePlan::from_live_synth(
                VoiceInstanceId::next_live(),
                BuiltInSynthSource::Square,
                &ControlMap::from([(ControlKey::Pitch, ControlValue::Scalar(67.0))]),
                Some(UnitValue::new(1.0).unwrap()),
                None,
                Duration::ZERO,
            )
            .unwrap(),
        ))
        .unwrap();

    assert!(resolver.tick());
    assert!(render_is_audible(&mut renderer, 64));
}

#[test]
fn host_can_play_mixed_scores_through_score_first_runtime() {
    let (audio, mut renderer) =
        AudioRenderer::split(AudioRendererSettings::new(8_000, 64)).unwrap();
    let mut runtime = PlaybackRuntime::new(PlaybackSettings::default(), audio);

    runtime.load_sample("bd", sample_buffer());
    runtime.play_score(mixed_score()).unwrap();

    runtime.tick().unwrap();

    assert_eq!(runtime.status().state, PlaybackState::Playing);
    assert!(render_is_audible(&mut renderer, 64));
}

#[test]
fn host_can_inspect_projected_output_with_typed_sample_and_synth_identity() {
    let renderer = RendererCore::new(mixed_score());
    let projected = renderer
        .projected_output(&TransportSpan::new(Time::ZERO, Time::ONE).unwrap())
        .unwrap();

    assert_eq!(projected.len(), 2);
    assert!(projected.moments().iter().any(
        |moment| matches!(moment.intent(), Intent::Sample(sample) if sample.sample_id == "bd")
    ));
    assert!(projected
        .moments()
        .iter()
        .any(|moment| matches!(moment.intent(), Intent::Synth(synth) if synth.source == BuiltInSynthSource::Sine)));
}

#[test]
fn host_sees_projected_output_as_richer_than_projected_mosaic() {
    let renderer = RendererCore::new(controlled_sample_score());
    let window = TransportSpan::new(Time::ZERO, Time::ONE).unwrap();

    let rich = renderer.projected_output(&window).unwrap();
    let thin = renderer.projected_mosaic(&window).unwrap();

    assert_eq!(rich.len(), 1);
    assert_eq!(thin.len(), 1);
    assert!(matches!(
        rich.moments()[0].controls().get(&ControlKey::Gain),
        Some(ControlValue::Ramp { from, to }) if (from, to) == (&1.0, &0.0)
    ));
    assert_eq!(rich.moments()[0].whole(), thin.moments()[0].span());
    assert!(matches!(
        thin.moments()[0].intent(),
        Intent::Sample(sample) if sample.sample_id == "pad"
    ));
}

#[test]
fn host_can_inspect_preview_evaluated_events_via_public_accessors() {
    let controls = ControlTrack::new(
        Time::ONE,
        vec![
            ControlTile::spanning(
                Time::ZERO,
                Time::new(1, 2),
                ControlKey::PitchBend,
                ControlValue::Bipolar(SignedUnitValue::new(0.0).unwrap()),
            )
            .unwrap(),
            ControlTile::spanning(
                Time::new(1, 2),
                Time::ONE,
                ControlKey::PitchBend,
                ControlValue::Bipolar(SignedUnitValue::new(1.0).unwrap()),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let score = Score::with_controls(
        Score::from(sample_voice("pad")),
        ControlScore::track(controls),
    );
    let window = TransportSpan::new(Time::ZERO, Time::ONE).unwrap();
    let report: PreviewReport = CadenceCompiler::new().preview(&score, &window).unwrap();

    assert_eq!(report.evaluated.len(), 2);
    let update: &EvaluatedEvent = report.evaluated.get(1).expect("control update");
    assert!(matches!(
        update.kind(),
        EvaluatedEventKind::UpdateVoiceControls { .. }
    ));
    assert!(matches!(
        update.projected().controls().get(&ControlKey::PitchBend),
        Some(ControlValue::Bipolar(value)) if value.value() == 1.0
    ));
}
