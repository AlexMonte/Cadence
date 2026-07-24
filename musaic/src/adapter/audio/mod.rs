use bevy::prelude::*;
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
    bevy::CadencePlugin,
    domain::rational::Time as CycleTime,
    infrastructure::{
        audio::SampleBuffer,
        playback::{PlaybackRuntime, PlaybackSettings},
    },
    prelude::PlaybackHandle,
};

#[derive(Message, Debug, Clone)]
pub struct ScheduledAudioEvent {
    pub target: String,
    pub detail: String,
    pub when: CycleTime,
}

const SAMPLE_RATE: u32 = 44_100;
const BLOCK_FRAMES: usize = 256;

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CadencePlugin);
        let (audio, renderer) =
            AudioRenderer::split(AudioRendererSettings::new(SAMPLE_RATE, BLOCK_FRAMES))
                .expect("audio renderer");
        let runtime = PlaybackHandle::new(PlaybackSettings::default(), audio);
        load_default_samples(&runtime);
        app.insert_non_send_resource(runtime)
            .insert_non_send_resource(renderer)
            .add_message::<ScheduledAudioEvent>()
            .add_systems(
                Update,
                (
                    pump_audio_renderer.after(cadence::bevy::CadenceSet::Tick),
                    emit_playback_status_events.after(pump_audio_renderer),
                )
                    .chain()
                    .in_set(crate::infrastructure::app::MusaicSet::Runtime),
            );
        #[cfg(not(target_arch = "wasm32"))]
        start_cpal_output(app);
    }
}

fn load_default_samples(runtime: &PlaybackRuntime) {
    let mut frames = Vec::with_capacity(SAMPLE_RATE as usize / 20);
    for i in 0..frames.capacity() {
        let t = i as f32 / SAMPLE_RATE as f32;
        frames.push(Frame::from_mono(
            (t * 440.0 * std::f32::consts::TAU).sin() * 0.2,
        ));
    }
    runtime.load_sample("bd", SampleBuffer::new(SAMPLE_RATE, frames));
    for note in ["a", "b", "c", "d", "e", "f", "g"] {
        for octave in 0..=8 {
            runtime.load_sample(
                format!("{note}{octave}"),
                SampleBuffer::new(SAMPLE_RATE, vec![Frame::from_mono(0.15); 512]),
            );
        }
    }
}

fn pump_audio_renderer(mut renderer: NonSendMut<AudioRenderer>) {
    let mut block = vec![Frame::ZERO; BLOCK_FRAMES];
    renderer.render(&mut block);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut queue = audio_queue().lock().expect("audio queue lock");
        for frame in block {
            queue.push((frame.left + frame.right) * 0.5);
        }
    }
}

fn emit_playback_status_events(
    playback: NonSend<'_, PlaybackRuntime>,
    mut events: MessageWriter<ScheduledAudioEvent>,
    mut last_position: Local<Option<CycleTime>>,
) {
    let position = playback.status().cycle_position;
    if last_position.is_some_and(|previous| previous == position) {
        return;
    }
    *last_position = Some(position);
    events.write(ScheduledAudioEvent {
        target: "master".into(),
        detail: format!("cycle={position:?} state={:?}", playback.status().state),
        when: position,
    });
}

#[cfg(not(target_arch = "wasm32"))]
mod native_output {
    use std::sync::{Mutex, OnceLock};

    static QUEUE: OnceLock<Mutex<Vec<f32>>> = OnceLock::new();

    pub fn queue() -> &'static Mutex<Vec<f32>> {
        QUEUE.get_or_init(|| Mutex::new(Vec::with_capacity(8192)))
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn audio_queue() -> &'static std::sync::Mutex<Vec<f32>> {
    native_output::queue()
}

#[cfg(not(target_arch = "wasm32"))]
fn start_cpal_output(app: &mut App) {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    let host = cpal::default_host();
    let device = host.default_output_device().expect("default output device");
    let config = device
        .default_output_config()
        .expect("default output config");
    let channels = config.channels() as usize;
    let err_fn = |err| eprintln!("cpal stream error: {err}");
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| fill_output(data, channels),
            err_fn,
            None,
        ),
        other => panic!("unsupported sample format: {other:?}"),
    }
    .expect("output stream");
    stream.play().expect("play output stream");
    app.insert_non_send_resource(AudioStream(stream));
}

#[cfg(not(target_arch = "wasm32"))]
struct AudioStream(cpal::Stream);

#[cfg(not(target_arch = "wasm32"))]
fn fill_output(data: &mut [f32], channels: usize) {
    let mut queue = audio_queue().lock().expect("audio queue lock");
    for sample in data.chunks_mut(channels) {
        let value = queue.first().copied().unwrap_or(0.0);
        if !queue.is_empty() {
            queue.remove(0);
        }
        for channel in sample.iter_mut() {
            *channel = value;
        }
    }
}
