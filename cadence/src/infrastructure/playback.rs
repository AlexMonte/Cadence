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
        renderer_core::RendererCore,
        sample::SampleTrigger,
        scheduler::events::event_sink::Sink,
        synth::SynthTrigger,
    },
    domain::{
        control::{ControlKey, ControlMap, ControlModelError, ControlValue},
        input::{ControlInput, HeldNotes, InputEvent, NoteInput, SampleNoteMap},
        intent::BuiltInSynthSource,
        mosaic::Mosaic,
        prelude::Time,
        score::Score,
        voice::Voice,
    },
};

pub use crate::adapter::audio::{Frame, SampleBuffer};
pub use crate::adapter::sample_bank::{
    ChokeGroup, SampleBank, SampleLoadOptions, SamplePitchRange,
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
    /// Error while resolving the current control model through projection/scheduling.
    #[error("invalid control model: {0}")]
    ControlModel(#[from] ControlModelError),
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
}

impl PlaybackRuntime {
    /// Creates a stopped playback runtime.
    #[must_use]
    pub fn new(settings: PlaybackSettings, audio: AudioControl) -> Self {
        let bank = RuntimeSampleBank::new();
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
        }
    }

    /// Returns a handle to the runtime sample bank.
    #[must_use]
    pub fn sample_bank(&self) -> SampleBank {
        self.bank.clone()
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
        self.engine.tick()?;
        let _ = self.resolver.tick();
        Ok(())
    }

    /// Returns a host-side hint for how often `tick()` should run.
    #[must_use]
    pub fn tick_interval_hint(&self) -> Duration {
        self.engine.clock.cycles_to_duration(self.settings.step)
    }

    /// Convenience helper for source-leaf playback.
    ///
    /// Hosts that already own score-shaped meaning should prefer `play_score`.
    pub fn play_voice(&mut self, voice: Voice) -> Result<(), PlaybackError> {
        self.play_renderer(RendererCore::voice(voice))
    }

    /// Convenience helper for multiple source leaves.
    ///
    /// Hosts that already own score-shaped meaning should prefer `play_score`.
    pub fn play_voices(&mut self, voices: Vec<Voice>) -> Result<(), PlaybackError> {
        self.play_renderer(RendererCore::voices(voices))
    }

    /// Preferred entry point for typed host lowering.
    ///
    /// Cadence should lower typed source meaning into `Score` before it enters
    /// `cadence`.
    pub fn play_score(&mut self, score: Score) -> Result<(), PlaybackError> {
        self.play_renderer(RendererCore::new(score))
    }

    /// Convenience helper for source-leaf replacement.
    ///
    /// Hosts that already own score-shaped meaning should prefer
    /// `replace_score`.
    pub fn replace_voice(&mut self, voice: Voice) -> Result<(), PlaybackError> {
        self.replace_renderer(RendererCore::voice(voice))
    }

    /// Convenience helper for replacing multiple source leaves.
    ///
    /// Hosts that already own score-shaped meaning should prefer
    /// `replace_score`.
    pub fn replace_voices(&mut self, voices: Vec<Voice>) -> Result<(), PlaybackError> {
        self.replace_renderer(RendererCore::voices(voices))
    }

    /// Preferred replacement entry point for typed host lowering.
    pub fn replace_score(&mut self, score: Score) -> Result<(), PlaybackError> {
        self.replace_renderer(RendererCore::new(score))
    }

    /// Plays already-projected transport-time output.
    ///
    /// Prefer score-first playback when the host still has repeating authoring
    /// structure. This method remains as a lower-level compatibility path.
    pub fn play_projected_mosaic(&mut self, mosaic: Mosaic) -> Result<(), PlaybackError> {
        self.play_renderer(RendererCore::mosaic(mosaic))
    }

    /// Replaces the current renderer with already-projected transport-time
    /// output.
    ///
    /// Prefer score-first replacement when the host still has repeating
    /// authoring structure. This method remains as a lower-level compatibility
    /// path.
    pub fn replace_projected_mosaic(&mut self, mosaic: Mosaic) -> Result<(), PlaybackError> {
        self.replace_renderer(RendererCore::mosaic(mosaic))
    }

    /// Changes the transport rate in cycles per second.
    ///
    /// # Panics
    ///
    /// Panics if `cps <= 0`.
    pub fn set_cps(&mut self, cps: Time) -> Result<(), PlaybackError> {
        assert!(cps > Time::ZERO, "cycles per second must be positive");
        self.settings.cps = cps;
        self.engine.set_cycles_per_second(cps);
        Ok(())
    }

    /// Pauses playback.
    pub fn pause(&mut self) -> Result<(), PlaybackError> {
        self.engine.pause();
        Ok(())
    }

    /// Resumes playback.
    pub fn resume(&mut self) -> Result<(), PlaybackError> {
        self.engine.resume();
        Ok(())
    }

    /// Stops transport and resets the observable cycle position to zero.
    pub fn stop(&mut self) -> Result<(), PlaybackError> {
        self.engine.stop();
        Ok(())
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
            cycle_position: self.engine.current_time(),
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
                self.audio.update_voice_controls(voice_id, delta)?;
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
        self.engine.stop();
        self.engine.replace_renderer(renderer);
        self.engine.set_cycles_per_second(self.settings.cps);
        self.engine.resume();
        Ok(())
    }

    fn replace_renderer(&mut self, renderer: RendererCore) -> Result<(), PlaybackError> {
        let was_stopped = matches!(self.engine.status(), EngineStatus::Stopped);
        self.engine.replace_renderer(renderer);
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
            moment::Moment,
            mosaic::Mosaic,
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

    fn sample_mosaic(sample: &str) -> Mosaic {
        Mosaic::new(vec![
            Moment::spanning(Time::ZERO, Time::ONE, Intent::sample(sample)).unwrap(),
        ])
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

    #[test]
    fn runtime_starts_stopped_with_no_active_score() {
        let (_renderer, runtime) = test_runtime();

        assert_eq!(runtime.status().state, PlaybackState::Stopped);
        assert_eq!(runtime.status().cycle_position, Time::ZERO);
        assert_eq!(runtime.engine.scheduler.renderer().score(), &Score::empty());
    }

    #[test]
    fn play_score_transitions_runtime_to_playing() {
        let (_renderer, mut runtime) = test_runtime();

        runtime.play_score(Score::from(sample_voice("bd"))).unwrap();

        assert_eq!(runtime.status().state, PlaybackState::Playing);
    }

    #[test]
    fn replace_score_preserves_transport_when_already_playing() {
        let (_renderer, mut runtime) = test_runtime();
        runtime.play_score(Score::from(sample_voice("bd"))).unwrap();
        runtime.tick().unwrap();
        let before = runtime.status().cycle_position;

        runtime
            .replace_score(Score::from(sample_voice("vox")))
            .unwrap();

        assert_eq!(runtime.status().state, PlaybackState::Playing);
        assert!(runtime.status().cycle_position >= before);
    }

    #[test]
    fn pause_and_resume_update_runtime_state() {
        let (_renderer, mut runtime) = test_runtime();
        runtime.play_score(Score::from(sample_voice("bd"))).unwrap();

        runtime.pause().unwrap();
        assert_eq!(runtime.status().state, PlaybackState::Paused);

        runtime.resume().unwrap();
        assert_eq!(runtime.status().state, PlaybackState::Playing);
    }

    #[test]
    fn stop_resets_cycle_position_to_zero() {
        let (_renderer, mut runtime) = test_runtime();
        runtime.play_score(Score::from(sample_voice("bd"))).unwrap();
        runtime.tick().unwrap();

        runtime.stop().unwrap();

        let status = runtime.status();
        assert_eq!(status.state, PlaybackState::Stopped);
        assert_eq!(status.cycle_position, Time::ZERO);
    }

    #[test]
    fn playback_keeps_voice_first_flow_and_projected_mosaic_compatibility() {
        let (_renderer, mut runtime) = test_runtime();

        runtime.play_voice(sample_voice("bd")).unwrap();
        assert_eq!(runtime.status().state, PlaybackState::Playing);

        runtime.stop().unwrap();
        runtime.play_projected_mosaic(sample_mosaic("bd")).unwrap();

        assert_eq!(runtime.status().state, PlaybackState::Playing);
    }

    #[test]
    fn playback_accepts_warped_voices_without_a_new_runtime_path() {
        let (_renderer, mut runtime) = test_runtime();
        let warped = ops::fast_by(&sample_voice("bd"), Time::new(2, 1));

        runtime.play_voice(warped).unwrap();

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
    fn play_score_handles_scheduled_synth_events() {
        let (mut renderer, mut runtime) = test_runtime();

        runtime
            .play_score(synth_score(BuiltInSynthSource::Sine, 69.0))
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
}
