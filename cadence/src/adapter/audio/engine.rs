//! Audio-trigger resolution into renderer commands.

use std::sync::mpsc::TryRecvError;

use crate::{
    adapter::{
        audio::{AudioControl, RenderCommand, RenderCommandError},
        sample_bank::SampleBank,
    },
    application::{
        audio::{AudioTriggerReceiver, ScheduledAudioEvent},
        sample::SampleTrigger,
        synth::SynthTrigger,
    },
};

#[derive(Debug, thiserror::Error)]
/// Resolution failure surfaced to the host, including missing project assets.
pub enum AudioTriggerResolveError {
    /// A bounded renderer queue rejected a command.
    #[error(transparent)]
    Queue(#[from] RenderCommandError),
    /// A named sample is not available in the supplied sample bank.
    #[error("sample '{0}' is not loaded")]
    MissingSample(String),
    /// The combined fitting, rate or root-pitch settings exceed supported bounds.
    #[error("invalid sample playback: {0}")]
    InvalidSamplePlayback(&'static str),
}

/// Pulls mixed audio triggers from the application queue and resolves them into
/// renderer commands.
pub struct AudioTriggerResolver {
    trigger_receiver: AudioTriggerReceiver,
    sample_bank: SampleBank,
    audio: AudioControl,
    last_error: Option<AudioTriggerResolveError>,
}

impl AudioTriggerResolver {
    /// Creates a trigger resolver.
    #[must_use]
    pub fn new(
        trigger_receiver: AudioTriggerReceiver,
        sample_bank: SampleBank,
        audio: AudioControl,
    ) -> Self {
        Self {
            trigger_receiver,
            sample_bank,
            audio,
            last_error: None,
        }
    }

    /// Drains currently available triggers.
    ///
    /// Returns `false` when the trigger source has disconnected permanently.
    pub fn tick(&mut self) -> bool {
        if self.trigger_receiver.take_overflow() {
            self.last_error = Some(RenderCommandError::QueueFull.into());
        }
        loop {
            match self.trigger_receiver.try_recv_timed() {
                Ok(queued) => self.handle_trigger(queued.trigger, queued.frame),
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
    }

    /// Takes the latest command rejection so the host can display an overload.
    pub fn take_error(&mut self) -> Option<AudioTriggerResolveError> {
        self.last_error.take()
    }

    fn handle_trigger(&mut self, trigger: ScheduledAudioEvent, frame: Option<u64>) {
        match trigger {
            ScheduledAudioEvent::StartVoice(plan) => self.handle_voice_plan(*plan, frame),
            ScheduledAudioEvent::UpdateVoiceControls { voice_id, delta } => {
                if let Err(error) = self.audio.schedule(
                    frame.unwrap_or_else(|| self.audio.next_render_frame()),
                    RenderCommand::UpdateVoiceControls {
                        voice_id,
                        delta: *delta,
                    },
                ) {
                    self.last_error = Some(error.into());
                }
            }
            ScheduledAudioEvent::ReleaseVoice(voice_id) => {
                if let Err(error) = self.audio.schedule(
                    frame.unwrap_or_else(|| self.audio.next_render_frame()),
                    RenderCommand::ReleaseVoice(voice_id),
                ) {
                    self.last_error = Some(error.into());
                }
            }
        }
    }

    fn handle_voice_plan(
        &mut self,
        plan: crate::application::audio::AudioVoicePlan,
        frame: Option<u64>,
    ) {
        match &plan.source {
            crate::application::audio::AudioSourcePlan::Sample(source) => {
                if !source.playback_start.is_finite()
                    || !source.playback_end.is_finite()
                    || source.playback_start < 0.0
                    || source.playback_start >= source.playback_end
                    || source.playback_end > 1.0
                {
                    self.last_error = Some(AudioTriggerResolveError::InvalidSamplePlayback(
                        "sample region must satisfy 0 <= start < end <= 1",
                    ));
                    return;
                }
                let Some(trigger) = SampleTrigger::from_voice_plan(&plan) else {
                    return;
                };
                let loaded = match self.sample_bank.resolve_trigger(&trigger) {
                    Some(loaded) => loaded,
                    None => {
                        self.last_error = Some(AudioTriggerResolveError::MissingSample(
                            trigger.sample.clone(),
                        ));
                        return;
                    }
                };

                if !loaded.trigger.playback_rate.is_finite()
                    || loaded.trigger.playback_rate <= 0.0
                    || loaded.trigger.playback_rate > 65_536.0
                {
                    self.last_error = Some(AudioTriggerResolveError::InvalidSamplePlayback(
                        "fitted rate including root pitch must be positive, finite and at most 65536; Fit requires a positive note duration",
                    ));
                    return;
                }

                if let Err(error) = self.audio.schedule(
                    frame.unwrap_or_else(|| self.audio.next_render_frame()),
                    RenderCommand::Play(loaded),
                ) {
                    self.last_error = Some(error.into());
                }
            }
            crate::application::audio::AudioSourcePlan::Synth(_) => {
                let Some(trigger) = SynthTrigger::from_voice_plan(&plan) else {
                    return;
                };
                if let Err(error) = self.audio.schedule(
                    frame.unwrap_or_else(|| self.audio.next_render_frame()),
                    RenderCommand::PlaySynth(trigger),
                ) {
                    self.last_error = Some(error.into());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::{
            audio::{AudioRenderer, AudioRendererSettings, Frame, SampleBuffer},
            sample_bank::{SampleBank, SampleLoadOptions},
        },
        application::audio::{ScheduledAudioEvent, audio_trigger_channel},
        domain::{
            control::ControlMap,
            intent::{BuiltInSynthSource, SampleIntent},
        },
    };
    use std::time::Duration;

    #[test]
    fn tick_resolves_loaded_sample_into_renderer() {
        let bank = SampleBank::new();
        bank.load_with_options(
            "kick",
            SampleBuffer::new(8_000, vec![Frame::from_mono(0.0), Frame::from_mono(0.5)]),
            SampleLoadOptions::default(),
        );
        let (trigger_sender, trigger_receiver) = audio_trigger_channel();
        let (audio, mut renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8_000, 8)).unwrap();
        let mut audio_engine = AudioTriggerResolver::new(trigger_receiver, bank, audio);

        trigger_sender
            .send(ScheduledAudioEvent::start_voice(
                crate::application::audio::AudioVoicePlan::from_live_sample(
                    crate::application::audio::VoiceInstanceId::new(1),
                    &SampleIntent::new("kick"),
                    &ControlMap::new(),
                    None,
                    None,
                    Duration::ZERO,
                )
                .unwrap(),
            ))
            .unwrap();

        assert!(audio_engine.tick());

        let mut out = vec![Frame::ZERO; 2];
        renderer.render(&mut out);

        assert!(out.iter().any(|frame| *frame != Frame::ZERO));
    }

    #[test]
    fn tick_stops_when_trigger_channel_disconnects() {
        let bank = SampleBank::new();
        let (trigger_sender, trigger_receiver) = audio_trigger_channel();
        drop(trigger_sender);
        let (audio, _renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8_000, 8)).unwrap();
        let mut audio_engine = AudioTriggerResolver::new(trigger_receiver, bank, audio);

        assert!(!audio_engine.tick());
    }

    #[test]
    fn tick_routes_synth_triggers_without_sample_resolution() {
        let bank = SampleBank::new();
        let (trigger_sender, trigger_receiver) = audio_trigger_channel();
        let (audio, mut renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8_000, 8)).unwrap();
        let mut audio_engine = AudioTriggerResolver::new(trigger_receiver, bank, audio);

        trigger_sender
            .send(ScheduledAudioEvent::start_voice(
                crate::application::audio::AudioVoicePlan::from_live_synth(
                    crate::application::audio::VoiceInstanceId::new(2),
                    BuiltInSynthSource::Sine,
                    &ControlMap::new(),
                    None,
                    None,
                    Duration::ZERO,
                )
                .unwrap(),
            ))
            .unwrap();

        assert!(audio_engine.tick());

        let mut out = vec![Frame::ZERO; 16];
        renderer.render(&mut out);

        assert!(out.iter().any(|frame| *frame != Frame::ZERO));
    }
}
