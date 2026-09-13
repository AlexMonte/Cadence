//! Frame-addressed render queue. Rendering belongs on a host render worker;
//! device callbacks should consume the bounded output ring from `output`.

use super::inserts::PreparedInsertChain;
use crate::adapter::{
    audio::{AudioMixer, Frame},
    sample_bank::LoadedSampleTrigger,
};
use crate::application::{
    audio::{AudioRuntimeControlDelta, VoiceInstanceId},
    synth::SynthTrigger,
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Sample rate and maximum queued commands (including future commands).
pub struct AudioRendererSettings {
    /// Negotiated output sample rate.
    pub sample_rate: u32,
    /// Capacity of each bounded command staging area.
    pub queue_capacity: usize,
}
impl AudioRendererSettings {
    /// Creates renderer settings.
    pub fn new(sample_rate: u32, queue_capacity: usize) -> Self {
        Self {
            sample_rate,
            queue_capacity,
        }
    }
}

#[derive(Debug, Clone)]
/// Work prepared by the control loop for the render worker.
pub enum RenderCommand {
    /// Start a resolved sample.
    Play(LoadedSampleTrigger),
    /// Start a synth.
    PlaySynth(SynthTrigger),
    /// Update one active voice.
    UpdateVoiceControls {
        /// Target voice.
        voice_id: VoiceInstanceId,
        /// Changed values.
        delta: AudioRuntimeControlDelta,
    },
    /// Release one voice.
    ReleaseVoice(VoiceInstanceId),
    /// Replace the single audition slot with a sample, preserving song voices.
    AuditionSample(LoadedSampleTrigger),
    /// Replace the single audition slot with a synth, preserving song voices.
    AuditionSynth(SynthTrigger),
    /// Silence the single audition slot without touching the song.
    StopAudition,
    /// Publish a complete score revision and reset prior voices/effect history.
    ReplaceScoreRevision(u64),
}

struct TimedCommand {
    inserts: PreparedInsertChain,
    frame: u64,
    generation: u64,
    revision: u64,
    command: RenderCommand,
}

#[derive(Default)]
pub(crate) struct RenderClock {
    pub rendered: AtomicU64,
    pub played: AtomicU64,
    pub device_driven: AtomicBool,
    pub generation: AtomicU64,
    pub panic_generation: AtomicU64,
    pub score_revision: AtomicU64,
    pub late_commands: AtomicU64,
    pub rejected_commands: AtomicU64,
    pub resource_limit_hits: AtomicU64,
}

#[derive(Clone)]
/// Control-side queue handle. Its mutex is never used by the audio callback.
pub struct RenderCommandSender {
    sample_rate: u32,
    producer: Arc<Mutex<Producer<TimedCommand>>>,
    pub(crate) clock: Arc<RenderClock>,
}
impl RenderCommandSender {
    /// Schedules a command at an absolute output frame. Commands may arrive out of order.
    pub fn schedule(&self, frame: u64, command: RenderCommand) -> Result<(), RenderCommandError> {
        // Coefficients are immutable and device-rate-specific before entering
        // the queue. Rendering only creates fixed-size zeroed state from them.
        let inserts = match &command {
            RenderCommand::Play(loaded) | RenderCommand::AuditionSample(loaded) => {
                PreparedInsertChain::new(&loaded.trigger.inserts, self.sample_rate)
            }
            RenderCommand::PlaySynth(trigger) | RenderCommand::AuditionSynth(trigger) => {
                PreparedInsertChain::new(&trigger.inserts, self.sample_rate)
            }
            _ => PreparedInsertChain::default(),
        };
        let mut producer = self
            .producer
            .lock()
            .map_err(|_| RenderCommandError::QueuePoisoned)?;
        producer
            .push(TimedCommand {
                inserts,
                frame,
                generation: self.clock.generation.load(Ordering::Acquire),
                revision: self.clock.score_revision.load(Ordering::Acquire),
                command,
            })
            .map_err(|_| {
                self.clock.rejected_commands.fetch_add(1, Ordering::Relaxed);
                RenderCommandError::QueueFull
            })
    }
    fn immediate(&self, command: RenderCommand) -> Result<(), RenderCommandError> {
        self.schedule(self.clock.rendered.load(Ordering::Acquire), command)
    }
    /// Queues a sample for the next unrendered frame.
    pub fn play(&self, loaded: LoadedSampleTrigger) -> Result<(), RenderCommandError> {
        self.immediate(RenderCommand::Play(loaded))
    }
    /// Queues a synth for the next unrendered frame.
    pub fn play_synth(&self, trigger: SynthTrigger) -> Result<(), RenderCommandError> {
        self.immediate(RenderCommand::PlaySynth(trigger))
    }
    /// Updates one voice on the next unrendered frame.
    pub fn update_voice_controls(
        &self,
        voice_id: VoiceInstanceId,
        delta: AudioRuntimeControlDelta,
    ) -> Result<(), RenderCommandError> {
        self.immediate(RenderCommand::UpdateVoiceControls { voice_id, delta })
    }
    /// Releases one voice on the next unrendered frame.
    pub fn release_voice(&self, voice_id: VoiceInstanceId) -> Result<(), RenderCommandError> {
        self.immediate(RenderCommand::ReleaseVoice(voice_id))
    }
}

#[derive(Clone)]
/// Host control handle and observable device frame clock.
pub struct AudioControl {
    sample_rate: u32,
    sender: RenderCommandSender,
}
impl AudioControl {
    pub(crate) fn new(sample_rate: u32, sender: RenderCommandSender) -> Self {
        Self {
            sample_rate,
            sender,
        }
    }
    /// Negotiated renderer sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    /// Frames consumed by the device, or rendered by a direct/offline host.
    pub fn playback_frame(&self) -> u64 {
        if self.sender.clock.device_driven.load(Ordering::Acquire) {
            self.sender.clock.played.load(Ordering::Acquire)
        } else {
            self.next_render_frame()
        }
    }
    /// First frame not yet rendered. Live input starts here.
    pub fn next_render_frame(&self) -> u64 {
        self.sender.clock.rendered.load(Ordering::Acquire)
    }
    /// Advances the score revision at a future frame, preserving old sound before it.
    pub fn publish_revision_at(&self, frame: u64) -> Result<(), RenderCommandError> {
        let revision = self
            .sender
            .clock
            .score_revision
            .load(Ordering::Acquire)
            .saturating_add(1);
        self.schedule(frame, RenderCommand::ReplaceScoreRevision(revision))?;
        self.sender
            .clock
            .score_revision
            .store(revision, Ordering::Release);
        Ok(())
    }
    /// Number of commands received after their requested onset.
    pub fn late_command_count(&self) -> u64 {
        self.sender.clock.late_commands.load(Ordering::Relaxed)
    }
    /// Number of rejected commands due to full queues.
    pub fn rejected_command_count(&self) -> u64 {
        self.sender.clock.rejected_commands.load(Ordering::Relaxed)
    }
    /// Rejected voices or effect-send frames due to the mixer resource bounds.
    pub fn resource_limit_count(&self) -> u64 {
        self.sender
            .clock
            .resource_limit_hits
            .load(Ordering::Relaxed)
    }

    /// Invalidates pending commands and old output, fading to silence over eight milliseconds.
    pub fn cancel_all(&self) {
        self.sender.clock.generation.fetch_add(1, Ordering::AcqRel);
    }
    /// Invalidates pending commands and silences the next device callback immediately.
    pub fn panic(&self) {
        let generation = self.sender.clock.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.sender
            .clock
            .panic_generation
            .store(generation, Ordering::Release);
    }
    /// Schedules a prepared operation at an absolute output frame.
    pub fn schedule(&self, frame: u64, command: RenderCommand) -> Result<(), RenderCommandError> {
        self.sender.schedule(frame, command)
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

/// Worker/offline renderer. Commands are applied at their exact frame, independent of block size.
pub struct AudioRenderer {
    sender: RenderCommandSender,
    consumer: Consumer<TimedCommand>,
    pending: Vec<TimedCommand>,
    capacity: usize,
    mixer: AudioMixer,
    sample_rate: u32,
    generation: u64,
    score_revision: u64,
    last_frame: Frame,
    fade_remaining: usize,
}
impl AudioRenderer {
    /// Creates the control and worker halves.
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
            sample_rate,
            producer: Arc::new(Mutex::new(producer)),
            clock: Arc::new(RenderClock::default()),
        };
        Ok(Self {
            sender,
            consumer,
            pending: Vec::with_capacity(queue_capacity),
            capacity: queue_capacity,
            mixer: AudioMixer::new(sample_rate),
            sample_rate,
            generation: 0,
            score_revision: 0,
            last_frame: Frame::ZERO,
            fade_remaining: 0,
        })
    }
    pub(crate) fn command_sender(&self) -> RenderCommandSender {
        self.sender.clone()
    }
    /// Negotiated output rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    /// Host processing hook.
    pub fn on_start_processing(&mut self) {}
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn active_generation(&self) -> u64 {
        self.generation
    }
    /// Next absolute frame.
    pub fn frame_position(&self) -> u64 {
        self.sender.clock.rendered.load(Ordering::Acquire)
    }
    /// Mixes the next block, splitting at every command onset.
    pub fn render(&mut self, out: &mut [Frame]) {
        let generation = self.sender.clock.generation.load(Ordering::Acquire);
        if generation != self.generation {
            self.generation = generation;
            self.pending.clear();
            self.score_revision = self.sender.clock.score_revision.load(Ordering::Acquire);
            self.mixer.clear();
            self.fade_remaining = if self.sender.clock.device_driven.load(Ordering::Acquire)
                || self.sender.clock.panic_generation.load(Ordering::Acquire) == generation
            {
                0
            } else {
                self.fade_frames()
            };
        }
        while self.pending.len() < self.capacity {
            let Ok(command) = self.consumer.pop() else {
                break;
            };
            if command.generation != generation {
                continue;
            }
            let index = self
                .pending
                .partition_point(|pending| pending.frame <= command.frame);
            self.pending.insert(index, command);
        }
        let start = self.frame_position();
        let end = start.saturating_add(out.len() as u64);
        let mut cursor = start;
        while cursor < end {
            while self
                .pending
                .first()
                .is_some_and(|next| next.frame <= cursor)
            {
                let timed = self.pending.remove(0);
                if timed.frame < cursor {
                    self.sender
                        .clock
                        .late_commands
                        .fetch_add(1, Ordering::Relaxed);
                }
                if timed.revision < self.score_revision
                    && !matches!(timed.command, RenderCommand::ReplaceScoreRevision(_))
                {
                    continue;
                }
                match timed.command {
                    RenderCommand::Play(loaded) => self.mixer.push_prepared(loaded, timed.inserts),
                    RenderCommand::PlaySynth(trigger) => {
                        self.mixer.push_synth_prepared(trigger, timed.inserts)
                    }
                    RenderCommand::UpdateVoiceControls { voice_id, delta } => {
                        self.mixer.update_voice_controls(voice_id, delta)
                    }
                    RenderCommand::ReleaseVoice(voice_id) => self.mixer.release_voice(voice_id),
                    RenderCommand::AuditionSample(loaded) => {
                        self.mixer.audition_sample(loaded, timed.inserts)
                    }
                    RenderCommand::AuditionSynth(trigger) => {
                        self.mixer.audition_synth(trigger, timed.inserts)
                    }
                    RenderCommand::StopAudition => self.mixer.stop_audition(),
                    RenderCommand::ReplaceScoreRevision(revision) => {
                        self.score_revision = revision;
                        self.mixer.clear();
                    }
                }
            }
            let next = self.pending.first().map_or(end, |next| next.frame.min(end));
            let slice = &mut out[(cursor - start) as usize..(next - start) as usize];
            self.mixer.render(slice);
            for frame in slice {
                if self.fade_remaining > 0 {
                    *frame +=
                        self.last_frame * (self.fade_remaining as f32 / self.fade_frames() as f32);
                    self.fade_remaining -= 1;
                }
            }
            cursor = next;
        }
        if self.fade_remaining == 0 {
            self.last_frame = out.last().copied().unwrap_or(self.last_frame);
        }
        self.sender
            .clock
            .resource_limit_hits
            .fetch_add(self.mixer.take_resource_limit_hits(), Ordering::Relaxed);
        self.sender.clock.rendered.store(end, Ordering::Release);
    }
    fn fade_frames(&self) -> usize {
        (self.sample_rate as usize * 8 / 1000).max(1)
    }
}

#[derive(Debug, Error)]
/// Invalid renderer configuration.
pub enum AudioRendererError {
    /// Zero sample rate.
    #[error("audio renderer sample rate must be positive")]
    InvalidSampleRate,
    /// Empty queue.
    #[error("audio renderer queue capacity must be positive")]
    InvalidQueueCapacity,
}
#[derive(Debug, Error)]
/// Control-side queue rejection.
pub enum RenderCommandError {
    /// Queue has reached its fixed bound.
    #[error("render command queue is full")]
    QueueFull,
    /// Control-side sender mutex was poisoned.
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
            sustain_loop: None,
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
            sustain_loop: None,
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
    fn stereo_impulse(sample_rate: u32) -> LoadedSampleTrigger {
        LoadedSampleTrigger {
            sustain_loop: None,
            trigger: SampleTrigger::builder()
                .sample("pulse")
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
            sample_key: "pulse".into(),
            sample: Arc::new(SampleBuffer::new(
                sample_rate,
                vec![Frame::new(0.25, -0.5); 4],
            )),
            playback_limit: None,
            choke_group: None,
        }
    }

    #[test]
    fn exact_onsets_are_block_independent_at_supported_device_rates() {
        for rate in [44_100, 48_000, 96_000] {
            let onset = rate as usize / 100;
            let mut reference = None;
            for block_size in [1, 7, 64, 257] {
                let (audio, mut renderer) =
                    AudioRenderer::split(AudioRendererSettings::new(rate, 8)).unwrap();
                // Deliberately enqueue in the opposite order to playback.
                audio
                    .schedule(
                        (onset + 17) as u64,
                        RenderCommand::Play(stereo_impulse(rate)),
                    )
                    .unwrap();
                audio
                    .schedule(onset as u64, RenderCommand::Play(stereo_impulse(rate)))
                    .unwrap();
                let mut output = vec![Frame::ZERO; onset + 24];
                for block in output.chunks_mut(block_size) {
                    renderer.render(block);
                }
                assert!(output[..onset].iter().all(|frame| *frame == Frame::ZERO));
                assert_eq!(output[onset], Frame::new(0.25, -0.5));
                assert_eq!(output[onset + 17], Frame::new(0.25, -0.5));
                if let Some(reference) = &reference {
                    assert_eq!(&output, reference);
                } else {
                    reference = Some(output);
                }
                assert_eq!(audio.late_command_count(), 0);
            }
        }
    }

    #[test]
    fn cancellation_invalidates_future_commands_even_when_queue_is_full() {
        let (audio, mut renderer) =
            AudioRenderer::split(AudioRendererSettings::new(1000, 2)).unwrap();
        audio
            .schedule(10, RenderCommand::Play(stereo_impulse(1000)))
            .unwrap();
        audio
            .schedule(20, RenderCommand::Play(stereo_impulse(1000)))
            .unwrap();
        assert!(matches!(
            audio.schedule(30, RenderCommand::Play(stereo_impulse(1000))),
            Err(RenderCommandError::QueueFull)
        ));
        audio.cancel_all();
        let mut output = [Frame::ZERO; 40];
        renderer.render(&mut output);
        assert!(output.iter().all(|frame| *frame == Frame::ZERO));
        assert_eq!(audio.rejected_command_count(), 1);
    }

    #[test]
    fn revision_boundary_keeps_old_onsets_before_it_and_discards_old_future_onsets() {
        let (audio, mut renderer) =
            AudioRenderer::split(AudioRendererSettings::new(1000, 8)).unwrap();
        audio
            .schedule(2, RenderCommand::Play(stereo_impulse(1000)))
            .unwrap();
        audio
            .schedule(20, RenderCommand::Play(stereo_impulse(1000)))
            .unwrap();
        audio.publish_revision_at(10).unwrap();
        audio
            .schedule(12, RenderCommand::Play(stereo_impulse(1000)))
            .unwrap();
        let mut output = [Frame::ZERO; 30];
        renderer.render(&mut output);
        assert_ne!(output[2], Frame::ZERO);
        assert_ne!(output[12], Frame::ZERO);
        assert_eq!(output[20], Frame::ZERO);
    }
    #[test]
    fn source_rate_is_resampled_to_the_negotiated_device_rate() {
        for rate in [44_100, 48_000, 96_000] {
            let (audio, mut renderer) =
                AudioRenderer::split(AudioRendererSettings::new(rate, 8)).unwrap();
            let mut loaded = stereo_impulse(44_100);
            loaded.sample = Arc::new(SampleBuffer::new(44_100, vec![Frame::new(0.25, -0.5); 441]));
            audio.schedule(0, RenderCommand::Play(loaded)).unwrap();
            let expected = rate as usize / 100;
            let mut output = vec![Frame::ZERO; expected + 4];
            renderer.render(&mut output);
            assert_ne!(
                output[expected - 1],
                Frame::ZERO,
                "sample shortened at {rate} Hz"
            );
            assert!(
                output[expected + 1..]
                    .iter()
                    .all(|frame| *frame == Frame::ZERO),
                "sample length changed at {rate} Hz"
            );
        }
    }
}
