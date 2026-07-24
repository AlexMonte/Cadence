//! Audio-trigger resolution into renderer commands.

use std::sync::mpsc::TryRecvError;

use crate::{
    adapter::{audio::AudioControl, sample_bank::SampleBank},
    application::{
        audio::{AudioTrigger, AudioTriggerReceiver},
        sample::SampleTrigger,
        synth::SynthTrigger,
    },
};

/// Pulls mixed audio triggers from the application queue and resolves them into
/// renderer commands.
pub struct AudioTriggerResolver {
    trigger_receiver: AudioTriggerReceiver,
    sample_bank: SampleBank,
    audio: AudioControl,
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
        }
    }

    /// Drains currently available triggers.
    ///
    /// Returns `false` when the trigger source has disconnected permanently.
    pub fn tick(&mut self) -> bool {
        loop {
            match self.trigger_receiver.try_recv() {
                Ok(trigger) => self.handle_trigger(trigger),
                Err(TryRecvError::Empty) => return true,
                Err(TryRecvError::Disconnected) => return false,
            }
        }
    }

    fn handle_trigger(&mut self, trigger: AudioTrigger) {
        match trigger {
            AudioTrigger::StartVoice(plan) => self.handle_voice_plan(plan),
            AudioTrigger::UpdateVoiceControls { voice_id, delta } => {
                if let Err(error) = self.audio.update_voice_controls(voice_id, delta) {
                    eprintln!("failed to enqueue voice update: {error}");
                }
            }
            AudioTrigger::ReleaseVoice(voice_id) => {
                if let Err(error) = self.audio.release_voice(voice_id) {
                    eprintln!("failed to enqueue voice release: {error}");
                }
            }
        }
    }

    fn handle_voice_plan(&mut self, plan: crate::application::audio::AudioVoicePlan) {
        match &plan.source {
            crate::application::audio::AudioSourcePlan::Sample(_) => {
                let Some(trigger) = SampleTrigger::from_voice_plan(&plan) else {
                    return;
                };
                let loaded = match self.sample_bank.resolve_trigger(&trigger) {
                    Some(loaded) => loaded,
                    None => {
                        eprintln!("failed to find decoded sample for trigger '{trigger:?}'");
                        return;
                    }
                };

                if let Err(error) = self.audio.play_loaded(loaded) {
                    eprintln!("failed to enqueue render command: {error}");
                }
            }
            crate::application::audio::AudioSourcePlan::Synth(_) => {
                let Some(trigger) = SynthTrigger::from_voice_plan(&plan) else {
                    return;
                };
                if let Err(error) = self.audio.play_synth(trigger) {
                    eprintln!("failed to enqueue synth render command: {error}");
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
        application::audio::{AudioTrigger, audio_trigger_channel},
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
            .send(AudioTrigger::StartVoice(
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
            .send(AudioTrigger::StartVoice(
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
