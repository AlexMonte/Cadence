//! Sample-trigger lowering from typed musical intent.

use std::{
    sync::{
        Arc, RwLock,
        mpsc::{self, Receiver, SendError, Sender, TryRecvError},
    },
    time::Duration,
};

use crate::{
    application::{
        audio::{
            AudioRuntimeControlState, AudioSourcePlan, AudioVoicePlan, SignalBinding,
            VoiceInstanceId, VoiceSpatial,
        },
        performer::Performer,
        scheduler::events::scheduled_intent::ScheduledIntent,
    },
    domain::{
        control::{
            CompressorSettings, ControlMap, DelaySettings, ReverbSettings, Symbol, UnitValue,
        },
        input::NoteNumber,
        intent::SampleIntent,
    },
};

fn assert_finite(label: &str, value: f64) {
    assert!(value.is_finite(), "{label} must be finite");
}

fn assert_non_negative(label: &str, value: f64) {
    assert!(
        value.is_finite() && value >= 0.0,
        "{label} must be finite and >= 0.0"
    );
}

fn assert_positive(label: &str, value: f64) {
    assert!(
        value.is_finite() && value > 0.0,
        "{label} must be finite and > 0.0"
    );
}

fn assert_unit_region(label: &str, start: f64, end: f64) {
    assert!(
        start.is_finite() && end.is_finite() && start >= 0.0 && start < end && end <= 1.0,
        "{label} must satisfy 0.0 <= start < end <= 1.0 with finite bounds"
    );
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Linear gain ramp used by sample playback.
pub struct SampleGainRamp {
    from: f64,
    to: f64,
}

impl SampleGainRamp {
    /// Creates a gain ramp.
    ///
    /// # Panics
    ///
    /// Panics if either endpoint is negative or not finite.
    #[must_use]
    pub fn new(from: f64, to: f64) -> Self {
        assert_non_negative("gain ramp start", from);
        assert_non_negative("gain ramp end", to);
        Self { from, to }
    }

    /// Returns the start gain.
    #[must_use]
    pub fn from(self) -> f64 {
        self.from
    }

    /// Returns the end gain.
    #[must_use]
    pub fn to(self) -> f64 {
        self.to
    }

    /// Evaluates the ramp at normalized `progress`.
    #[must_use]
    pub fn value_at(self, progress: f64) -> f64 {
        assert_finite("gain ramp progress", progress);
        let progress = progress.clamp(0.0, 1.0);
        self.from + (self.to - self.from) * progress
    }

    /// Slices the ramp so it matches `subspan` inside `whole`.
    ///
    /// # Panics
    ///
    /// Panics if `subspan` is not contained inside `whole`.
    #[must_use]
    pub fn slice_for<Space>(
        self,
        whole: crate::domain::span::Span<Space>,
        subspan: crate::domain::span::Span<Space>,
    ) -> Self
    where
        Space: PartialEq + Eq,
    {
        assert!(
            whole.contains(&subspan),
            "subspan must be contained by whole"
        );
        let duration = whole.end() - whole.start();
        let start_progress = ((subspan.start() - whole.start()) / duration).value();
        let end_progress = ((subspan.end() - whole.start()) / duration).value();
        Self::new(self.value_at(start_progress), self.value_at(end_progress))
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Envelope parameters carried alongside a sample trigger.
pub struct SampleEnvelope {
    attack: Duration,
    decay: Duration,
    sustain_level: UnitValue,
    release: Duration,
    gate_duration: Option<Duration>,
    elapsed: Duration,
}

impl SampleEnvelope {
    /// Creates a sample envelope.
    #[must_use]
    pub fn new(
        attack: Duration,
        decay: Duration,
        sustain_level: UnitValue,
        release: Duration,
        gate_duration: Option<Duration>,
        elapsed: Duration,
    ) -> Self {
        Self {
            attack,
            decay,
            sustain_level,
            release,
            gate_duration,
            elapsed,
        }
    }

    /// Returns the attack duration.
    #[must_use]
    pub fn attack(&self) -> Duration {
        self.attack
    }

    /// Returns the decay duration.
    #[must_use]
    pub fn decay(&self) -> Duration {
        self.decay
    }

    /// Returns the sustain level.
    #[must_use]
    pub fn sustain_level(&self) -> UnitValue {
        self.sustain_level
    }

    /// Returns the release duration.
    #[must_use]
    pub fn release(&self) -> Duration {
        self.release
    }

    /// Returns the gate duration, if one was computed.
    #[must_use]
    pub fn gate_duration(&self) -> Option<Duration> {
        self.gate_duration
    }

    /// Returns how much of the envelope has already elapsed.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

impl Eq for SampleEnvelope {}

/// Adapter-local command for sample playback.
///
/// The core runtime schedules typed `Intent` values. This type is the first
/// sample adapter seam: it translates musical sample intent into concrete
/// playback settings without owning any audio backend.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleTrigger {
    /// Concrete voice instance this trigger belongs to.
    pub voice_id: VoiceInstanceId,
    /// Initial runtime control state for the active voice.
    pub runtime_controls: AudioRuntimeControlState,
    /// Logical sample identifier to resolve in the sample bank.
    pub sample: String,
    /// Optional named sample-bank override.
    pub sample_bank: Option<Symbol>,
    /// Optional sample-variant selection.
    pub sample_variant: Option<usize>,
    /// Optional pitch value, usually in semitone/MIDI-note space.
    pub pitch: Option<f64>,
    /// Final velocity after control merging.
    pub velocity: UnitValue,
    /// Final gain multiplier.
    pub gain: f64,
    /// Optional gain ramp.
    pub gain_ramp: Option<SampleGainRamp>,
    /// Spatial placement and cycle clock for per-frame position.
    pub spatial: VoiceSpatial,
    /// Final playback rate multiplier.
    pub playback_rate: f64,
    /// Unit-range playback start position.
    pub playback_start: f64,
    /// Unit-range playback end position.
    pub playback_end: f64,
    /// Whether playback runs in reverse.
    pub reverse: bool,
    /// Envelope settings for this playback.
    pub envelope: SampleEnvelope,
    /// Optional low-pass cutoff.
    pub low_pass_cutoff_hz: Option<f64>,
    /// Low-pass resonance amount.
    pub low_pass_resonance: UnitValue,
    /// Optional high-pass cutoff.
    pub high_pass_cutoff_hz: Option<f64>,
    /// High-pass resonance amount.
    pub high_pass_resonance: UnitValue,
    /// Final post-effect gain.
    pub post_gain: f64,
    /// Optional reverb-send settings.
    pub reverb: Option<ReverbSettings>,
    /// Optional delay-send settings.
    pub delay: Option<DelaySettings>,
    /// Optional compressor settings.
    pub compressor: Option<CompressorSettings>,
    /// Live note to release later, if this trigger came from live input.
    pub live_note: Option<NoteNumber>,
    /// Remaining wall-clock time the note should be held for.
    pub play_for: Duration,
    /// Continuous signal bindings evaluated per output frame.
    pub modulations: Vec<SignalBinding>,
}

impl SampleTrigger {
    /// Default gain multiplier.
    pub const DEFAULT_GAIN: f64 = 1.0;
    /// Default playback rate.
    pub const DEFAULT_PLAYBACK_RATE: f64 = 1.0;
    /// Default playback start.
    pub const DEFAULT_PLAYBACK_START: f64 = 0.0;
    /// Default playback end.
    pub const DEFAULT_PLAYBACK_END: f64 = 1.0;
    /// Default reverse flag.
    pub const DEFAULT_REVERSE: bool = false;
    /// Default post-effect gain.
    pub const DEFAULT_POST_GAIN: f64 = 1.0;

    /// Creates a validated sample trigger.
    ///
    /// # Panics
    ///
    /// Panics when any scalar precondition is violated.
    #[must_use]
    pub fn new(settings: SampleTriggerSettings) -> Self {
        assert_non_negative("sample trigger gain", settings.gain);
        assert_positive("sample trigger playback_rate", settings.playback_rate);
        assert_unit_region(
            "sample trigger region",
            settings.playback_start,
            settings.playback_end,
        );
        if let Some(cutoff) = settings.low_pass_cutoff_hz {
            assert_positive("sample trigger low-pass cutoff", cutoff);
        }
        if let Some(cutoff) = settings.high_pass_cutoff_hz {
            assert_positive("sample trigger high-pass cutoff", cutoff);
        }
        assert_non_negative("sample trigger post-gain", settings.post_gain);
        if let Some(pitch) = settings.pitch {
            assert_finite("sample trigger pitch", pitch);
        }

        Self {
            voice_id: settings.voice_id,
            runtime_controls: settings.runtime_controls,
            sample: settings.sample,
            sample_bank: settings.sample_bank,
            sample_variant: settings.sample_variant,
            pitch: settings.pitch,
            velocity: settings.velocity,
            gain: settings.gain,
            gain_ramp: settings.gain_ramp,
            spatial: settings.spatial,
            playback_rate: settings.playback_rate,
            playback_start: settings.playback_start,
            playback_end: settings.playback_end,
            reverse: settings.reverse,
            envelope: settings.envelope,
            low_pass_cutoff_hz: settings.low_pass_cutoff_hz,
            low_pass_resonance: settings.low_pass_resonance,
            high_pass_cutoff_hz: settings.high_pass_cutoff_hz,
            high_pass_resonance: settings.high_pass_resonance,
            post_gain: settings.post_gain,
            reverb: settings.reverb,
            delay: settings.delay,
            compressor: settings.compressor,
            live_note: settings.live_note,
            play_for: settings.play_for,
            modulations: settings.modulations,
        }
    }

    /// Returns a builder for constructing a sample trigger incrementally.
    #[must_use]
    pub fn builder() -> SampleTriggerBuilder {
        SampleTriggerBuilder::new()
    }

    /// Converts a scheduled intent into a sample trigger with no ambient
    /// controls.
    #[must_use]
    pub fn from_scheduled_intent(event: &ScheduledIntent) -> Option<Self> {
        Self::from_scheduled_intent_with_ambient(event, &ControlMap::new())
    }

    /// Converts a scheduled intent into a sample trigger using ambient
    /// controls.
    #[must_use]
    pub fn from_scheduled_intent_with_ambient(
        event: &ScheduledIntent,
        ambient_controls: &ControlMap,
    ) -> Option<Self> {
        AudioVoicePlan::from_scheduled_intent_with_ambient(event, ambient_controls)
            .and_then(|plan| Self::from_voice_plan(&plan))
    }

    /// Converts a live sample intent plus controls into a trigger.
    #[must_use]
    pub fn from_live_sample(
        sample: &SampleIntent,
        controls: &ControlMap,
        velocity: Option<UnitValue>,
        live_note: Option<NoteNumber>,
        play_for: Duration,
    ) -> Option<Self> {
        AudioVoicePlan::from_live_sample(
            VoiceInstanceId::next_live(),
            sample,
            controls,
            velocity,
            live_note,
            play_for,
        )
        .and_then(|plan| Self::from_voice_plan(&plan))
    }

    /// Converts one shared audio voice plan into a sample trigger.
    #[must_use]
    pub fn from_voice_plan(plan: &AudioVoicePlan) -> Option<Self> {
        let AudioSourcePlan::Sample(source) = &plan.source else {
            return None;
        };

        let mut builder = Self::builder()
            .voice_id(plan.voice_id)
            .runtime_controls(plan.runtime_controls)
            .sample(source.sample.clone())
            .velocity(plan.mix.velocity)
            .gain(plan.mix.gain)
            .spatial(plan.spatial)
            .playback_rate(source.playback_rate)
            .playback_start(source.playback_start)
            .playback_end(source.playback_end)
            .reverse(source.reverse)
            .envelope(SampleEnvelope::new(
                plan.envelope.attack,
                plan.envelope.decay,
                plan.envelope.sustain_level,
                plan.envelope.release,
                plan.lifecycle.gate_duration,
                plan.lifecycle.elapsed,
            ))
            .low_pass_resonance(plan.filters.low_pass_resonance)
            .high_pass_resonance(plan.filters.high_pass_resonance)
            .post_gain(plan.mix.post_gain)
            .play_for(plan.lifecycle.play_for);

        if let Some(sample_bank) = &source.sample_bank {
            builder = builder.sample_bank(sample_bank.clone());
        }
        if let Some(sample_variant) = source.sample_variant {
            builder = builder.sample_variant(sample_variant);
        }
        if let Some(pitch) = source.pitch {
            builder = builder.pitch(pitch);
        }
        if let Some(gain_ramp) = plan.mix.gain_ramp {
            builder = builder.gain_ramp(gain_ramp);
        }
        if let Some(cutoff) = plan.filters.low_pass_cutoff_hz {
            builder = builder.low_pass_cutoff_hz(cutoff);
        }
        if let Some(cutoff) = plan.filters.high_pass_cutoff_hz {
            builder = builder.high_pass_cutoff_hz(cutoff);
        }
        if let Some(reverb) = &plan.sends.reverb {
            builder = builder.reverb(reverb.clone());
        }
        if let Some(delay) = &plan.sends.delay {
            builder = builder.delay(delay.clone());
        }
        if let Some(compressor) = &plan.dynamics.compressor {
            builder = builder.compressor(compressor.clone());
        }
        if let Some(note) = plan.live_note {
            builder = builder.live_note(note);
        }
        builder = builder.modulations(plan.modulations.clone());

        builder.build()
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Field-based settings struct accepted by [`SampleTrigger::new`].
pub struct SampleTriggerSettings {
    /// Concrete voice instance this trigger belongs to.
    pub voice_id: VoiceInstanceId,
    /// Initial runtime control state for the active voice.
    pub runtime_controls: AudioRuntimeControlState,
    /// Logical sample identifier.
    pub sample: String,
    /// Optional named sample-bank override.
    pub sample_bank: Option<Symbol>,
    /// Optional sample-variant selection.
    pub sample_variant: Option<usize>,
    /// Optional pitch value.
    pub pitch: Option<f64>,
    /// Final velocity.
    pub velocity: UnitValue,
    /// Final gain multiplier.
    pub gain: f64,
    /// Optional gain ramp.
    pub gain_ramp: Option<SampleGainRamp>,
    /// Spatial placement and cycle clock for per-frame position.
    pub spatial: VoiceSpatial,
    /// Final playback rate.
    pub playback_rate: f64,
    /// Unit-range playback start.
    pub playback_start: f64,
    /// Unit-range playback end.
    pub playback_end: f64,
    /// Reverse playback flag.
    pub reverse: bool,
    /// Envelope settings.
    pub envelope: SampleEnvelope,
    /// Optional low-pass cutoff.
    pub low_pass_cutoff_hz: Option<f64>,
    /// Low-pass resonance amount.
    pub low_pass_resonance: UnitValue,
    /// Optional high-pass cutoff.
    pub high_pass_cutoff_hz: Option<f64>,
    /// High-pass resonance amount.
    pub high_pass_resonance: UnitValue,
    /// Final post-effect gain.
    pub post_gain: f64,
    /// Optional reverb-send settings.
    pub reverb: Option<ReverbSettings>,
    /// Optional delay-send settings.
    pub delay: Option<DelaySettings>,
    /// Optional compressor settings.
    pub compressor: Option<CompressorSettings>,
    /// Live note to release later.
    pub live_note: Option<NoteNumber>,
    /// Remaining wall-clock playback duration.
    pub play_for: Duration,
    /// Continuous signal bindings evaluated per output frame.
    pub modulations: Vec<SignalBinding>,
}

#[derive(Debug, Clone, Default)]
/// Builder for [`SampleTrigger`].
pub struct SampleTriggerBuilder {
    voice_id: Option<VoiceInstanceId>,
    runtime_controls: Option<AudioRuntimeControlState>,
    sample: Option<String>,
    sample_bank: Option<Symbol>,
    sample_variant: Option<usize>,
    pitch: Option<f64>,
    velocity: Option<UnitValue>,
    gain: Option<f64>,
    gain_ramp: Option<SampleGainRamp>,
    spatial: Option<VoiceSpatial>,
    playback_rate: Option<f64>,
    playback_start: Option<f64>,
    playback_end: Option<f64>,
    reverse: Option<bool>,
    envelope: Option<SampleEnvelope>,
    low_pass_cutoff_hz: Option<f64>,
    low_pass_resonance: Option<UnitValue>,
    high_pass_cutoff_hz: Option<f64>,
    high_pass_resonance: Option<UnitValue>,
    post_gain: Option<f64>,
    reverb: Option<ReverbSettings>,
    delay: Option<DelaySettings>,
    compressor: Option<CompressorSettings>,
    live_note: Option<NoteNumber>,
    play_for: Option<Duration>,
    modulations: Vec<SignalBinding>,
}

impl SampleTriggerBuilder {
    /// Creates an empty builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the concrete voice instance identifier.
    #[must_use]
    pub fn voice_id(mut self, voice_id: VoiceInstanceId) -> Self {
        self.voice_id = Some(voice_id);
        self
    }

    /// Sets the initial runtime control state.
    #[must_use]
    pub fn runtime_controls(mut self, runtime_controls: AudioRuntimeControlState) -> Self {
        self.runtime_controls = Some(runtime_controls);
        self
    }

    /// Sets the logical sample identifier.
    #[must_use]
    pub fn sample(mut self, sample: impl Into<String>) -> Self {
        self.sample = Some(sample.into());
        self
    }

    /// Sets the optional named sample-bank override.
    #[must_use]
    pub fn sample_bank(mut self, sample_bank: impl Into<Symbol>) -> Self {
        self.sample_bank = Some(sample_bank.into());
        self
    }

    /// Sets the optional sample-variant selection.
    #[must_use]
    pub fn sample_variant(mut self, sample_variant: usize) -> Self {
        self.sample_variant = Some(sample_variant);
        self
    }

    /// Sets pitch.
    ///
    /// # Panics
    ///
    /// Panics if `pitch` is not finite.
    #[must_use]
    pub fn pitch(mut self, pitch: f64) -> Self {
        assert_finite("sample trigger pitch", pitch);
        self.pitch = Some(pitch);
        self
    }

    /// Sets velocity.
    #[must_use]
    pub fn velocity(mut self, velocity: UnitValue) -> Self {
        self.velocity = Some(velocity);
        self
    }

    /// Sets gain.
    ///
    /// # Panics
    ///
    /// Panics if `gain` is negative or not finite.
    #[must_use]
    pub fn gain(mut self, gain: f64) -> Self {
        assert_non_negative("sample trigger gain", gain);
        self.gain = Some(gain);
        self
    }

    /// Sets an explicit gain ramp.
    #[must_use]
    pub fn gain_ramp(mut self, gain_ramp: SampleGainRamp) -> Self {
        self.gain_ramp = Some(gain_ramp);
        self
    }

    /// Sets the spatial placement and cycle clock.
    #[must_use]
    pub fn spatial(mut self, spatial: VoiceSpatial) -> Self {
        self.spatial = Some(spatial);
        self
    }

    /// Sets playback rate.
    ///
    /// # Panics
    ///
    /// Panics if `playback_rate <= 0` or not finite.
    #[must_use]
    pub fn playback_rate(mut self, playback_rate: f64) -> Self {
        assert_positive("sample trigger playback_rate", playback_rate);
        self.playback_rate = Some(playback_rate);
        self
    }

    /// Sets the unit-range playback start.
    ///
    /// Validation happens when the trigger is built.
    #[must_use]
    pub fn playback_start(mut self, playback_start: f64) -> Self {
        assert_finite("sample trigger playback_start", playback_start);
        self.playback_start = Some(playback_start);
        self
    }

    /// Sets the unit-range playback end.
    ///
    /// Validation happens when the trigger is built.
    #[must_use]
    pub fn playback_end(mut self, playback_end: f64) -> Self {
        assert_finite("sample trigger playback_end", playback_end);
        self.playback_end = Some(playback_end);
        self
    }

    /// Sets reverse playback.
    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = Some(reverse);
        self
    }

    /// Sets the envelope.
    #[must_use]
    pub fn envelope(mut self, envelope: SampleEnvelope) -> Self {
        self.envelope = Some(envelope);
        self
    }

    /// Sets the low-pass cutoff.
    ///
    /// # Panics
    ///
    /// Panics if `cutoff <= 0` or not finite.
    #[must_use]
    pub fn low_pass_cutoff_hz(mut self, cutoff: f64) -> Self {
        assert_positive("sample trigger low-pass cutoff", cutoff);
        self.low_pass_cutoff_hz = Some(cutoff);
        self
    }

    /// Sets the low-pass resonance.
    #[must_use]
    pub fn low_pass_resonance(mut self, resonance: UnitValue) -> Self {
        self.low_pass_resonance = Some(resonance);
        self
    }

    /// Sets the high-pass cutoff.
    ///
    /// # Panics
    ///
    /// Panics if `cutoff <= 0` or not finite.
    #[must_use]
    pub fn high_pass_cutoff_hz(mut self, cutoff: f64) -> Self {
        assert_positive("sample trigger high-pass cutoff", cutoff);
        self.high_pass_cutoff_hz = Some(cutoff);
        self
    }

    /// Sets the high-pass resonance.
    #[must_use]
    pub fn high_pass_resonance(mut self, resonance: UnitValue) -> Self {
        self.high_pass_resonance = Some(resonance);
        self
    }

    /// Sets post-effect gain.
    ///
    /// # Panics
    ///
    /// Panics if `post_gain` is negative or not finite.
    #[must_use]
    pub fn post_gain(mut self, post_gain: f64) -> Self {
        assert_non_negative("sample trigger post-gain", post_gain);
        self.post_gain = Some(post_gain);
        self
    }

    /// Sets reverb-send settings.
    #[must_use]
    pub fn reverb(mut self, reverb: ReverbSettings) -> Self {
        self.reverb = Some(reverb);
        self
    }

    /// Sets delay-send settings.
    #[must_use]
    pub fn delay(mut self, delay: DelaySettings) -> Self {
        self.delay = Some(delay);
        self
    }

    /// Sets compressor settings.
    #[must_use]
    pub fn compressor(mut self, compressor: CompressorSettings) -> Self {
        self.compressor = Some(compressor);
        self
    }

    /// Associates a live note for later release handling.
    #[must_use]
    pub fn live_note(mut self, note: NoteNumber) -> Self {
        self.live_note = Some(note);
        self
    }

    /// Sets the remaining playback duration.
    #[must_use]
    pub fn play_for(mut self, play_for: Duration) -> Self {
        self.play_for = Some(play_for);
        self
    }

    /// Sets the continuous signal bindings.
    #[must_use]
    pub fn modulations(mut self, modulations: Vec<SignalBinding>) -> Self {
        self.modulations = modulations;
        self
    }

    /// Builds the trigger.
    ///
    /// Returns `None` when the required `sample` field was never provided.
    #[must_use]
    pub fn build(self) -> Option<SampleTrigger> {
        Some(SampleTrigger::new(SampleTriggerSettings {
            voice_id: self.voice_id.unwrap_or_else(VoiceInstanceId::next_live),
            runtime_controls: self.runtime_controls.unwrap_or_default(),
            sample: self.sample?,
            sample_bank: self.sample_bank,
            sample_variant: self.sample_variant,
            pitch: self.pitch,
            velocity: self
                .velocity
                .unwrap_or_else(|| UnitValue::new(1.0).unwrap()),
            gain: self.gain.unwrap_or(SampleTrigger::DEFAULT_GAIN),
            gain_ramp: self.gain_ramp,
            spatial: self.spatial.unwrap_or_default(),
            playback_rate: self
                .playback_rate
                .unwrap_or(SampleTrigger::DEFAULT_PLAYBACK_RATE),
            playback_start: self
                .playback_start
                .unwrap_or(SampleTrigger::DEFAULT_PLAYBACK_START),
            playback_end: self
                .playback_end
                .unwrap_or(SampleTrigger::DEFAULT_PLAYBACK_END),
            reverse: self.reverse.unwrap_or(SampleTrigger::DEFAULT_REVERSE),
            envelope: self.envelope.unwrap_or_else(|| {
                SampleEnvelope::new(
                    Duration::ZERO,
                    Duration::ZERO,
                    UnitValue::new(1.0).unwrap(),
                    Duration::ZERO,
                    None,
                    Duration::ZERO,
                )
            }),
            low_pass_cutoff_hz: self.low_pass_cutoff_hz,
            low_pass_resonance: self
                .low_pass_resonance
                .unwrap_or_else(|| UnitValue::new(0.0).unwrap()),
            high_pass_cutoff_hz: self.high_pass_cutoff_hz,
            high_pass_resonance: self
                .high_pass_resonance
                .unwrap_or_else(|| UnitValue::new(0.0).unwrap()),
            post_gain: self.post_gain.unwrap_or(SampleTrigger::DEFAULT_POST_GAIN),
            reverb: self.reverb,
            delay: self.delay,
            compressor: self.compressor,
            live_note: self.live_note,
            play_for: self.play_for.unwrap_or(Duration::ZERO),
            modulations: self.modulations,
        }))
    }
}

#[derive(Clone)]
/// Sender half of the sample-trigger queue.
pub struct SampleTriggerSender {
    sender: Sender<SampleTrigger>,
}

impl SampleTriggerSender {
    /// Sends one sample trigger.
    pub fn send(&self, trigger: SampleTrigger) -> Result<(), SendError<SampleTrigger>> {
        self.sender.send(trigger)
    }

    /// Creates a performer with no ambient controls.
    #[must_use]
    pub fn performer(&self) -> SampleTriggerOutputPerformer {
        SampleTriggerOutputPerformer::new(self.clone(), Arc::new(RwLock::new(ControlMap::new())))
    }

    /// Creates a performer that merges scheduled controls with shared ambient
    /// controls.
    #[must_use]
    pub fn performer_with_ambient(
        &self,
        ambient_controls: Arc<RwLock<ControlMap>>,
    ) -> SampleTriggerOutputPerformer {
        SampleTriggerOutputPerformer::new(self.clone(), ambient_controls)
    }
}

/// Receiver half of the sample-trigger queue.
pub struct SampleTriggerReceiver {
    receiver: Receiver<SampleTrigger>,
}

impl SampleTriggerReceiver {
    /// Attempts to receive one trigger without blocking.
    pub fn try_recv(&self) -> Result<SampleTrigger, TryRecvError> {
        self.receiver.try_recv()
    }

    /// Drains all currently queued triggers.
    #[must_use]
    pub fn drain(&self) -> Vec<SampleTrigger> {
        self.receiver.try_iter().collect()
    }
}

/// Creates a sample-trigger channel.
#[must_use]
pub fn sample_trigger_channel() -> (SampleTriggerSender, SampleTriggerReceiver) {
    let (sender, receiver) = mpsc::channel();

    (
        SampleTriggerSender { sender },
        SampleTriggerReceiver { receiver },
    )
}

/// Queue-backed sample performer for the future audio adapter boundary.
pub struct SampleTriggerOutputPerformer {
    trigger_sender: SampleTriggerSender,
    ambient_controls: Arc<RwLock<ControlMap>>,
}

impl SampleTriggerOutputPerformer {
    /// Creates an output performer backed by a trigger sender and ambient
    /// control store.
    #[must_use]
    pub fn new(
        trigger_sender: SampleTriggerSender,
        ambient_controls: Arc<RwLock<ControlMap>>,
    ) -> Self {
        Self {
            trigger_sender,
            ambient_controls,
        }
    }
}

impl Performer for SampleTriggerOutputPerformer {
    fn perform(&self, event: ScheduledIntent) {
        let ambient_controls = self
            .ambient_controls
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();

        if let Some(trigger) =
            SampleTrigger::from_scheduled_intent_with_ambient(&event, &ambient_controls)
        {
            let _ = self.trigger_sender.send(trigger);
        }
    }
}

/// Performer adapter that turns scheduled sample intents into sample triggers.
pub struct SampleTriggerPerformer<F> {
    on_trigger: F,
}

impl<F> SampleTriggerPerformer<F> {
    /// Creates a performer that forwards each produced trigger into
    /// `on_trigger`.
    #[must_use]
    pub fn new(on_trigger: F) -> Self {
        Self { on_trigger }
    }
}

impl<F> Performer for SampleTriggerPerformer<F>
where
    F: Fn(SampleTrigger),
{
    fn perform(&self, event: ScheduledIntent) {
        if let Some(trigger) = SampleTrigger::from_scheduled_intent(&event) {
            (self.on_trigger)(trigger);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, panic, rc::Rc, time::Instant};

    use crate::{
        application::{
            clock::{Clock, ClockTime},
            scheduler::events::{IDGenerator, scheduled_intent::ScheduledIntent},
        },
        domain::{
            control::{
                CompressorSettings, ControlKey, ControlMap, ControlValue, DelaySettings,
                ReverbSettings, SignedUnitValue, Symbol, UnitValue,
            },
            intent::Intent,
            moment::Moment,
            projection::ProjectedMoment,
            rational::Time,
            span::{Span, TransportSpan},
        },
    };

    fn scheduled_intent(value: Intent, play_for: Duration) -> ScheduledIntent {
        let projected = ProjectedMoment::new(
            Moment::new(Span::new(Time::ZERO, Time::ONE).unwrap(), value),
            TransportSpan::new(Time::ZERO, Time::ONE).unwrap(),
            ControlMap::new(),
        );

        ScheduledIntent::new(
            IDGenerator::new(1).next(),
            projected,
            ClockTime::ZERO,
            Instant::now(),
            play_for,
        )
    }

    fn scheduled_intent_with_visible(
        value: Intent,
        whole: ((i64, i64), (i64, i64)),
        visible: ((i64, i64), (i64, i64)),
        controls: ControlMap,
    ) -> ScheduledIntent {
        let clock = Clock::new(Time::new(1, 1), Instant::now());
        let projected = ProjectedMoment::new(
            Moment::new(
                Span::new(
                    Time::new((whole.0).0, (whole.0).1),
                    Time::new((whole.1).0, (whole.1).1),
                )
                .unwrap(),
                value,
            ),
            TransportSpan::new(
                Time::new((visible.0).0, (visible.0).1),
                Time::new((visible.1).0, (visible.1).1),
            )
            .unwrap(),
            controls,
        );

        ScheduledIntent::from_projected(IDGenerator::new(1).next(), projected, &clock)
    }

    #[test]
    fn gain_ramp_slices_a_visible_subspan() {
        let envelope = SampleGainRamp::new(1.0, 0.0);
        let whole: Span = Span::new(Time::ZERO, Time::ONE).unwrap();
        let visible: Span = Span::new(Time::new(1, 2), Time::ONE).unwrap();

        assert_eq!(
            envelope.slice_for(whole, visible),
            SampleGainRamp::new(0.5, 0.0)
        );
    }

    #[test]
    fn builder_requires_sample() {
        let trigger = SampleTrigger::builder()
            .gain(0.5)
            .play_for(Duration::from_millis(10))
            .build();

        assert_eq!(trigger, None);
    }

    #[test]
    fn builder_uses_defaults_for_optional_controls() {
        let trigger = SampleTrigger::builder()
            .sample("kick")
            .play_for(Duration::from_millis(10))
            .build()
            .unwrap();

        assert_eq!(trigger.sample, "kick");
        assert_eq!(trigger.gain, SampleTrigger::DEFAULT_GAIN);
        assert_eq!(trigger.gain_ramp, None);
        assert_eq!(trigger.spatial, VoiceSpatial::ORIGIN);
        assert_eq!(trigger.playback_rate, SampleTrigger::DEFAULT_PLAYBACK_RATE);
        assert_eq!(
            trigger.playback_start,
            SampleTrigger::DEFAULT_PLAYBACK_START
        );
        assert_eq!(trigger.playback_end, SampleTrigger::DEFAULT_PLAYBACK_END);
        assert_eq!(trigger.reverse, SampleTrigger::DEFAULT_REVERSE);
        assert_eq!(trigger.play_for, Duration::from_millis(10));
    }

    #[test]
    fn typed_sample_intent_becomes_sample_trigger() {
        let event = scheduled_intent(
            Intent::Sample(
                SampleIntent::new("vox")
                    .gain(0.4)
                    .rate(1.5)
                    .region(0.1, 0.9)
                    .reverse(true),
            ),
            Duration::from_millis(25),
        );

        let trigger = SampleTrigger::from_scheduled_intent(&event).unwrap();

        assert_eq!(trigger.sample, "vox");
        assert_eq!(trigger.gain, 0.4);
        assert_eq!(trigger.gain_ramp, None);
        assert_eq!(trigger.playback_rate, 1.5);
        assert_eq!(trigger.playback_start, 0.1);
        assert_eq!(trigger.playback_end, 0.9);
        assert!(trigger.reverse);
        assert_eq!(trigger.play_for, Duration::from_millis(25));
    }

    #[test]
    fn non_sample_typed_intent_is_not_a_sample_trigger() {
        let event = scheduled_intent(Intent::level("filter", 0.8), Duration::from_millis(25));

        assert_eq!(SampleTrigger::from_scheduled_intent(&event), None);
    }

    #[test]
    fn performer_emits_sample_triggers() {
        let triggers = Rc::new(RefCell::new(Vec::new()));
        let performer = SampleTriggerPerformer::new({
            let triggers = Rc::clone(&triggers);
            move |trigger| triggers.borrow_mut().push(trigger)
        });
        let event = scheduled_intent(Intent::sample("kick"), Duration::from_millis(25));

        performer.perform(event);

        let triggers = triggers.borrow();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].sample, "kick");
        assert_eq!(triggers[0].play_for, Duration::from_millis(25));
    }

    #[test]
    fn performer_ignores_non_sample_events() {
        let triggers = Rc::new(RefCell::new(Vec::new()));
        let performer = SampleTriggerPerformer::new({
            let triggers = Rc::clone(&triggers);
            move |trigger| triggers.borrow_mut().push(trigger)
        });
        let event = scheduled_intent(Intent::gate("mute", false), Duration::from_millis(25));

        performer.perform(event);

        assert!(triggers.borrow().is_empty());
    }

    #[test]
    fn output_performer_sends_triggers_into_receiver() {
        let (sender, receiver) = sample_trigger_channel();
        let performer = sender.performer();
        let event = scheduled_intent(Intent::sample("kick"), Duration::from_millis(25));

        performer.perform(event);

        let triggers = receiver.drain();
        assert_eq!(triggers.len(), 1);
        assert_eq!(triggers[0].sample, "kick");
        assert_eq!(triggers[0].play_for, Duration::from_millis(25));
    }

    #[test]
    fn receiver_drain_preserves_trigger_order() {
        let (sender, receiver) = sample_trigger_channel();

        sender
            .send(
                SampleTrigger::builder()
                    .sample("kick")
                    .play_for(Duration::from_millis(10))
                    .build()
                    .unwrap(),
            )
            .unwrap();
        sender
            .send(
                SampleTrigger::builder()
                    .sample("hat")
                    .play_for(Duration::from_millis(20))
                    .build()
                    .unwrap(),
            )
            .unwrap();

        let triggers = receiver.drain();

        assert_eq!(triggers.len(), 2);
        assert_eq!(triggers[0].sample, "kick");
        assert_eq!(triggers[1].sample, "hat");
    }

    #[test]
    fn builder_rejects_invalid_numeric_values() {
        assert!(
            panic::catch_unwind(|| {
                let _ = SampleTrigger::builder().playback_rate(0.0);
            })
            .is_err()
        );
        assert!(
            panic::catch_unwind(|| {
                let _ = SampleTrigger::builder().gain(-0.1);
            })
            .is_err()
        );
    }

    #[test]
    fn trigger_new_rejects_invalid_regions() {
        assert!(
            panic::catch_unwind(|| {
                let _ = SampleTrigger::new(SampleTriggerSettings {
                    voice_id: VoiceInstanceId::new(99),
                    runtime_controls: AudioRuntimeControlState::default(),
                    sample: "kick".to_string(),
                    sample_bank: None,
                    sample_variant: None,
                    pitch: None,
                    velocity: UnitValue::new(1.0).unwrap(),
                    gain: 1.0,
                    gain_ramp: None,
                    spatial: VoiceSpatial::ORIGIN,
                    playback_rate: 1.0,
                    playback_start: 0.8,
                    playback_end: 0.8,
                    reverse: false,
                    envelope: SampleEnvelope::new(
                        Duration::ZERO,
                        Duration::ZERO,
                        UnitValue::new(1.0).unwrap(),
                        Duration::ZERO,
                        None,
                        Duration::ZERO,
                    ),
                    low_pass_cutoff_hz: None,
                    low_pass_resonance: UnitValue::new(0.0).unwrap(),
                    high_pass_cutoff_hz: None,
                    high_pass_resonance: UnitValue::new(0.0).unwrap(),
                    post_gain: SampleTrigger::DEFAULT_POST_GAIN,
                    reverb: None,
                    delay: None,
                    compressor: None,
                    live_note: None,
                    play_for: Duration::from_millis(5),
                    modulations: Vec::new(),
                });
            })
            .is_err()
        );
    }

    #[test]
    fn scheduled_sample_translation_rejects_invalid_manual_sample_intents() {
        let event = scheduled_intent(
            Intent::Sample(SampleIntent {
                sample_id: "vox".to_string(),
                gain: 0.5,
                rate: 0.0,
                start: 0.0,
                end: 1.0,
                reverse: false,
            }),
            Duration::from_millis(25),
        );

        assert!(panic::catch_unwind(|| SampleTrigger::from_scheduled_intent(&event)).is_err());
    }

    #[test]
    fn scheduled_sample_translation_preserves_gain_ramp() {
        let projected = ProjectedMoment::new(
            Moment::new(
                Span::new(Time::ZERO, Time::ONE).unwrap(),
                Intent::sample("vox"),
            ),
            TransportSpan::new(Time::ZERO, Time::ONE).unwrap(),
            {
                let mut controls = ControlMap::new();
                controls.insert(ControlKey::Gain, ControlValue::Ramp { from: 1.0, to: 0.0 });
                controls
            },
        );
        let event = ScheduledIntent::new(
            IDGenerator::new(1).next(),
            projected,
            ClockTime::ZERO,
            Instant::now(),
            Duration::from_millis(25),
        );

        let trigger = SampleTrigger::from_scheduled_intent(&event).unwrap();

        assert_eq!(trigger.gain_ramp, Some(SampleGainRamp::new(1.0, 0.0)));
    }

    #[test]
    fn backfilled_forward_sample_starts_from_the_correct_interior_region() {
        let event = scheduled_intent_with_visible(
            Intent::Sample(SampleIntent::new("vox").region(0.0, 1.0)),
            ((0, 1), (1, 1)),
            ((1, 4), (1, 2)),
            ControlMap::new(),
        );

        let trigger = SampleTrigger::from_scheduled_intent(&event).unwrap();

        assert_eq!(trigger.playback_start, 0.25);
        assert_eq!(trigger.playback_end, 1.0);
        assert_eq!(trigger.play_for, Duration::from_millis(750));
    }

    #[test]
    fn backfilled_reverse_sample_starts_from_the_mirrored_region_point() {
        let event = scheduled_intent_with_visible(
            Intent::Sample(SampleIntent::new("vox").region(0.0, 1.0).reverse(true)),
            ((0, 1), (1, 1)),
            ((1, 4), (1, 2)),
            ControlMap::new(),
        );

        let trigger = SampleTrigger::from_scheduled_intent(&event).unwrap();

        assert_eq!(trigger.playback_start, 0.0);
        assert_eq!(trigger.playback_end, 0.75);
        assert!(trigger.reverse);
        assert_eq!(trigger.play_for, Duration::from_millis(750));
    }

    #[test]
    fn backfilled_gain_ramp_resumes_from_the_correct_interior_value() {
        let event = scheduled_intent_with_visible(
            Intent::sample("vox"),
            ((0, 1), (1, 1)),
            ((1, 2), (1, 1)),
            {
                let mut controls = ControlMap::new();
                controls.insert(ControlKey::Gain, ControlValue::Ramp { from: 1.0, to: 0.0 });
                controls
            },
        );

        let trigger = SampleTrigger::from_scheduled_intent(&event).unwrap();

        assert_eq!(trigger.gain_ramp, Some(SampleGainRamp::new(0.5, 0.0)));
        assert_eq!(trigger.play_for, Duration::from_millis(500));
    }

    #[test]
    fn song_slice_controls_survive_into_sample_trigger() {
        let event = scheduled_intent_with_visible(
            Intent::sample("kalimba"),
            ((0, 1), (1, 1)),
            ((0, 1), (1, 1)),
            {
                let mut controls = ControlMap::new();
                controls.insert(
                    ControlKey::SampleBank,
                    ControlValue::Choice(Symbol::from("gm")),
                );
                controls.insert(ControlKey::Pitch, ControlValue::Scalar(62.0));
                controls.insert(
                    ControlKey::Velocity,
                    ControlValue::Unipolar(UnitValue::new(0.8).unwrap()),
                );
                controls.insert(ControlKey::Legato, ControlValue::Scalar(1.2));
                controls.insert(ControlKey::Attack, ControlValue::Scalar(0.025));
                controls.insert(ControlKey::Decay, ControlValue::Scalar(0.05));
                controls.insert(
                    ControlKey::Sustain,
                    ControlValue::Unipolar(UnitValue::new(0.7).unwrap()),
                );
                controls.insert(ControlKey::Release, ControlValue::Scalar(0.2));
                controls.insert(ControlKey::LowPassCutoff, ControlValue::Scalar(1200.0));
                controls.insert(
                    ControlKey::LowPassResonance,
                    ControlValue::Unipolar(UnitValue::new(0.45).unwrap()),
                );
                controls.insert(ControlKey::HighPassCutoff, ControlValue::Scalar(180.0));
                controls.insert(
                    ControlKey::HighPassResonance,
                    ControlValue::Unipolar(UnitValue::new(0.2).unwrap()),
                );
                controls.insert(ControlKey::PostGain, ControlValue::Scalar(1.5));
                controls.insert(
                    ControlKey::ReverbSend,
                    ControlValue::Reverb(ReverbSettings::new(
                        UnitValue::new(0.4).unwrap(),
                        Duration::from_secs_f64(2.0),
                        UnitValue::new(0.3).unwrap(),
                    )),
                );
                controls.insert(
                    ControlKey::DelaySend,
                    ControlValue::Delay(DelaySettings::new(
                        UnitValue::new(0.35).unwrap(),
                        Duration::from_millis(360),
                        UnitValue::new(0.3).unwrap(),
                        UnitValue::new(0.2).unwrap(),
                    )),
                );
                controls.insert(
                    ControlKey::Compressor,
                    ControlValue::Compressor(CompressorSettings::new(
                        UnitValue::new(0.35).unwrap(),
                        4.0,
                        Duration::from_millis(5),
                        Duration::from_millis(40),
                    )),
                );
                controls
            },
        );

        let trigger = SampleTrigger::from_scheduled_intent(&event).unwrap();

        assert_eq!(trigger.sample_bank, Some(Symbol::from("gm")));
        assert_eq!(trigger.pitch, Some(62.0));
        assert_eq!(trigger.velocity, UnitValue::new(0.8).unwrap());
        assert_eq!(trigger.low_pass_cutoff_hz, Some(1200.0));
        assert_eq!(trigger.low_pass_resonance, UnitValue::new(0.45).unwrap());
        assert_eq!(trigger.high_pass_cutoff_hz, Some(180.0));
        assert_eq!(trigger.high_pass_resonance, UnitValue::new(0.2).unwrap());
        assert_eq!(trigger.post_gain, 1.5);
        assert!(trigger.reverb.is_some());
        assert!(trigger.delay.is_some());
        assert!(trigger.compressor.is_some());
        assert_eq!(trigger.envelope.attack(), Duration::from_millis(25));
        assert_eq!(trigger.envelope.release(), Duration::from_millis(200));
    }

    #[test]
    fn live_sample_pitch_bend_offsets_pitch_control() {
        let mut controls = ControlMap::new();
        controls.insert(ControlKey::Pitch, ControlValue::Scalar(60.0));
        controls.insert(
            ControlKey::PitchBend,
            ControlValue::Bipolar(SignedUnitValue::new(1.0).unwrap()),
        );

        let trigger = SampleTrigger::from_live_sample(
            &SampleIntent::new("bass"),
            &controls,
            None,
            Some(NoteNumber::new(60).unwrap()),
            Duration::ZERO,
        )
        .unwrap();

        assert_eq!(trigger.pitch, Some(60.0));
        assert_eq!(
            trigger.runtime_controls.pitch_bend_semitones,
            crate::application::audio::DEFAULT_PITCH_BEND_RANGE_SEMITONES
        );
    }
}
