//! Host-facing render queue and block renderer.

use std::sync::{Arc, Mutex};

use rtrb::{Consumer, Producer, RingBuffer};
use thiserror::Error;

use crate::adapter::{
    audio::{AudioMixer, Frame},
    sample_bank::LoadedSampleTrigger,
};
use crate::application::{
    audio::{AudioRuntimeControlDelta, VoiceInstanceId},
    synth::SynthTrigger,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Settings used to create an [`AudioRenderer`].
pub struct AudioRendererSettings {
    /// Renderer output sample rate.
    pub sample_rate: u32,
    /// Capacity of the render-command queue.
    pub queue_capacity: usize,
}

impl AudioRendererSettings {
    /// Creates renderer settings.
    #[must_use]
    pub fn new(sample_rate: u32, queue_capacity: usize) -> Self {
        Self {
            sample_rate,
            queue_capacity,
        }
    }
}

#[derive(Debug, Clone)]
/// Command sent from the control thread to the renderer thread.
pub enum RenderCommand {
    /// Start a resolved sample voice.
    Play(LoadedSampleTrigger),
    /// Start a synth voice.
    PlaySynth(SynthTrigger),
    /// Update runtime controls for one active voice.
    UpdateVoiceControls {
        /// Voice instance that should receive the update.
        voice_id: VoiceInstanceId,
        /// Runtime control delta to apply.
        delta: AudioRuntimeControlDelta,
    },
    /// Release one voice instance explicitly.
    ReleaseVoice(VoiceInstanceId),
}

#[derive(Clone)]
/// Queue sender for render commands.
pub struct RenderCommandSender {
    producer: Arc<Mutex<Producer<RenderCommand>>>,
}

impl RenderCommandSender {
    /// Queues a sample-playback command.
    pub fn play(&self, loaded: LoadedSampleTrigger) -> Result<(), RenderCommandError> {
        let mut producer = self
            .producer
            .lock()
            .map_err(|_| RenderCommandError::QueuePoisoned)?;

        producer
            .push(RenderCommand::Play(loaded))
            .map_err(|_| RenderCommandError::QueueFull)
    }

    /// Queues a synth-playback command.
    pub fn play_synth(&self, trigger: SynthTrigger) -> Result<(), RenderCommandError> {
        let mut producer = self
            .producer
            .lock()
            .map_err(|_| RenderCommandError::QueuePoisoned)?;

        producer
            .push(RenderCommand::PlaySynth(trigger))
            .map_err(|_| RenderCommandError::QueueFull)
    }

    /// Queues a runtime control update for one voice.
    pub fn update_voice_controls(
        &self,
        voice_id: VoiceInstanceId,
        delta: AudioRuntimeControlDelta,
    ) -> Result<(), RenderCommandError> {
        let mut producer = self
            .producer
            .lock()
            .map_err(|_| RenderCommandError::QueuePoisoned)?;

        producer
            .push(RenderCommand::UpdateVoiceControls { voice_id, delta })
            .map_err(|_| RenderCommandError::QueueFull)
    }

    /// Queues an explicit voice release.
    pub fn release_voice(&self, voice_id: VoiceInstanceId) -> Result<(), RenderCommandError> {
        let mut producer = self
            .producer
            .lock()
            .map_err(|_| RenderCommandError::QueuePoisoned)?;

        producer
            .push(RenderCommand::ReleaseVoice(voice_id))
            .map_err(|_| RenderCommandError::QueueFull)
    }
}

#[derive(Clone)]
/// Control-thread handle for talking to the renderer.
pub struct AudioControl {
    sample_rate: u32,
    sender: RenderCommandSender,
}

impl AudioControl {
    #[must_use]
    pub(crate) fn new(sample_rate: u32, sender: RenderCommandSender) -> Self {
        Self {
            sample_rate,
            sender,
        }
    }

    /// Returns the renderer sample rate.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub(crate) fn play_loaded(
        &self,
        loaded: LoadedSampleTrigger,
    ) -> Result<(), RenderCommandError> {
        self.sender.play(loaded)
    }

    pub(crate) fn play_synth(&self, trigger: SynthTrigger) -> Result<(), RenderCommandError> {
        self.sender.play_synth(trigger)
    }

    pub(crate) fn update_voice_controls(
        &self,
        voice_id: VoiceInstanceId,
        delta: AudioRuntimeControlDelta,
    ) -> Result<(), RenderCommandError> {
        self.sender.update_voice_controls(voice_id, delta)
    }

    pub(crate) fn release_voice(
        &self,
        voice_id: VoiceInstanceId,
    ) -> Result<(), RenderCommandError> {
        self.sender.release_voice(voice_id)
    }
}

/// Renderer-thread object that mixes queued sample and synth voices.
pub struct AudioRenderer {
    sender: RenderCommandSender,
    consumer: Consumer<RenderCommand>,
    mixer: AudioMixer,
    sample_rate: u32,
}

impl AudioRenderer {
    /// Creates a paired control handle and renderer.
    pub fn split(
        settings: AudioRendererSettings,
    ) -> Result<(AudioControl, Self), AudioRendererError> {
        let renderer = Self::new(settings.sample_rate, settings.queue_capacity)?;
        let control = AudioControl::new(renderer.sample_rate(), renderer.command_sender());
        Ok((control, renderer))
    }

    pub(crate) fn new(sample_rate: u32, queue_capacity: usize) -> Result<Self, AudioRendererError> {
        if sample_rate == 0 {
            return Err(AudioRendererError::InvalidSampleRate);
        }

        if queue_capacity == 0 {
            return Err(AudioRendererError::InvalidQueueCapacity);
        }

        let (producer, consumer) = RingBuffer::new(queue_capacity);
        let sender = RenderCommandSender {
            producer: Arc::new(Mutex::new(producer)),
        };

        Ok(Self {
            sender,
            consumer,
            mixer: AudioMixer::new(sample_rate),
            sample_rate,
        })
    }

    #[must_use]
    pub(crate) fn command_sender(&self) -> RenderCommandSender {
        self.sender.clone()
    }

    /// Returns the output sample rate.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Hook for hosts that need a start-of-block callback.
    pub fn on_start_processing(&mut self) {}

    /// Drains pending commands and renders the next output block.
    pub fn render(&mut self, out: &mut [Frame]) {
        while let Ok(command) = self.consumer.pop() {
            match command {
                RenderCommand::Play(loaded) => self.mixer.push(loaded),
                RenderCommand::PlaySynth(trigger) => self.mixer.push_synth(trigger),
                RenderCommand::UpdateVoiceControls { voice_id, delta } => {
                    self.mixer.update_voice_controls(voice_id, delta)
                }
                RenderCommand::ReleaseVoice(voice_id) => self.mixer.release_voice(voice_id),
            }
        }

        self.mixer.render(out);
    }
}

#[derive(Debug, Error)]
/// Errors returned while creating an [`AudioRenderer`].
pub enum AudioRendererError {
    /// `sample_rate` was zero.
    #[error("audio renderer sample rate must be positive")]
    InvalidSampleRate,
    /// `queue_capacity` was zero.
    #[error("audio renderer queue capacity must be positive")]
    InvalidQueueCapacity,
}

#[derive(Debug, Error)]
/// Errors returned while queuing render commands.
pub enum RenderCommandError {
    /// The ring buffer is full.
    #[error("render command queue is full")]
    QueueFull,
    /// The shared queue state was poisoned.
    #[error("render command queue is poisoned")]
    QueuePoisoned,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::audio::SampleBuffer,
        application::{sample::SampleTrigger, synth::SynthTrigger},
        domain::intent::BuiltInSynthSource,
    };

    #[test]
    fn split_produces_control_handle_and_renderer() {
        let (control, renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8_000, 4)).unwrap();

        assert_eq!(control.sample_rate(), 8_000);
        assert_eq!(renderer.sample_rate(), 8_000);
    }

    #[test]
    fn renderer_renders_enqueued_loaded_sample() {
        let mut renderer = AudioRenderer::new(8_000, 4).unwrap();
        let loaded = LoadedSampleTrigger {
            trigger: SampleTrigger::builder()
                .sample("kick")
                .envelope(crate::application::sample::SampleEnvelope::new(
                    std::time::Duration::ZERO,
                    std::time::Duration::ZERO,
                    crate::domain::control::UnitValue::new(1.0).unwrap(),
                    std::time::Duration::ZERO,
                    None,
                    std::time::Duration::ZERO,
                ))
                .play_for(std::time::Duration::ZERO)
                .build()
                .unwrap(),
            sample_key: "kick".to_string(),
            sample: Arc::new(SampleBuffer::new(
                8_000,
                vec![Frame::from_mono(0.0), Frame::from_mono(0.5)],
            )),
            playback_limit: None,
            choke_group: None,
        };
        let sender = renderer.command_sender();

        sender.play(loaded).unwrap();

        let mut out = vec![Frame::ZERO; 2];
        renderer.render(&mut out);

        assert!(out.iter().any(|frame| *frame != Frame::ZERO));
    }

    #[test]
    fn zero_capacity_queue_is_rejected() {
        assert!(matches!(
            AudioRenderer::new(8_000, 0),
            Err(AudioRendererError::InvalidQueueCapacity)
        ));
    }

    #[test]
    fn zero_sample_rate_is_rejected() {
        assert!(matches!(
            AudioRenderer::new(0, 4),
            Err(AudioRendererError::InvalidSampleRate)
        ));
    }

    #[test]
    fn renderer_releases_voice_commands() {
        let mut renderer = AudioRenderer::new(8_000, 8).unwrap();
        let sender = renderer.command_sender();
        let voice_id = VoiceInstanceId::new(60);
        let loaded = LoadedSampleTrigger {
            trigger: SampleTrigger::builder()
                .voice_id(voice_id)
                .sample("pad")
                .envelope(crate::application::sample::SampleEnvelope::new(
                    std::time::Duration::ZERO,
                    std::time::Duration::ZERO,
                    crate::domain::control::UnitValue::new(1.0).unwrap(),
                    std::time::Duration::from_millis(10),
                    None,
                    std::time::Duration::ZERO,
                ))
                .play_for(std::time::Duration::ZERO)
                .build()
                .unwrap(),
            sample_key: "pad".to_string(),
            sample: Arc::new(SampleBuffer::new(8_000, vec![Frame::from_mono(1.0); 128])),
            playback_limit: None,
            choke_group: None,
        };

        sender.play(loaded).unwrap();
        sender.release_voice(voice_id).unwrap();
        let mut out = vec![Frame::ZERO; 64];
        renderer.render(&mut out);

        assert!(out.iter().any(|frame| frame.left > 0.0));
    }

    #[test]
    fn renderer_renders_enqueued_synth_trigger() {
        let mut renderer = AudioRenderer::new(8_000, 4).unwrap();
        let sender = renderer.command_sender();

        sender
            .play_synth(
                SynthTrigger::builder()
                    .source(BuiltInSynthSource::Sine)
                    .pitch(69.0)
                    .play_for(std::time::Duration::from_millis(40))
                    .build()
                    .unwrap(),
            )
            .unwrap();

        let mut out = vec![Frame::ZERO; 16];
        renderer.render(&mut out);

        assert!(out.iter().any(|frame| *frame != Frame::ZERO));
    }
}
