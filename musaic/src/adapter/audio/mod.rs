use bevy::prelude::*;
pub mod meter;
pub mod project_samples;
use cadence::{
    adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
    bevy::CadencePlugin,
    domain::rational::Time as CycleTime,
    infrastructure::{
        audio::SampleBuffer,
        playback::{PlaybackRuntime, PlaybackSettings},
    },
};
use project_samples::ProjectSampleBindings;

#[derive(Message, Debug, Clone)]
pub struct ScheduledAudioEvent {
    pub target: String,
    pub detail: String,
    pub when: CycleTime,
}

#[derive(Message)]
pub struct AuditionRequest {
    pub epoch: u64,
    pub intent: cadence::prelude::Intent,
    pub pitch: Option<f64>,
}

#[derive(Resource, Default)]
pub struct AudioPreviewEpoch(pub u64);

/// Device negotiation and delivery health, suitable for transport/inspector UI.
#[derive(Resource, Default)]
pub struct AudioDeviceStatus {
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub error: Option<String>,
    pub underrun_frames: u64,
    pub late_commands: u64,
    pub rejected_commands: u64,
    pub resource_limit_hits: u64,
}

pub struct AudioPlugin;
impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(CadencePlugin)
            .init_resource::<meter::MasterMeter>()
            .add_message::<meter::ClearMasterClip>()
            .add_message::<ScheduledAudioEvent>()
            .init_resource::<AudioPreviewEpoch>()
            .add_message::<AuditionRequest>();
        #[cfg(not(target_arch = "wasm32"))]
        let (audio, device_status) = match native::start(app) {
            Ok(result) => result,
            Err(error) => {
                warn!("Audio output unavailable: {error}");
                let (audio, _) = AudioRenderer::split(AudioRendererSettings::new(48_000, 4096))
                    .expect("valid fallback audio settings");
                (
                    audio,
                    AudioDeviceStatus {
                        error: Some(error),
                        ..default()
                    },
                )
            }
        };
        #[cfg(target_arch = "wasm32")]
        let (audio, device_status) = {
            let (audio, _) = AudioRenderer::split(AudioRendererSettings::new(48_000, 4096))
                .expect("valid fallback audio settings");
            (
                audio,
                AudioDeviceStatus {
                    error: Some("Browser audio output is not connected yet".into()),
                    ..default()
                },
            )
        };
        let runtime = PlaybackRuntime::new(PlaybackSettings::default(), audio);
        load_default_samples(&runtime);
        app.insert_non_send_resource(runtime)
            .insert_resource(device_status)
            .add_systems(
                Update,
                sync_project_samples
                    .before(cadence::bevy::CadenceSet::ReplaceScores)
                    .in_set(crate::infrastructure::app::MusaicSet::Runtime),
            )
            .add_systems(
                Update,
                play_sound_previews
                    .after(cadence::bevy::CadenceSet::Tick)
                    .in_set(crate::infrastructure::app::MusaicSet::Runtime),
            )
            .add_systems(
                Update,
                emit_playback_status_events
                    .after(cadence::bevy::CadenceSet::Tick)
                    .in_set(crate::infrastructure::app::MusaicSet::Runtime),
            );
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(
            Update,
            native::read_device_status.in_set(crate::infrastructure::app::MusaicSet::Runtime),
        );
    }
}

fn play_sound_previews(
    mut requests: MessageReader<AuditionRequest>,
    epoch: Res<AudioPreviewEpoch>,
    mut playback: NonSendMut<PlaybackRuntime>,
    mut diagnostics: ResMut<crate::infrastructure::diagnostics::DiagnosticStore>,
) {
    if let Some(request) = requests
        .read()
        .filter(|request| request.epoch == epoch.0)
        .last()
    {
        if let Err(error) = playback.audition(
            request.intent.clone(),
            request.pitch,
            std::time::Duration::from_secs(2),
        ) {
            use crate::infrastructure::diagnostics::*;
            diagnostics.push(LayeredDiagnostic {
                phase: DiagnosticPhase::Runtime,
                diagnostic: AppDiagnostic::Runtime(RuntimeDiagnostic::ProjectionFailed {
                    detail: format!("Could not preview sound: {error}"),
                }),
            });
        }
    }
}

/// File decoding is complete before this control-thread binding stage. The
/// device callback never reads files or takes the sample-bank lock.
fn sync_project_samples(
    project: Res<crate::application::session::MusaicProject>,
    playback: NonSend<PlaybackRuntime>,
    mut bindings: Local<ProjectSampleBindings>,
) {
    if !project.is_changed() {
        return;
    }
    bindings.synchronize(&project, &playback.sample_bank());
}

fn load_default_samples(runtime: &PlaybackRuntime) {
    load_builtin_samples(&runtime.sample_bank());
}

pub(crate) fn load_builtin_samples(bank: &cadence::infrastructure::playback::SampleBank) {
    use cadence::infrastructure::playback::{ChokeGroup, SampleLoadOptions};
    const SAMPLE_RATE: u32 = 48_000;
    // Built-in percussion is generated once on the control thread. Pitched
    // note tiles use Cadence synth voices and never placeholder sample buffers.
    for (name, drum, duration) in [
        ("bd", Drum::Kick, 0.45),
        ("sd", Drum::Snare, 0.30),
        ("hh", Drum::ClosedHat, 0.14),
        ("oh", Drum::OpenHat, 0.55),
    ] {
        let frames = drum_frames(drum, SAMPLE_RATE, duration);
        bank.load_with_options(
            name,
            SampleBuffer::new(SAMPLE_RATE, frames),
            SampleLoadOptions {
                choke_group: matches!(drum, Drum::ClosedHat | Drum::OpenHat)
                    .then_some(ChokeGroup::Hat),
                ..default()
            },
        );
    }
}

#[derive(Clone, Copy)]
enum Drum {
    Kick,
    Snare,
    ClosedHat,
    OpenHat,
}

fn drum_frames(drum: Drum, sample_rate: u32, duration: f32) -> Vec<Frame> {
    let count = (duration * sample_rate as f32) as usize;
    let mut frames = Vec::with_capacity(count);
    let mut noise_state = 0x7a31_c496_u32;
    let mut previous_noise = 0.0;
    let mut phase = 0.0_f32;
    for index in 0..count {
        let t = index as f32 / sample_rate as f32;
        noise_state ^= noise_state << 13;
        noise_state ^= noise_state >> 17;
        noise_state ^= noise_state << 5;
        let noise = noise_state as f32 / u32::MAX as f32 * 2.0 - 1.0;
        let high_noise = (noise - previous_noise) * 0.5;
        previous_noise = noise;
        let attack = (t / 0.0015).min(1.0);
        let fade = ((duration - t) / 0.015).clamp(0.0, 1.0);
        let sample = match drum {
            Drum::Kick => {
                let frequency = 46.0 + 140.0 * (-t * 35.0).exp();
                phase += std::f32::consts::TAU * frequency / sample_rate as f32;
                phase.sin() * (-t * 10.0).exp() * 0.85 + high_noise * (-t * 180.0).exp() * 0.15
            }
            Drum::Snare => {
                ((t * 185.0 * std::f32::consts::TAU).sin() * 0.25 + high_noise * 0.75)
                    * (-t * 18.0).exp()
                    * 0.7
            }
            Drum::ClosedHat => high_noise * (-t * 40.0).exp() * 0.4,
            Drum::OpenHat => high_noise * (-t * 9.0).exp() * 0.35,
        };
        frames.push(Frame::from_mono(sample * attack * fade));
    }
    frames
}

#[cfg(test)]
mod kit_tests {
    use super::*;
    #[test]
    fn built_in_kit_is_finite_decaying_and_deterministic() {
        for drum in [Drum::Kick, Drum::Snare, Drum::ClosedHat, Drum::OpenHat] {
            let frames = drum_frames(drum, 48_000, 0.45);
            assert_eq!(frames, drum_frames(drum, 48_000, 0.45));
            assert!(
                frames
                    .iter()
                    .all(|frame| frame.left.is_finite() && frame.left.abs() <= 1.0)
            );
            assert!(frames[1..4800].iter().any(|frame| frame.left.abs() > 0.05));
            assert!(frames.last().unwrap().left.abs() < 0.001);
            assert_eq!(frames[0], Frame::ZERO);
        }
    }
}

#[cfg(test)]
mod audio_seam_tests {
    use super::*;
    use crate::{
        application::session::{LoadedProjectSample, MusaicProject},
        domain::project::samples::{SampleId, SampleImportOptions, SampleMetadata},
        infrastructure::diagnostics::DiagnosticStore,
    };
    use cadence::{
        adapter::audio::AudioTriggerResolveError,
        infrastructure::playback::{PlaybackError, PlaybackState},
        prelude::Intent,
    };
    use std::{sync::Arc, time::Duration};

    const RATE: u32 = 8_000;
    const SAMPLE: SampleId = SampleId(1);

    fn project_sample(root_pitch: Option<f64>, frame: Frame) -> MusaicProject {
        let mut project = MusaicProject::new_empty();
        project.samples.manifest.samples.insert(
            SAMPLE,
            SampleMetadata {
                source_name: "recording.wav".into(),
                relative_path: "samples/recording.wav".into(),
                sample_rate: RATE,
                channels: 2,
                frame_count: 2_000,
                byte_length: 8_044,
                checksum: format!("{}:{}", frame.left, frame.right),
                options: SampleImportOptions {
                    root_pitch,
                    ..default()
                },
            },
        );
        project.samples.loaded.insert(
            SAMPLE,
            LoadedProjectSample::new(Arc::from([]), SampleBuffer::new(RATE, vec![frame; 2_000])),
        );
        project
    }

    fn setup(project: MusaicProject) -> (App, AudioRenderer) {
        let (audio, renderer) = AudioRenderer::split(AudioRendererSettings::new(RATE, 64)).unwrap();
        let runtime = PlaybackRuntime::new(PlaybackSettings::default(), audio);
        load_default_samples(&runtime);
        let mut app = App::new();
        app.insert_non_send_resource(runtime)
            .insert_resource(project)
            .init_resource::<AudioPreviewEpoch>()
            .init_resource::<DiagnosticStore>()
            .add_message::<AuditionRequest>()
            .add_systems(Update, (sync_project_samples, play_sound_previews).chain());
        app.update();
        (app, renderer)
    }

    fn render(renderer: &mut AudioRenderer) -> Vec<Frame> {
        let mut frames = vec![Frame::ZERO; 128];
        renderer.render(&mut frames);
        frames
    }

    fn audition_sample(app: &mut App, pitch: Option<f64>) -> Result<(), PlaybackError> {
        app.world_mut()
            .non_send_resource_mut::<PlaybackRuntime>()
            .audition(
                Intent::sample(SAMPLE.runtime_name()),
                pitch,
                Duration::from_secs(1),
            )
    }

    #[test]
    fn changed_root_pitch_replaces_old_project_audio_for_pitched_and_unpitched_notes() {
        let (mut app, mut renderer) = setup(project_sample(Some(60.0), Frame::new(0.25, 0.0)));
        audition_sample(&mut app, Some(60.0)).unwrap();
        let initial = render(&mut renderer);
        assert!(initial[80].left > 0.1 && initial[80].right == 0.0);

        // Both projects legitimately use their own SampleId(1). The old root
        // would otherwise remain the closest bank variant for a C4 trigger.
        app.insert_resource(project_sample(Some(72.0), Frame::new(0.0, -0.5)));
        app.update();
        for pitch in [Some(60.0), None] {
            audition_sample(&mut app, pitch).unwrap();
            let replacement = render(&mut renderer);
            assert!(replacement.iter().all(|frame| frame.left == 0.0));
            assert!(replacement[80].right < -0.3);
        }

        // Clearing the root must also retire every earlier pitched variant.
        app.insert_resource(project_sample(None, Frame::new(-0.25, 0.0)));
        app.update();
        audition_sample(&mut app, Some(60.0)).unwrap();
        let unpitched = render(&mut renderer);
        assert!(unpitched[80].left < -0.1);
        assert!(unpitched.iter().all(|frame| frame.right == 0.0));
    }

    #[test]
    fn bank_member_replacement_dissolution_and_project_switch_retire_old_aliases() {
        use crate::domain::project::samples::{
            SampleBankDefinition, SamplePitchZone, SampleVariant,
        };
        let member = SampleId(2);
        let mut project = project_sample(Some(60.0), Frame::new(0.25, 0.0));
        let other = project_sample(Some(72.0), Frame::new(0.0, -0.5));
        project
            .samples
            .manifest
            .samples
            .insert(member, other.samples.manifest.samples[&SAMPLE].clone());
        project
            .samples
            .loaded
            .insert(member, other.samples.loaded[&SAMPLE].clone());
        project.samples.manifest.banks.insert(
            SAMPLE,
            SampleBankDefinition {
                variants: vec![
                    SampleVariant {
                        pitch_zone: Some(SamplePitchZone {
                            low: 0.0,
                            high: 65.0,
                        }),
                        ..SampleVariant::new(SAMPLE)
                    },
                    SampleVariant {
                        pitch_zone: Some(SamplePitchZone {
                            low: 66.0,
                            high: 127.0,
                        }),
                        ..SampleVariant::new(member)
                    },
                ],
            },
        );
        let (mut app, mut renderer) = setup(project.clone());
        audition_sample(&mut app, Some(72.0)).unwrap();
        let first = render(&mut renderer);
        assert!(first[80].right < -0.3 && first[80].left == 0.0);
        let relinked = project_sample(Some(72.0), Frame::new(-0.5, 0.0));
        project
            .samples
            .loaded
            .insert(member, relinked.samples.loaded[&SAMPLE].clone());
        app.insert_resource(project.clone());
        app.update();
        audition_sample(&mut app, Some(72.0)).unwrap();
        let replaced = render(&mut renderer);
        assert!(replaced[80].left < -0.3 && replaced[80].right == 0.0);
        project.samples.manifest.banks.clear();
        app.insert_resource(project);
        app.update();
        audition_sample(&mut app, Some(72.0)).unwrap();
        let direct = render(&mut renderer);
        assert!(
            direct[80].left > 0.1 && direct[80].right == 0.0,
            "dissolution restores the lead's direct recording"
        );
        app.insert_resource(MusaicProject::new_empty());
        app.update();
        let bank = app
            .world()
            .non_send_resource::<PlaybackRuntime>()
            .sample_bank();
        assert!(!bank.contains(&SAMPLE.runtime_name()));
        assert!(!bank.contains(&member.runtime_name()));
        assert!(bank.contains("bd"));
    }

    #[test]
    fn missing_or_removed_project_samples_cannot_resolve_stale_audio() {
        for retain_manifest in [true, false] {
            let (mut app, _renderer) = setup(project_sample(Some(60.0), Frame::new(0.25, 0.0)));
            let mut replacement = project_sample(Some(60.0), Frame::new(-0.25, 0.0));
            replacement.samples.loaded.clear();
            if !retain_manifest {
                replacement.samples.manifest.samples.clear();
            }
            app.insert_resource(replacement);
            app.update();
            let bank = app
                .world()
                .non_send_resource::<PlaybackRuntime>()
                .sample_bank();
            assert!(!bank.contains(&SAMPLE.runtime_name()));
            assert!(
                bank.contains("bd"),
                "project changes must retain the built-in kit"
            );
            assert!(matches!(
                audition_sample(&mut app, Some(60.0)),
                Err(PlaybackError::AudioResolution(AudioTriggerResolveError::MissingSample(name)))
                    if name == SAMPLE.runtime_name()
            ));
        }
    }

    #[test]
    fn preview_epochs_drop_old_requests_and_play_the_last_current_request() {
        let (mut app, mut renderer) = setup(project_sample(None, Frame::new(0.0, -0.5)));
        app.world_mut().write_message(AuditionRequest {
            epoch: 0,
            intent: Intent::sample(SAMPLE.runtime_name()),
            pitch: None,
        });
        // Project replacement, Stop, and Panic advance this authority before
        // the audio stage consumes requests from the command batch.
        app.world_mut().resource_mut::<AudioPreviewEpoch>().0 = 1;
        app.update();
        assert!(
            render(&mut renderer)
                .iter()
                .all(|frame| *frame == Frame::ZERO)
        );

        for (epoch, sample) in [
            (1, "bd".to_owned()),
            (1, SAMPLE.runtime_name()),
            (0, "missing-old-project-sound".to_owned()),
        ] {
            app.world_mut().write_message(AuditionRequest {
                epoch,
                intent: Intent::sample(sample),
                pitch: None,
            });
        }
        app.update();
        let current = render(&mut renderer);
        assert!(current.iter().all(|frame| frame.left == 0.0));
        assert!(
            current[80].right < -0.3,
            "the final current preview must play"
        );
        assert!(app.world().resource::<DiagnosticStore>().items.is_empty());
        let status = app.world().non_send_resource::<PlaybackRuntime>().status();
        assert_eq!(status.state, PlaybackState::Stopped);
        assert_eq!(status.cycle_position, CycleTime::ZERO);
    }
}

fn emit_playback_status_events(
    playback: NonSend<'_, PlaybackRuntime>,
    mut events: MessageWriter<ScheduledAudioEvent>,
    mut last_position: Local<Option<CycleTime>>,
) {
    let status = playback.status();
    if last_position.is_some_and(|previous| previous == status.cycle_position) {
        return;
    }
    *last_position = Some(status.cycle_position);
    events.write(ScheduledAudioEvent {
        target: "master".into(),
        detail: format!("cycle={:?} state={:?}", status.cycle_position, status.state),
        when: status.cycle_position,
    });
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use cadence::adapter::audio::{
        AudioControl,
        output::{AudioOutput, AudioOutputWorker, OutputStatus, start_output_worker},
    };
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    // Roughly 20 ms of renderer lead, independent of graphics frame rate.
    const BUFFER_MILLISECONDS: u32 = 20;
    pub(super) struct AudioStream {
        _stream: cpal::Stream,
        _worker: AudioOutputWorker,
        status: OutputStatus,
        audio: AudioControl,
        failed: Arc<AtomicBool>,
        meter: Arc<meter::MeterBridge>,
    }

    pub(super) fn start(app: &mut App) -> Result<(AudioControl, AudioDeviceStatus), String> {
        let device = cpal::default_host()
            .default_output_device()
            .ok_or("No audio output device found")?;
        let supported = device
            .default_output_config()
            .map_err(|error| error.to_string())?;
        let sample_rate = supported.sample_rate().0;
        let channels = supported.channels();
        if channels == 0 {
            return Err("Audio device exposes no output channels".into());
        }
        let (audio, renderer) = AudioRenderer::split(AudioRendererSettings::new(sample_rate, 4096))
            .map_err(|error| error.to_string())?;
        let capacity = (sample_rate as usize * BUFFER_MILLISECONDS as usize / 1000).max(128);
        let (output, worker) =
            start_output_worker(renderer, capacity).map_err(|error| error.to_string())?;
        let status = output.status();
        let failed = Arc::new(AtomicBool::new(false));
        let meter = Arc::new(meter::MeterBridge::default());
        let config = supported.config();
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => {
                build::<f32>(&device, &config, output, failed.clone(), meter.clone())
            }
            cpal::SampleFormat::F64 => {
                build::<f64>(&device, &config, output, failed.clone(), meter.clone())
            }
            cpal::SampleFormat::I8 => {
                build::<i8>(&device, &config, output, failed.clone(), meter.clone())
            }
            cpal::SampleFormat::I16 => {
                build::<i16>(&device, &config, output, failed.clone(), meter.clone())
            }
            cpal::SampleFormat::I32 => {
                build::<i32>(&device, &config, output, failed.clone(), meter.clone())
            }
            cpal::SampleFormat::I64 => {
                build::<i64>(&device, &config, output, failed.clone(), meter.clone())
            }
            cpal::SampleFormat::U8 => {
                build::<u8>(&device, &config, output, failed.clone(), meter.clone())
            }
            cpal::SampleFormat::U16 => {
                build::<u16>(&device, &config, output, failed.clone(), meter.clone())
            }
            cpal::SampleFormat::U32 => {
                build::<u32>(&device, &config, output, failed.clone(), meter.clone())
            }
            cpal::SampleFormat::U64 => {
                build::<u64>(&device, &config, output, failed.clone(), meter.clone())
            }
            other => return Err(format!("Unsupported audio sample format: {other:?}")),
        }
        .map_err(|error| error.to_string())?;
        stream.play().map_err(|error| error.to_string())?;
        app.insert_non_send_resource(AudioStream {
            _stream: stream,
            _worker: worker,
            status,
            audio: audio.clone(),
            failed,
            meter,
        });
        Ok((
            audio,
            AudioDeviceStatus {
                sample_rate: Some(sample_rate),
                channels: Some(channels),
                ..default()
            },
        ))
    }

    fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
        device: &cpal::Device,
        config: &cpal::StreamConfig,
        mut output: AudioOutput,
        failed: Arc<AtomicBool>,
        meter: Arc<meter::MeterBridge>,
    ) -> Result<cpal::Stream, cpal::BuildStreamError> {
        let channels = usize::from(config.channels);
        device.build_output_stream(
            config,
            move |data: &mut [T], _| {
                let mut peak = Frame::ZERO;
                for frame in data.chunks_mut(channels) {
                    let delivered = write_channels(frame, output.next_frame());
                    peak.left = peak.left.max(delivered.left.abs());
                    peak.right = peak.right.max(delivered.right.abs());
                }
                if !data.is_empty() {
                    meter.publish(peak.left, peak.right);
                }
            },
            move |_| {
                failed.store(true, Ordering::Release);
            },
            None,
        )
    }

    /// Returns channel-mapped samples before clipping, from the exact conversion path.
    fn write_channels<T: cpal::Sample + cpal::FromSample<f32>>(
        output: &mut [T],
        frame: Frame,
    ) -> Frame {
        if output.len() == 1 {
            let mono = (frame.left + frame.right) * 0.5;
            output[0] = T::from_sample(mono.clamp(-1.0, 1.0));
            return Frame::from_mono(mono);
        }
        for (index, sample) in output.iter_mut().enumerate() {
            *sample = T::from_sample(
                match index {
                    0 => frame.left,
                    1 => frame.right,
                    _ => 0.0,
                }
                .clamp(-1.0, 1.0),
            );
        }
        frame
    }

    pub(super) fn read_device_status(
        stream: Option<NonSend<AudioStream>>,
        mut device: ResMut<AudioDeviceStatus>,
        mut meter: ResMut<meter::MasterMeter>,
        mut clear: MessageReader<meter::ClearMasterClip>,
        time: Res<Time<Real>>,
    ) {
        let Some(stream) = stream else {
            clear.clear();
            return;
        };
        stream
            .meter
            .sample(&mut meter, time.delta_secs(), clear.read().count() > 0);
        device.underrun_frames = stream.status.underrun_frames();
        device.late_commands = stream.audio.late_command_count();
        device.rejected_commands = stream.audio.rejected_command_count();
        device.resource_limit_hits = stream.audio.resource_limit_count();
        if stream.failed.load(Ordering::Acquire) {
            device.error = Some("Audio device stopped delivering output".into());
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn meter_measures_mono_cancellation_and_stereo_overload_before_conversion_clips() {
            let mut mono = [0.0_f32];
            assert_eq!(
                write_channels(&mut mono, Frame::new(2.0, -2.0)),
                Frame::ZERO
            );
            assert_eq!(mono, [0.0]);
            let mut stereo = [0_i16; 2];
            let observed = write_channels(&mut stereo, Frame::new(2.0, -0.5));
            assert_eq!(observed, Frame::new(2.0, -0.5));
            assert_eq!(stereo, [i16::MAX, -16384]);
            let bridge = meter::MeterBridge::default();
            bridge.publish(observed.left.abs(), observed.right.abs());
            let mut status = meter::MasterMeter::default();
            bridge.sample(&mut status, 0.1, false);
            assert!(status.clipped);
            assert_eq!((status.left, status.right), (2.0, 0.5));
        }
        #[test]
        fn stereo_is_preserved_and_extra_channels_are_silent() {
            let mut out = [1.0_f32; 4];
            write_channels(&mut out, Frame::new(0.25, -0.5));
            assert_eq!(out, [0.25, -0.5, 0.0, 0.0]);
        }
        #[test]
        fn integer_devices_receive_scaled_audio_and_mono_is_explicit() {
            let mut out = [0_i16; 2];
            write_channels(&mut out, Frame::new(0.5, -0.5));
            assert_eq!(out, [16_384, -16_384]);
            let mut mono = [0.0_f32];
            write_channels(&mut mono, Frame::new(0.25, 0.75));
            assert_eq!(mono, [0.5]);
        }
    }
}
