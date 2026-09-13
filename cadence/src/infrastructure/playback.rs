use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::{
    adapter::{
        audio::{AudioControl, AudioTriggerResolver, RenderCommandError},
        sample_bank::SampleBank as RuntimeSampleBank,
    },
    application::{
        audio::{
            AudioRuntimeControlDelta, AudioSourcePlan, AudioTriggerOutputPerformer, AudioVoicePlan,
            VoiceInstanceId, audio_trigger_channel,
        },
        engine::{Engine, EngineStatus},
        preparation::PreparedScore,
        renderer_core::RendererCore,
        sample::SampleTrigger,
        scheduler::events::event_sink::{EventSink, Sink},
        synth::SynthTrigger,
    },
    domain::{
        control::{ControlKey, ControlMap, ControlModelError, ControlValue},
        input::{ControlInput, HeldNotes, InputEvent, NoteInput, SampleNoteMap},
        intent::{BuiltInSynthSource, Intent},
        prelude::Time,
    },
};

pub use crate::adapter::audio::{Frame, SampleBuffer};
pub use crate::adapter::sample_bank::{
    ChokeGroup, SampleBank, SampleLoadOptions, SampleLoopRegion, SamplePitchRange,
};

/// Observable playback state for host crates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// Transport is stopped and considered to be at cycle zero.
    Stopped,
    /// Transport is paused at its current cycle position.
    Paused,
    /// Transport is actively playing.
    Playing,
}

/// Exact cycle-domain playback configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackSettings {
    /// Transport rate in cycles per second.
    pub cps: Time,
    /// Scheduler look-ahead window in cycles.
    pub look_ahead: Time,
    /// Scheduler polling step size in cycles.
    pub step: Time,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            cps: Time::new(2, 1),
            look_ahead: Time::new(1, 2),
            step: Time::new(1, 16),
        }
    }
}

/// Snapshot of the runtime transport from the host perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackStatus {
    /// Current playback state.
    pub state: PlaybackState,
    /// Current transport position in cycles.
    pub cycle_position: Time,
    /// Current transport rate in cycles per second.
    pub cps: Time,
}

#[derive(Debug, thiserror::Error)]
/// Errors surfaced by the host-facing playback runtime.
pub enum PlaybackError {
    /// Error while sending a render command to the audio thread.
    #[error("failed to send render command: {0}")]
    RenderCommand(#[from] RenderCommandError),
    /// A required decoded sample is missing or a bounded queue rejected work.
    #[error(transparent)]
    AudioResolution(#[from] crate::adapter::audio::AudioTriggerResolveError),
    /// The score exceeds the bounded audio preparation budget.
    #[error("{0}")]
    ScheduleLimit(#[from] crate::application::preparation::AudioScheduleError),
    /// Error while resolving the current control model through projection/scheduling.
    #[error("invalid control model: {0}")]
    ControlModel(#[from] ControlModelError),
    /// A preview request uses unsupported intent, pitch, duration or source settings.
    #[error("invalid sound preview: {0}")]
    InvalidAudition(&'static str),
}

/// Host-facing manual control runtime.
///
/// Expected host flow:
/// 1. Lower Cadence/Tessera meaning into typed `Score` and normalized live
///    `InputEvent` values
/// 2. Decode samples outside `cadence` when sample-backed playback is needed
/// 3. Create `(audio, renderer)` with `AudioRenderer::split(...)`
/// 4. Create `PlaybackRuntime::new(settings, audio)`
/// 5. Run `renderer` on the host-owned audio thread/worklet
/// 6. Run `tick()` from the host-owned control loop
///
/// `PlaybackRuntime` is the preferred host-facing runtime path. The direct
/// trigger seam in `infrastructure::audio` remains available for advanced host
/// wiring, but score-first playback and normalized input are the canonical
/// integration story.
pub struct PlaybackRuntime {
    settings: PlaybackSettings,
    frame_anchor: u64,
    cycle_anchor: Time,
    last_prepared_cycle: Time,
    pending_renderer: Option<(Time, RendererCore)>,
    published_boundary: Option<Time>,
    bank: RuntimeSampleBank,
    audio: AudioControl,
    engine: Engine<Sink, AudioTriggerOutputPerformer>,
    resolver: AudioTriggerResolver,
    ambient_controls: Arc<RwLock<ControlMap>>,
    sample_note_map: SampleNoteMap,
    live_synth_source: Option<BuiltInSynthSource>,
    held_notes: HeldNotes,
    sustained_notes: HeldNotes,
    live_voice_ids: BTreeMap<crate::domain::input::NoteNumber, Vec<VoiceInstanceId>>,
    audition_pending: bool,
}

impl PlaybackRuntime {
    /// Creates a stopped playback runtime.
    #[must_use]
    pub fn new(settings: PlaybackSettings, audio: AudioControl) -> Self {
        Self::with_sample_bank(settings, audio, RuntimeSampleBank::new())
    }

    /// Creates a runtime using a caller-owned decoded sample bank.
    #[must_use]
    pub fn with_sample_bank(
        settings: PlaybackSettings,
        audio: AudioControl,
        bank: SampleBank,
    ) -> Self {
        assert!(
            settings.look_ahead <= Time::ONE && settings.step <= Time::ONE,
            "playback look-ahead and step must not exceed one cycle"
        );
        let ambient_controls = Arc::new(RwLock::new(ControlMap::new()));
        let (trigger_sender, trigger_receiver) = audio_trigger_channel();
        let performer = trigger_sender.performer_with_ambient(Arc::clone(&ambient_controls));
        let resolver = AudioTriggerResolver::new(trigger_receiver, bank.clone(), audio.clone());
        let mut engine = Engine::new(
            RendererCore::empty(),
            Sink::new(),
            settings.look_ahead,
            settings.cps,
            performer,
            settings.step,
        );
        engine.stop();

        Self {
            settings,
            frame_anchor: audio.next_render_frame(),
            cycle_anchor: Time::ZERO,
            last_prepared_cycle: Time::ZERO,
            pending_renderer: None,
            published_boundary: None,
            bank,
            audio,
            engine,
            resolver,
            ambient_controls,
            sample_note_map: SampleNoteMap::new(),
            live_synth_source: None,
            held_notes: HeldNotes::default(),
            sustained_notes: HeldNotes::default(),
            live_voice_ids: BTreeMap::new(),
            audition_pending: false,
        }
    }

    /// Returns a handle to the runtime sample bank.
    #[must_use]
    pub fn sample_bank(&self) -> SampleBank {
        self.bank.clone()
    }

    /// Previews one sample or synth in a reserved slot without changing transport,
    /// the score, or keyboard mappings. A new preview replaces the prior one.
    ///
    /// Duration must be positive and at most two seconds; one-shot samples can
    /// end earlier. Optional pitch uses finite MIDI-note values in [0, 127].
    /// Commands start at the next unrendered device frame. Panic silences previews.
    pub fn audition(
        &mut self,
        intent: Intent,
        pitch: Option<f64>,
        duration: Duration,
    ) -> Result<(), PlaybackError> {
        if duration.is_zero() || duration > Duration::from_secs(2) {
            return Err(PlaybackError::InvalidAudition(
                "duration must be positive and at most two seconds",
            ));
        }
        if pitch.is_some_and(|value| !value.is_finite() || !(0.0..=127.0).contains(&value)) {
            return Err(PlaybackError::InvalidAudition(
                "pitch must be finite and within MIDI notes 0–127",
            ));
        }
        let mut controls = ControlMap::new();
        if let Some(pitch) = pitch {
            controls.insert(ControlKey::Pitch, ControlValue::Scalar(pitch));
        }
        let voice_id = VoiceInstanceId::next_live();
        let mut plan = match &intent {
            Intent::Sample(source) => {
                if !source.rate.is_finite()
                    || source.rate <= 0.0
                    || source.rate > 65_536.0
                    || !source.gain.is_finite()
                    || source.gain < 0.0
                    || !source.start.is_finite()
                    || !source.end.is_finite()
                    || source.start < 0.0
                    || source.start >= source.end
                    || source.end > 1.0
                {
                    return Err(PlaybackError::InvalidAudition(
                        "sample rate, gain and region must be valid",
                    ));
                }
                AudioVoicePlan::from_live_sample(voice_id, source, &controls, None, None, duration)
            }
            Intent::Synth(source) => AudioVoicePlan::from_live_synth_intent(
                voice_id, source, &controls, None, None, duration,
            ),
            _ => {
                return Err(PlaybackError::InvalidAudition(
                    "preview supports sample or synth intents",
                ));
            }
        }
        .ok_or(PlaybackError::InvalidAudition(
            "could not prepare preview voice",
        ))?;
        plan.mix.gain *= 0.65;
        let fade = Duration::from_millis(5).min(duration / 4);
        plan.envelope.attack = plan.envelope.attack.max(fade).min(duration / 2);
        plan.envelope.release = plan.envelope.release.max(fade).min(duration / 2);
        plan.lifecycle.gate_duration = Some(duration - plan.envelope.release);
        let command = match &plan.source {
            AudioSourcePlan::Sample(_) => {
                let trigger = SampleTrigger::from_voice_plan(&plan).ok_or(
                    PlaybackError::InvalidAudition("could not prepare sample preview"),
                )?;
                let loaded = self.bank.resolve_trigger(&trigger).ok_or_else(|| {
                    crate::adapter::audio::AudioTriggerResolveError::MissingSample(
                        trigger.sample.clone(),
                    )
                })?;
                if !loaded.trigger.playback_rate.is_finite()
                    || loaded.trigger.playback_rate <= 0.0
                    || loaded.trigger.playback_rate > 65_536.0
                {
                    return Err(PlaybackError::InvalidAudition(
                        "resolved sample rate must be finite, positive and at most 65536",
                    ));
                }
                crate::adapter::audio::RenderCommand::AuditionSample(loaded)
            }
            AudioSourcePlan::Synth(_) => crate::adapter::audio::RenderCommand::AuditionSynth(
                SynthTrigger::from_voice_plan(&plan).ok_or(PlaybackError::InvalidAudition(
                    "could not prepare synth preview",
                ))?,
            ),
        };
        self.audio
            .schedule(self.audio.next_render_frame(), command)?;
        self.audition_pending = true;
        Ok(())
    }

    /// Plays pre-rendered stereo audio in the reserved preview slot without adding
    /// a sample-bank entry. The caller owns song pause/resume and preview scope.
    /// Returns the device-frame deadline; output buffering can add device latency.
    pub fn audition_buffer(&mut self, sample: SampleBuffer) -> Result<u64, PlaybackError> {
        let duration = sample.duration();
        if sample.is_empty() || duration > Duration::from_secs(30) {
            return Err(PlaybackError::InvalidAudition(
                "rendered preview must last 0–30 seconds",
            ));
        }
        if sample
            .frames()
            .iter()
            .any(|frame| !frame.left.is_finite() || !frame.right.is_finite())
        {
            return Err(PlaybackError::InvalidAudition(
                "rendered preview contains a non-finite sample",
            ));
        }
        let start = self.audio.next_render_frame();
        let frames = (duration.as_secs_f64() * f64::from(self.audio.sample_rate())).ceil() as u64;
        let end = start
            .checked_add(frames)
            .ok_or(PlaybackError::InvalidAudition(
                "preview frame range overflow",
            ))?;
        let trigger = SampleTrigger::builder()
            .sample("rendered-preview")
            .play_for(duration)
            .build()
            .ok_or(PlaybackError::InvalidAudition(
                "could not prepare rendered preview",
            ))?;
        self.audio.schedule(
            start,
            crate::adapter::audio::RenderCommand::AuditionSample(
                crate::adapter::sample_bank::LoadedSampleTrigger {
                    trigger,
                    sample_key: "rendered-preview".into(),
                    sample: Arc::new(sample),
                    playback_limit: Some(duration),
                    choke_group: None,
                    sustain_loop: None,
                },
            ),
        )?;
        self.audition_pending = true;
        Ok(end)
    }

    /// Next unrendered device frame, for observing a rendered-preview deadline.
    pub fn rendered_audio_frame(&self) -> u64 {
        self.audio.next_render_frame()
    }

    /// Stops the reserved preview slot, preserving song and live-keyboard voices.
    pub fn stop_audition(&mut self) -> Result<(), PlaybackError> {
        if self.audition_pending {
            self.audio.schedule(
                self.audio.next_render_frame(),
                crate::adapter::audio::RenderCommand::StopAudition,
            )?;
            self.audition_pending = false;
        }
        Ok(())
    }

    /// Loads a decoded sample with default load options.
    pub fn load_sample(&self, name: impl Into<String>, sample: SampleBuffer) {
        self.bank.load(name, sample);
    }

    /// Loads a decoded sample with explicit load options.
    pub fn load_sample_with_options(
        &self,
        name: impl Into<String>,
        sample: SampleBuffer,
        options: SampleLoadOptions,
    ) {
        self.bank.load_with_options(name, sample, options);
    }

    /// Replaces the live note-to-sample mapping.
    pub fn set_sample_note_map(&mut self, map: SampleNoteMap) {
        self.sample_note_map = map;
    }

    /// Sets the synth source used for live played notes.
    pub fn set_live_synth_source(&mut self, source: BuiltInSynthSource) {
        self.live_synth_source = Some(source);
    }

    /// Clears the live synth source.
    pub fn clear_live_synth_source(&mut self) {
        self.live_synth_source = None;
    }

    /// Returns the currently configured live synth source.
    #[must_use]
    pub fn live_synth_source(&self) -> Option<BuiltInSynthSource> {
        self.live_synth_source
    }

    /// Returns whether a live note is currently held.
    #[must_use]
    pub fn is_note_held(&self, note: crate::domain::input::NoteNumber) -> bool {
        self.held_notes.contains(note)
    }

    /// Advances scheduling and drains ready audio triggers into the renderer
    /// queue.
    pub fn tick(&mut self) -> Result<(), PlaybackError> {
        let position = self.device_cycle_position();
        if position - self.last_prepared_cycle > self.settings.look_ahead {
            // A control-loop stall beyond lookahead must not enqueue the missed
            // past as a huge burst. Reconstruct only the device's current window.
            self.audio.cancel_all();
            self.reanchor(position);
            self.engine.scheduler.reset_to(position);
            self.engine.event_sink.clear_pending();
        }
        if self
            .published_boundary
            .is_some_and(|boundary| position >= boundary)
        {
            self.published_boundary = None;
        }
        self.engine.performer.set_frame_timing(
            self.cycle_anchor,
            self.frame_anchor,
            self.settings.cps,
            self.audio.sample_rate(),
        );
        if self
            .pending_renderer
            .as_ref()
            .is_some_and(|(boundary, _)| *boundary <= position + self.settings.look_ahead)
        {
            let (boundary, renderer) = self
                .pending_renderer
                .take()
                .expect("checked pending revision");
            // Fill the final old-score windows before changing revision ownership.
            let through =
                (boundary - self.settings.look_ahead + self.settings.step).max(Time::ZERO);
            self.engine.tick_ahead(
                crate::application::clock::ClockTime::from_time(through),
                self.settings.look_ahead,
            )?;
            let _ = self.resolver.tick();
            let seconds = (boundary - self.cycle_anchor).max(Time::ZERO) / self.settings.cps;
            let frame = self.frame_anchor.saturating_add(
                (seconds.value() * f64::from(self.audio.sample_rate())).round() as u64,
            );
            if let Err(error) = self.audio.publish_revision_at(frame) {
                self.pending_renderer = Some((boundary, renderer));
                return Err(error.into());
            }
            self.engine.replace_renderer(renderer);
            self.engine.scheduler.reset_to(boundary);
            self.published_boundary = Some(boundary);
        }
        self.engine.tick_ahead(
            crate::application::clock::ClockTime::from_time(position),
            self.settings.look_ahead,
        )?;
        let _ = self.resolver.tick();
        if let Some(error) = self.resolver.take_error() {
            return Err(error.into());
        }
        self.last_prepared_cycle = position;
        Ok(())
    }

    /// Musical cycle where the newest valid score revision will become audible.
    pub fn pending_revision_cycle(&self) -> Option<Time> {
        self.pending_renderer
            .as_ref()
            .map(|(boundary, _)| *boundary)
            .or(self.published_boundary)
    }

    /// Returns a host-side hint for how often `tick()` should run.
    #[must_use]
    pub fn tick_interval_hint(&self) -> Duration {
        self.engine.clock.cycles_to_duration(self.settings.step)
    }

    /// Starts playback from a score that has already passed preparation.
    pub fn play_prepared_score(&mut self, score: PreparedScore) -> Result<(), PlaybackError> {
        self.play_renderer(RendererCore::new(score.into_score()))
    }

    /// Replaces playback from a score that has already passed preparation.
    ///
    /// The current transport window is still evaluated before publication so
    /// time-dependent query sources and control models cannot replace the last
    /// playable score with an invalid result.
    pub fn replace_prepared_score(&mut self, score: PreparedScore) -> Result<(), PlaybackError> {
        self.replace_renderer(RendererCore::new(score.into_score()))
    }

    /// Changes the transport rate in cycles per second.
    ///
    /// # Panics
    ///
    /// Panics if `cps <= 0`.
    pub fn set_cps(&mut self, cps: Time) -> Result<(), PlaybackError> {
        assert!(cps > Time::ZERO, "cycles per second must be positive");
        if cps == self.settings.cps {
            return Ok(());
        }
        let position = self.device_cycle_position();
        self.settings.cps = cps;
        self.engine.set_cycles_per_second(cps);
        self.reanchor(position);
        self.engine.scheduler.reset_to(position);
        self.engine.event_sink.clear_pending();
        self.audio.cancel_all();
        Ok(())
    }

    /// Pauses playback.
    pub fn pause(&mut self) -> Result<(), PlaybackError> {
        if !matches!(self.engine.status(), EngineStatus::Playing) {
            return Ok(());
        }
        let position = self.device_cycle_position();
        self.engine
            .clock
            .reset_at(crate::application::clock::ClockTime::from_time(position));
        self.engine.pause();
        self.engine.status =
            EngineStatus::Paused(crate::application::clock::ClockTime::from_time(position));
        self.audio.cancel_all();
        self.reanchor(position);
        Ok(())
    }

    /// Resumes playback.
    pub fn resume(&mut self) -> Result<(), PlaybackError> {
        let position = self.status().cycle_position;
        if !matches!(self.engine.status(), EngineStatus::Playing) {
            self.reanchor(position);
            self.engine.resume();
        }
        Ok(())
    }

    /// Stops transport and resets the observable cycle position to zero.
    pub fn stop(&mut self) -> Result<(), PlaybackError> {
        if matches!(self.engine.status(), EngineStatus::Stopped)
            && self.live_voice_ids.is_empty()
            && self.pending_renderer.is_none()
        {
            return Ok(());
        }
        if let Some((_, renderer)) = self.pending_renderer.take() {
            self.engine.replace_renderer(renderer);
        }
        self.published_boundary = None;
        self.engine.stop();
        self.audio.cancel_all();
        self.reanchor(Time::ZERO);
        self.live_voice_ids.clear();
        self.held_notes = HeldNotes::default();
        self.sustained_notes = HeldNotes::default();
        Ok(())
    }

    /// Immediately silences all output, including pending voices and effect tails.
    pub fn panic(&mut self) -> Result<(), PlaybackError> {
        self.stop()?;
        self.audio.panic();
        self.audition_pending = false;
        Ok(())
    }

    /// Seeks to a nonnegative cycle, reconstructing voices on the next tick.
    /// Effect history is reset, as with pause/resume.
    pub fn seek(&mut self, position: Time) -> Result<(), PlaybackError> {
        let position = position.max(Time::ZERO);
        if let Some((boundary, _)) = &mut self.pending_renderer {
            *boundary = Time::whole_number(position.floor() + 1);
        }
        self.published_boundary = None;
        self.audio.cancel_all();
        self.reanchor(position);
        self.engine.scheduler.reset_to(position);
        self.engine.event_sink.clear_pending();
        if !matches!(self.engine.status(), EngineStatus::Playing) {
            self.engine.status =
                EngineStatus::Paused(crate::application::clock::ClockTime::from_time(position));
        }
        Ok(())
    }

    fn reanchor(&mut self, position: Time) {
        self.frame_anchor = self.audio.next_render_frame();
        self.cycle_anchor = position;
        self.last_prepared_cycle = position;
        self.engine
            .clock
            .reset_at(crate::application::clock::ClockTime::from_time(position));
    }

    fn device_cycle_position(&self) -> Time {
        match self.engine.status() {
            EngineStatus::Stopped => Time::ZERO,
            EngineStatus::Paused(at) => at.as_time(),
            EngineStatus::Playing => {
                self.cycle_anchor
                    + Time::new(
                        i64::try_from(
                            self.audio
                                .playback_frame()
                                .saturating_sub(self.frame_anchor),
                        )
                        .unwrap_or(i64::MAX),
                        i64::from(self.audio.sample_rate()),
                    ) * self.settings.cps
            }
        }
    }

    /// Returns the current host-visible playback status.
    #[must_use]
    pub fn status(&self) -> PlaybackStatus {
        let state = match self.engine.status() {
            EngineStatus::Stopped => PlaybackState::Stopped,
            EngineStatus::Paused(_) => PlaybackState::Paused,
            EngineStatus::Playing => PlaybackState::Playing,
        };

        PlaybackStatus {
            state,
            cycle_position: self.device_cycle_position(),
            cps: self.engine.cycles_per_second(),
        }
    }

    /// Handles one normalized live input event.
    pub fn handle_input(&mut self, input: InputEvent) -> Result<(), PlaybackError> {
        match input {
            InputEvent::Control(ControlInput { key, value, .. }) => {
                self.handle_control_input(key, value)
            }
            InputEvent::Note(NoteInput::On { note, velocity }) => {
                self.held_notes.note_on(note);
                self.sustained_notes.note_off(note);
                let ambient_controls = self
                    .ambient_controls
                    .read()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone();

                if let Some(binding) = self.sample_note_map.resolve(note).cloned() {
                    let mut merged = ambient_controls.clone();
                    merged.extend(binding.controls().clone());
                    merged.insert(
                        ControlKey::Pitch,
                        ControlValue::Scalar(f64::from(note.value())),
                    );
                    let voice_id = VoiceInstanceId::next_live();
                    if let Some(plan) = AudioVoicePlan::from_live_sample(
                        voice_id,
                        &crate::domain::intent::SampleIntent::new(binding.sample()),
                        &merged,
                        Some(velocity.as_unit()),
                        Some(note),
                        Duration::ZERO,
                    ) {
                        self.start_live_voice(plan)?;
                    }
                }

                if let Some(source) = self.live_synth_source {
                    let mut merged = ambient_controls;
                    merged.insert(
                        ControlKey::Pitch,
                        ControlValue::Scalar(f64::from(note.value())),
                    );
                    let voice_id = VoiceInstanceId::next_live();
                    if let Some(plan) = AudioVoicePlan::from_live_synth(
                        voice_id,
                        source,
                        &merged,
                        Some(velocity.as_unit()),
                        Some(note),
                        Duration::ZERO,
                    ) {
                        self.start_live_voice(plan)?;
                    }
                }

                Ok(())
            }
            InputEvent::Note(NoteInput::Off { note }) => {
                self.held_notes.note_off(note);
                if self.sustain_active() {
                    self.sustained_notes.note_on(note);
                } else {
                    self.release_live_note(note)?;
                }
                Ok(())
            }
        }
    }

    fn handle_control_input(
        &mut self,
        key: ControlKey,
        value: ControlValue,
    ) -> Result<(), PlaybackError> {
        let was_sustain_active = self.sustain_active();
        {
            let mut ambient = self
                .ambient_controls
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            ambient.insert(key.clone(), value.clone());
        }

        if let Some(delta) = AudioRuntimeControlDelta::from_live_control(&key, &value) {
            for voice_id in self.active_live_voice_ids() {
                self.audio.update_voice_controls(voice_id, delta.clone())?;
            }
        }

        // Sustain release is edge-triggered: when the pedal goes false, only
        // notes that are no longer physically held should be released.
        if matches!(
            (key, value),
            (ControlKey::SustainPedal, ControlValue::Bool(false))
        ) && was_sustain_active
        {
            self.release_sustained_notes()?;
        }

        Ok(())
    }

    fn sustain_active(&self) -> bool {
        let ambient = self
            .ambient_controls
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        matches!(
            ambient.get(&ControlKey::SustainPedal),
            Some(ControlValue::Bool(true))
        )
    }

    fn release_sustained_notes(&mut self) -> Result<(), PlaybackError> {
        let notes: Vec<_> = self
            .sustained_notes
            .iter()
            .filter(|note| !self.held_notes.contains(*note))
            .collect();

        for note in notes {
            self.release_live_note(note)?;
            self.sustained_notes.note_off(note);
        }

        Ok(())
    }

    fn start_live_voice(&mut self, plan: AudioVoicePlan) -> Result<(), PlaybackError> {
        let voice_id = plan.voice_id;
        let live_note = plan.live_note;

        match &plan.source {
            AudioSourcePlan::Sample(_) => {
                let Some(trigger) = SampleTrigger::from_voice_plan(&plan) else {
                    return Ok(());
                };
                if let Some(loaded) = self.bank.resolve_trigger(&trigger) {
                    self.audio.play_loaded(loaded)?;
                } else {
                    return Ok(());
                }
            }
            AudioSourcePlan::Synth(_) => {
                let Some(trigger) = SynthTrigger::from_voice_plan(&plan) else {
                    return Ok(());
                };
                self.audio.play_synth(trigger)?;
            }
        }

        if let Some(note) = live_note {
            self.live_voice_ids.entry(note).or_default().push(voice_id);
        }

        Ok(())
    }

    fn release_live_note(
        &mut self,
        note: crate::domain::input::NoteNumber,
    ) -> Result<(), PlaybackError> {
        if let Some(voice_ids) = self.live_voice_ids.remove(&note) {
            for voice_id in voice_ids {
                self.audio.release_voice(voice_id)?;
            }
        }
        Ok(())
    }

    fn active_live_voice_ids(&self) -> Vec<VoiceInstanceId> {
        self.live_voice_ids
            .values()
            .flat_map(|voice_ids| voice_ids.iter().copied())
            .collect()
    }

    fn play_renderer(&mut self, renderer: RendererCore) -> Result<(), PlaybackError> {
        self.pending_renderer = None;
        self.published_boundary = None;
        self.audio.cancel_all();
        self.engine.stop();
        self.reanchor(Time::ZERO);
        self.engine.replace_renderer(renderer);
        self.engine.set_cycles_per_second(self.settings.cps);
        self.engine.resume();
        Ok(())
    }

    fn replace_renderer(&mut self, renderer: RendererCore) -> Result<(), PlaybackError> {
        // Validate the complete proposal before changing the last valid score.
        let position = self.device_cycle_position();
        let validation = crate::domain::span::TransportSpan::new(position, position + Time::ONE)
            .expect("positive validation window");
        renderer.evaluate_window(&validation)?;
        if matches!(self.engine.status(), EngineStatus::Playing) {
            let boundary = Time::whole_number(position.floor() + 1);
            self.pending_renderer = Some((boundary, renderer));
            return Ok(());
        }
        let was_stopped = matches!(self.engine.status(), EngineStatus::Stopped);
        self.pending_renderer = None;
        self.published_boundary = None;
        self.audio.cancel_all();
        self.reanchor(position);
        self.engine.replace_renderer(renderer);
        self.engine.scheduler.reset_to(position);
        if was_stopped {
            self.engine.resume();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
        domain::{
            control::{ControlTile, ControlTrack, UnitValue},
            input::{
                ControlInput, InputEvent, InputSourceId, NoteInput, NoteNumber, SampleNoteBinding,
                SampleNoteMap, Velocity,
            },
            intent::{BuiltInSynthSource, Intent},
            score::Score,
            voice::{Tile, Voice, ops},
        },
    };

    fn test_runtime() -> (AudioRenderer, PlaybackRuntime) {
        let (audio, renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8_000, 64)).unwrap();
        let runtime = PlaybackRuntime::new(PlaybackSettings::default(), audio);
        runtime.load_sample(
            "bd",
            SampleBuffer::new(8_000, vec![Frame::from_mono(0.0), Frame::from_mono(0.5)]),
        );
        runtime.load_sample(
            "vox",
            SampleBuffer::new(8_000, vec![Frame::from_mono(0.0), Frame::from_mono(0.5)]),
        );
        (renderer, runtime)
    }

    #[test]
    fn rendered_audition_preserves_stereo_pcm_and_stops_without_bank_entries() {
        let (mut renderer, mut runtime) = test_runtime();
        let frames = vec![Frame::new(0.125, -0.25); 400];
        let end = runtime
            .audition_buffer(SampleBuffer::new(8_000, frames.clone()))
            .unwrap();
        assert_eq!(end, 400);
        assert!(!runtime.sample_bank().contains("rendered-preview"));
        let mut actual = vec![Frame::ZERO; 400];
        renderer.render(&mut actual);
        assert_eq!(actual, frames);
        assert_eq!(runtime.rendered_audio_frame(), end);
        let mut tail = [Frame::ZERO; 80];
        renderer.render(&mut tail);
        assert!(tail.iter().all(|f| *f == Frame::ZERO));
        runtime
            .audition_buffer(SampleBuffer::new(8_000, frames))
            .unwrap();
        renderer.render(&mut tail);
        assert!(block_energy(&tail) > 1.0);
        runtime.stop_audition().unwrap();
        renderer.render(&mut tail);
        assert_eq!(block_energy(&tail), 0.0);
        assert!(
            runtime
                .audition_buffer(SampleBuffer::new(8_000, vec![Frame::new(f32::NAN, 0.)]))
                .is_err()
        );
        assert!(
            runtime
                .audition_buffer(SampleBuffer::new(8_000, vec![]))
                .is_err()
        );
    }

    fn block_energy(frames: &[Frame]) -> f32 {
        frames
            .iter()
            .map(|frame| frame.left.abs() + frame.right.abs())
            .sum()
    }

    fn sample_voice(sample: &str) -> Voice {
        Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample(sample)).unwrap()],
        )
        .unwrap()
    }

    fn synth_score(source: BuiltInSynthSource, pitch: f64) -> Score {
        let voice = Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::synth(source)).unwrap()],
        )
        .unwrap();
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    ControlKey::Pitch,
                    ControlValue::Scalar(pitch),
                )
                .unwrap(),
            ],
        )
        .unwrap();

        Score::with_controls(
            Score::from(voice),
            crate::domain::score::ControlScore::track(controls),
        )
    }

    fn prepared(score: Score) -> PreparedScore {
        PreparedScore::new(score).unwrap()
    }

    #[test]
    fn runtime_starts_stopped_with_no_active_score() {
        let (_renderer, runtime) = test_runtime();

        assert_eq!(runtime.status().state, PlaybackState::Stopped);
        assert_eq!(runtime.status().cycle_position, Time::ZERO);
        assert_eq!(runtime.engine.scheduler.renderer().score(), &Score::empty());
    }

    #[test]
    fn prepared_score_transitions_runtime_to_playing() {
        let (_renderer, mut runtime) = test_runtime();

        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("bd"))))
            .unwrap();

        assert_eq!(runtime.status().state, PlaybackState::Playing);
    }

    #[test]
    fn prepared_replacement_preserves_transport_when_already_playing() {
        let (_renderer, mut runtime) = test_runtime();
        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("bd"))))
            .unwrap();
        runtime.tick().unwrap();
        let before = runtime.status().cycle_position;

        runtime
            .replace_prepared_score(prepared(Score::from(sample_voice("vox"))))
            .unwrap();

        assert_eq!(runtime.status().state, PlaybackState::Playing);
        assert!(runtime.status().cycle_position >= before);
    }

    #[test]
    fn pause_and_resume_update_runtime_state() {
        let (_renderer, mut runtime) = test_runtime();
        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("bd"))))
            .unwrap();

        runtime.pause().unwrap();
        assert_eq!(runtime.status().state, PlaybackState::Paused);

        runtime.resume().unwrap();
        assert_eq!(runtime.status().state, PlaybackState::Playing);
    }

    #[test]
    fn stop_resets_cycle_position_to_zero() {
        let (_renderer, mut runtime) = test_runtime();
        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("bd"))))
            .unwrap();
        runtime.tick().unwrap();

        runtime.stop().unwrap();

        let status = runtime.status();
        assert_eq!(status.state, PlaybackState::Stopped);
        assert_eq!(status.cycle_position, Time::ZERO);
    }

    #[test]
    fn playback_accepts_prepared_scores() {
        let (_renderer, mut runtime) = test_runtime();

        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("bd"))))
            .unwrap();
        assert_eq!(runtime.status().state, PlaybackState::Playing);

        runtime.stop().unwrap();
        let prepared = PreparedScore::new(Score::from(sample_voice("bd"))).unwrap();
        runtime.play_prepared_score(prepared).unwrap();

        assert_eq!(runtime.status().state, PlaybackState::Playing);
    }

    #[test]
    fn playback_accepts_warped_voices_without_a_new_runtime_path() {
        let (_renderer, mut runtime) = test_runtime();
        let warped = ops::fast_by(&sample_voice("bd"), Time::new(2, 1));

        runtime
            .play_prepared_score(prepared(Score::from(warped)))
            .unwrap();

        assert_eq!(runtime.status().state, PlaybackState::Playing);
    }

    #[test]
    fn tick_interval_hint_matches_step_and_current_cps() {
        let (_renderer, mut runtime) = test_runtime();

        assert_eq!(
            runtime.tick_interval_hint(),
            runtime.engine.clock.cycles_to_duration(Time::new(1, 16))
        );

        runtime.set_cps(Time::new(4, 1)).unwrap();

        assert_eq!(
            runtime.tick_interval_hint(),
            runtime.engine.clock.cycles_to_duration(Time::new(1, 16))
        );
    }

    #[test]
    fn handle_input_plays_live_samples_from_note_map() {
        let (_renderer, mut runtime) = test_runtime();
        runtime.set_sample_note_map(
            SampleNoteMap::new().exact(NoteNumber::new(36).unwrap(), SampleNoteBinding::new("bd")),
        );

        runtime
            .handle_input(InputEvent::Note(NoteInput::On {
                note: NoteNumber::new(36).unwrap(),
                velocity: Velocity::new(100).unwrap(),
            }))
            .unwrap();

        assert!(runtime.is_note_held(NoteNumber::new(36).unwrap()));
    }

    #[test]
    fn prepared_score_handles_scheduled_synth_events() {
        let (mut renderer, mut runtime) = test_runtime();

        runtime
            .play_prepared_score(prepared(synth_score(BuiltInSynthSource::Sine, 69.0)))
            .unwrap();
        runtime.tick().unwrap();

        let mut out = vec![Frame::ZERO; 64];
        renderer.render(&mut out);

        assert!(out.iter().any(|frame| *frame != Frame::ZERO));
    }

    #[test]
    fn live_note_input_triggers_configured_synth_source() {
        let (mut renderer, mut runtime) = test_runtime();
        runtime.set_live_synth_source(BuiltInSynthSource::Triangle);

        runtime
            .handle_input(InputEvent::Note(NoteInput::On {
                note: NoteNumber::new(60).unwrap(),
                velocity: Velocity::new(100).unwrap(),
            }))
            .unwrap();

        let mut out = vec![Frame::ZERO; 64];
        renderer.render(&mut out);

        assert!(out.iter().any(|frame| *frame != Frame::ZERO));
    }

    #[test]
    fn sustain_pedal_holds_live_synth_until_pedal_release() {
        let (mut renderer, mut runtime) = test_runtime();
        let note = NoteNumber::new(60).unwrap();
        runtime.set_live_synth_source(BuiltInSynthSource::Triangle);

        runtime
            .handle_input(InputEvent::Control(ControlInput::new(
                ControlKey::SustainPedal,
                ControlValue::Bool(true),
                InputSourceId::from("keyboard"),
                None,
            )))
            .unwrap();
        runtime
            .handle_input(InputEvent::Note(NoteInput::On {
                note,
                velocity: Velocity::new(100).unwrap(),
            }))
            .unwrap();
        runtime
            .handle_input(InputEvent::Note(NoteInput::Off { note }))
            .unwrap();

        let mut sustained = vec![Frame::ZERO; 64];
        renderer.render(&mut sustained);
        assert!(sustained.iter().any(|frame| *frame != Frame::ZERO));

        runtime
            .handle_input(InputEvent::Control(ControlInput::new(
                ControlKey::SustainPedal,
                ControlValue::Bool(false),
                InputSourceId::from("keyboard"),
                None,
            )))
            .unwrap();

        let mut released = vec![Frame::ZERO; 64];
        renderer.render(&mut released);
        assert!(released.iter().all(|frame| *frame == Frame::ZERO));
    }

    #[test]
    fn pitch_bend_updates_active_live_synth_after_note_on() {
        let note = NoteNumber::new(60).unwrap();

        let (mut bent_renderer, mut bent_runtime) = test_runtime();
        bent_runtime.set_live_synth_source(BuiltInSynthSource::Triangle);
        bent_runtime
            .handle_input(InputEvent::Note(NoteInput::On {
                note,
                velocity: Velocity::new(100).unwrap(),
            }))
            .unwrap();
        let mut bent_warmup = vec![Frame::ZERO; 32];
        bent_renderer.render(&mut bent_warmup);
        bent_runtime
            .handle_input(InputEvent::Control(ControlInput::new(
                ControlKey::PitchBend,
                ControlValue::Bipolar(crate::domain::control::SignedUnitValue::new(1.0).unwrap()),
                InputSourceId::from("keyboard"),
                None,
            )))
            .unwrap();
        let mut bent_block = vec![Frame::ZERO; 32];
        bent_renderer.render(&mut bent_block);

        let (mut plain_renderer, mut plain_runtime) = test_runtime();
        plain_runtime.set_live_synth_source(BuiltInSynthSource::Triangle);
        plain_runtime
            .handle_input(InputEvent::Note(NoteInput::On {
                note,
                velocity: Velocity::new(100).unwrap(),
            }))
            .unwrap();
        let mut plain_warmup = vec![Frame::ZERO; 32];
        plain_renderer.render(&mut plain_warmup);
        let mut plain_block = vec![Frame::ZERO; 32];
        plain_renderer.render(&mut plain_block);

        assert_ne!(bent_block, plain_block);
    }

    #[test]
    fn expression_updates_active_live_synth_gain() {
        let (mut renderer, mut runtime) = test_runtime();
        runtime.set_live_synth_source(BuiltInSynthSource::Saw);

        runtime
            .handle_input(InputEvent::Note(NoteInput::On {
                note: NoteNumber::new(60).unwrap(),
                velocity: Velocity::new(100).unwrap(),
            }))
            .unwrap();

        let mut baseline = vec![Frame::ZERO; 64];
        renderer.render(&mut baseline);

        runtime
            .handle_input(InputEvent::Control(ControlInput::new(
                ControlKey::Expression,
                ControlValue::Unipolar(UnitValue::new(0.2).unwrap()),
                InputSourceId::from("keyboard"),
                None,
            )))
            .unwrap();

        let mut quiet = vec![Frame::ZERO; 64];
        renderer.render(&mut quiet);

        assert!(block_energy(&quiet) < block_energy(&baseline));
    }
    fn clocked_runtime() -> (AudioRenderer, PlaybackRuntime) {
        let (audio, renderer) =
            AudioRenderer::split(AudioRendererSettings::new(1_000, 128)).unwrap();
        let runtime = PlaybackRuntime::new(
            PlaybackSettings {
                cps: Time::ONE,
                look_ahead: Time::ONE,
                step: Time::new(1, 16),
            },
            audio,
        );
        runtime.load_sample(
            "old",
            SampleBuffer::new(1000, vec![Frame::new(0.25, -0.5); 2000]),
        );
        runtime.load_sample(
            "new",
            SampleBuffer::new(1000, vec![Frame::new(-0.25, 0.5); 2000]),
        );
        (renderer, runtime)
    }

    #[test]
    fn lookahead_audio_is_identical_during_a_graphics_stall() {
        fn render(tick_between_blocks: bool) -> Vec<Frame> {
            let (mut renderer, mut runtime) = clocked_runtime();
            let score = Score::from(
                Voice::new(
                    Time::ONE,
                    (0..4)
                        .map(|index| {
                            Tile::spanning(
                                Time::new(index, 4),
                                Time::new(index * 4 + 1, 16),
                                Intent::sample("old"),
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap(),
            );
            runtime.play_prepared_score(prepared(score)).unwrap();
            runtime.tick().unwrap();
            let mut output = vec![Frame::ZERO; 900];
            for block in output.chunks_mut(73) {
                if tick_between_blocks {
                    runtime.set_cps(Time::ONE).unwrap();
                    runtime.resume().unwrap();
                    runtime.tick().unwrap();
                }
                renderer.render(block);
            }
            assert_eq!(runtime.status().cycle_position, Time::new(9, 10));
            output
        }
        let without_graphics = render(false);
        assert!(
            without_graphics[750..800]
                .iter()
                .any(|frame| frame.left > 0.0)
        );
        assert_eq!(without_graphics, render(true));
    }

    #[test]
    fn replacement_becomes_audible_at_exact_next_cycle() {
        let (mut renderer, mut runtime) = clocked_runtime();
        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("old"))))
            .unwrap();
        runtime.tick().unwrap();
        renderer.render(&mut [Frame::ZERO; 200]);
        runtime
            .replace_prepared_score(prepared(Score::from(sample_voice("new"))))
            .unwrap();
        assert_eq!(runtime.pending_revision_cycle(), Some(Time::ONE));
        runtime.tick().unwrap();
        let mut output = vec![Frame::ZERO; 1100];
        renderer.render(&mut output);
        assert!(
            output[799].left > 0.0,
            "old score must still sound at frame 999"
        );
        assert!(
            output[800].left < 0.0,
            "new score starts exactly at frame 1000: {:?}",
            &output[795..805]
        );
        assert!(output[1000].left < 0.0);
        runtime.tick().unwrap();
        assert_eq!(runtime.pending_revision_cycle(), None);
    }

    #[test]
    fn pause_preserves_device_position_and_stop_cancels_all_future_audio() {
        let (mut renderer, mut runtime) = clocked_runtime();
        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("old"))))
            .unwrap();
        runtime.tick().unwrap();
        renderer.render(&mut [Frame::ZERO; 250]);
        runtime.pause().unwrap();
        assert_eq!(runtime.status().cycle_position, Time::new(1, 4));
        let mut paused = [Frame::ZERO; 100];
        renderer.render(&mut paused);
        assert!(paused[8..].iter().all(|frame| *frame == Frame::ZERO));
        assert_eq!(runtime.status().cycle_position, Time::new(1, 4));
        runtime.resume().unwrap();
        runtime.tick().unwrap();
        let mut resumed = [Frame::ZERO; 20];
        renderer.render(&mut resumed);
        assert!(resumed.iter().any(|frame| frame.left > 0.0));
        runtime.stop().unwrap();
        let mut stopped = [Frame::ZERO; 1500];
        for block in stopped.chunks_mut(2) {
            runtime.stop().unwrap();
            renderer.render(block);
        }
        assert_eq!(runtime.status().cycle_position, Time::ZERO);
        assert!(stopped[8..].iter().all(|frame| *frame == Frame::ZERO));
    }
    #[test]
    fn seek_reconstructs_sample_position_instead_of_retriggering_the_start() {
        let (mut renderer, mut runtime) = clocked_runtime();
        runtime.load_sample(
            "ramp",
            SampleBuffer::new(
                1000,
                (0..1000)
                    .map(|index| Frame::from_mono(index as f32 / 1000.0))
                    .collect::<Vec<_>>(),
            ),
        );
        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("ramp"))))
            .unwrap();
        runtime.seek(Time::new(1, 4)).unwrap();
        runtime.tick().unwrap();
        let mut output = [Frame::ZERO; 16];
        renderer.render(&mut output);
        assert!(
            (output[8].left - 0.258).abs() < 0.001,
            "seek must reconstruct elapsed source frames: {:?}",
            output[8]
        );
        assert_eq!(runtime.status().cycle_position, Time::new(133, 500));
    }
    #[test]
    fn stopping_before_pending_revision_still_keeps_the_latest_score_for_resume() {
        let (mut renderer, mut runtime) = clocked_runtime();
        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("old"))))
            .unwrap();
        runtime.tick().unwrap();
        renderer.render(&mut [Frame::ZERO; 200]);
        runtime
            .replace_prepared_score(prepared(Score::from(sample_voice("new"))))
            .unwrap();
        runtime.stop().unwrap();
        renderer.render(&mut [Frame::ZERO; 16]);
        runtime.resume().unwrap();
        runtime.tick().unwrap();
        let mut output = [Frame::ZERO; 20];
        renderer.render(&mut output);
        assert!(
            output.iter().any(|frame| frame.left < 0.0),
            "the latest valid authored score survives Stop"
        );
        assert_eq!(runtime.pending_revision_cycle(), None);
    }

    #[test]
    fn seeking_stopped_transport_preserves_requested_position_for_resume() {
        let (_, mut runtime) = clocked_runtime();
        runtime.seek(Time::new(3, 4)).unwrap();
        assert_eq!(runtime.status().state, PlaybackState::Paused);
        assert_eq!(runtime.status().cycle_position, Time::new(3, 4));
        runtime.resume().unwrap();
        assert_eq!(runtime.status().cycle_position, Time::new(3, 4));
    }
    #[test]
    fn tick_after_a_long_stall_reconstructs_current_audio_without_a_past_event_burst() {
        let (mut renderer, mut runtime) = clocked_runtime();
        runtime
            .play_prepared_score(prepared(Score::from(sample_voice("old"))))
            .unwrap();
        runtime.tick().unwrap();
        renderer.render(&mut vec![Frame::ZERO; 3500]);
        runtime.tick().unwrap();
        let mut output = [Frame::ZERO; 20];
        renderer.render(&mut output);
        assert!(output[8].left > 0.0);
        assert_eq!(
            runtime.audio.late_command_count(),
            0,
            "missed events must not be replayed as simultaneous late commands"
        );
        assert_eq!(runtime.status().cycle_position, Time::new(88, 25));
    }
}
