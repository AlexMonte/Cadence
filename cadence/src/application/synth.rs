//! Synth-trigger lowering from typed musical intent.

use std::time::Duration;

use crate::{
    application::{
        audio::{
            AudioRuntimeControlState, AudioSourcePlan, AudioVoicePlan, SignalBinding,
            VoiceInstanceId, VoiceSpatial,
        },
        scheduler::events::scheduled_intent::ScheduledIntent,
    },
    domain::{
        control::{CompressorSettings, ControlMap, DelaySettings, ReverbSettings, UnitValue},
        input::NoteNumber,
        intent::BuiltInSynthSource,
    },
};

use super::sample::{SampleEnvelope, SampleGainRamp};

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

/// Synth-envelope alias shared with sample playback.
pub type SynthEnvelope = SampleEnvelope;
/// Synth gain-ramp alias shared with sample playback.
pub type SynthGainRamp = SampleGainRamp;

#[derive(Debug, Clone, PartialEq)]
/// Concrete playback packet for a built-in synth voice.
pub struct SynthTrigger {
    /// Concrete voice instance this trigger belongs to.
    pub voice_id: VoiceInstanceId,
    /// Initial runtime control state for the active voice.
    pub runtime_controls: AudioRuntimeControlState,
    /// Built-in synth source to render.
    pub source: BuiltInSynthSource,
    /// Pitch value, usually in MIDI-note/semitone space.
    pub pitch: f64,
    /// Final velocity after control merging.
    pub velocity: UnitValue,
    /// Final gain multiplier.
    pub gain: f64,
    /// Optional gain ramp.
    pub gain_ramp: Option<SynthGainRamp>,
    /// Spatial placement and cycle clock for per-frame position.
    pub spatial: VoiceSpatial,
    /// Envelope settings.
    pub envelope: SynthEnvelope,
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
    /// Live note to release later, if any.
    pub live_note: Option<NoteNumber>,
    /// Remaining wall-clock time the note should be held for.
    pub play_for: Duration,
    /// Continuous signal bindings evaluated per output frame.
    pub modulations: Vec<SignalBinding>,
}

impl SynthTrigger {
    /// Default gain multiplier.
    pub const DEFAULT_GAIN: f64 = 1.0;
    /// Default post-effect gain.
    pub const DEFAULT_POST_GAIN: f64 = 1.0;
    /// Default pitch value when no pitch control is present.
    pub const DEFAULT_PITCH: f64 = 60.0;

    /// Creates a validated synth trigger.
    ///
    /// # Panics
    ///
    /// Panics when any scalar precondition is violated.
    #[must_use]
    pub fn new(settings: SynthTriggerSettings) -> Self {
        assert_finite("synth trigger pitch", settings.pitch);
        assert_non_negative("synth trigger gain", settings.gain);
        if let Some(cutoff) = settings.low_pass_cutoff_hz {
            assert_positive("synth trigger low-pass cutoff", cutoff);
        }
        if let Some(cutoff) = settings.high_pass_cutoff_hz {
            assert_positive("synth trigger high-pass cutoff", cutoff);
        }
        assert_non_negative("synth trigger post-gain", settings.post_gain);

        Self {
            voice_id: settings.voice_id,
            runtime_controls: settings.runtime_controls,
            source: settings.source,
            pitch: settings.pitch,
            velocity: settings.velocity,
            gain: settings.gain,
            gain_ramp: settings.gain_ramp,
            spatial: settings.spatial,
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

    /// Returns a builder for constructing a synth trigger incrementally.
    #[must_use]
    pub fn builder() -> SynthTriggerBuilder {
        SynthTriggerBuilder::new()
    }

    /// Converts a scheduled intent into a synth trigger with no ambient
    /// controls.
    #[must_use]
    pub fn from_scheduled_intent(event: &ScheduledIntent) -> Option<Self> {
        Self::from_scheduled_intent_with_ambient(event, &ControlMap::new())
    }

    /// Converts a scheduled intent into a synth trigger using ambient controls.
    #[must_use]
    pub fn from_scheduled_intent_with_ambient(
        event: &ScheduledIntent,
        ambient_controls: &ControlMap,
    ) -> Option<Self> {
        AudioVoicePlan::from_scheduled_intent_with_ambient(event, ambient_controls)
            .and_then(|plan| Self::from_voice_plan(&plan))
    }

    /// Converts a live synth source plus controls into a trigger.
    #[must_use]
    pub fn from_live_source(
        source: BuiltInSynthSource,
        controls: &ControlMap,
        velocity: Option<UnitValue>,
        live_note: Option<NoteNumber>,
        play_for: Duration,
    ) -> Option<Self> {
        AudioVoicePlan::from_live_synth(
            VoiceInstanceId::next_live(),
            source,
            controls,
            velocity,
            live_note,
            play_for,
        )
        .and_then(|plan| Self::from_voice_plan(&plan))
    }

    /// Converts one shared audio voice plan into a synth trigger.
    #[must_use]
    pub fn from_voice_plan(plan: &AudioVoicePlan) -> Option<Self> {
        let AudioSourcePlan::Synth(source) = &plan.source else {
            return None;
        };

        let mut builder = Self::builder()
            .voice_id(plan.voice_id)
            .runtime_controls(plan.runtime_controls)
            .source(source.source)
            .pitch(source.pitch)
            .velocity(plan.mix.velocity)
            .gain(plan.mix.gain)
            .spatial(plan.spatial)
            .envelope(SynthEnvelope::new(
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
/// Field-based settings struct accepted by [`SynthTrigger::new`].
pub struct SynthTriggerSettings {
    /// Concrete voice instance this trigger belongs to.
    pub voice_id: VoiceInstanceId,
    /// Initial runtime control state for the active voice.
    pub runtime_controls: AudioRuntimeControlState,
    /// Built-in source to render.
    pub source: BuiltInSynthSource,
    /// Final pitch.
    pub pitch: f64,
    /// Final velocity.
    pub velocity: UnitValue,
    /// Final gain multiplier.
    pub gain: f64,
    /// Optional gain ramp.
    pub gain_ramp: Option<SynthGainRamp>,
    /// Spatial placement and cycle clock for per-frame position.
    pub spatial: VoiceSpatial,
    /// Envelope settings.
    pub envelope: SynthEnvelope,
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
/// Builder for [`SynthTrigger`].
pub struct SynthTriggerBuilder {
    voice_id: Option<VoiceInstanceId>,
    runtime_controls: Option<AudioRuntimeControlState>,
    source: Option<BuiltInSynthSource>,
    pitch: Option<f64>,
    velocity: Option<UnitValue>,
    gain: Option<f64>,
    gain_ramp: Option<SynthGainRamp>,
    spatial: Option<VoiceSpatial>,
    envelope: Option<SynthEnvelope>,
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

impl SynthTriggerBuilder {
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

    /// Sets the synth source.
    #[must_use]
    pub fn source(mut self, source: BuiltInSynthSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Sets pitch.
    ///
    /// # Panics
    ///
    /// Panics if `pitch` is not finite.
    #[must_use]
    pub fn pitch(mut self, pitch: f64) -> Self {
        assert_finite("synth trigger pitch", pitch);
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
        assert_non_negative("synth trigger gain", gain);
        self.gain = Some(gain);
        self
    }

    /// Sets an explicit gain ramp.
    #[must_use]
    pub fn gain_ramp(mut self, gain_ramp: SynthGainRamp) -> Self {
        self.gain_ramp = Some(gain_ramp);
        self
    }

    /// Sets the spatial placement and cycle clock.
    #[must_use]
    pub fn spatial(mut self, spatial: VoiceSpatial) -> Self {
        self.spatial = Some(spatial);
        self
    }

    /// Sets the envelope.
    #[must_use]
    pub fn envelope(mut self, envelope: SynthEnvelope) -> Self {
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
        assert_positive("synth trigger low-pass cutoff", cutoff);
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
        assert_positive("synth trigger high-pass cutoff", cutoff);
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
        assert_non_negative("synth trigger post-gain", post_gain);
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
    /// Returns `None` when a required field such as `source` or `pitch` was
    /// never provided.
    #[must_use]
    pub fn build(self) -> Option<SynthTrigger> {
        Some(SynthTrigger::new(SynthTriggerSettings {
            voice_id: self.voice_id.unwrap_or_else(VoiceInstanceId::next_live),
            runtime_controls: self.runtime_controls.unwrap_or_default(),
            source: self.source?,
            pitch: self.pitch?,
            velocity: self
                .velocity
                .unwrap_or_else(|| UnitValue::new(1.0).unwrap()),
            gain: self.gain.unwrap_or(SynthTrigger::DEFAULT_GAIN),
            gain_ramp: self.gain_ramp,
            spatial: self.spatial.unwrap_or_default(),
            envelope: self.envelope.unwrap_or_else(|| {
                SynthEnvelope::new(
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
            post_gain: self.post_gain.unwrap_or(SynthTrigger::DEFAULT_POST_GAIN),
            reverb: self.reverb,
            delay: self.delay,
            compressor: self.compressor,
            live_note: self.live_note,
            play_for: self.play_for.unwrap_or(Duration::ZERO),
            modulations: self.modulations,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::{
        application::{
            clock::ClockTime,
            scheduler::events::{IDGenerator, scheduled_intent::ScheduledIntent},
        },
        domain::{
            control::{
                ControlKey, ControlMap, ControlValue, ReverbSettings, SignedUnitValue, UnitValue,
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

    #[test]
    fn synth_intent_with_pitch_becomes_trigger() {
        let mut controls = ControlMap::new();
        controls.insert(ControlKey::Pitch, ControlValue::Scalar(69.0));
        controls.insert(ControlKey::Gain, ControlValue::Scalar(0.5));
        controls.insert(ControlKey::LowPassCutoff, ControlValue::Scalar(800.0));
        let event = ScheduledIntent::new(
            IDGenerator::new(1).next(),
            ProjectedMoment::new(
                Moment::new(
                    Span::new(Time::ZERO, Time::ONE).unwrap(),
                    Intent::synth(BuiltInSynthSource::Sine),
                ),
                TransportSpan::new(Time::ZERO, Time::ONE).unwrap(),
                controls,
            ),
            ClockTime::ZERO,
            Instant::now(),
            Duration::from_millis(100),
        );

        let trigger = SynthTrigger::from_scheduled_intent(&event).unwrap();

        assert_eq!(trigger.source, BuiltInSynthSource::Sine);
        assert_eq!(trigger.pitch, 69.0);
        assert_eq!(trigger.gain, 0.5);
        assert_eq!(trigger.low_pass_cutoff_hz, Some(800.0));
    }

    #[test]
    fn synth_intent_without_pitch_defaults_to_middle_c() {
        let event = scheduled_intent(Intent::synth(BuiltInSynthSource::Saw), Duration::ZERO);

        let trigger = SynthTrigger::from_scheduled_intent(&event).unwrap();

        assert_eq!(trigger.pitch, SynthTrigger::DEFAULT_PITCH);
    }

    #[test]
    fn live_synth_uses_velocity_and_room_controls() {
        let mut controls = ControlMap::new();
        controls.insert(ControlKey::Pitch, ControlValue::Scalar(60.0));
        controls.insert(
            ControlKey::ReverbSend,
            ControlValue::Reverb(ReverbSettings::new(
                UnitValue::new(0.3).unwrap(),
                std::time::Duration::from_secs_f64(2.0),
                UnitValue::new(0.3).unwrap(),
            )),
        );

        let trigger = SynthTrigger::from_live_source(
            BuiltInSynthSource::Triangle,
            &controls,
            Some(UnitValue::new(0.8).unwrap()),
            Some(NoteNumber::new(60).unwrap()),
            Duration::ZERO,
        )
        .unwrap();

        assert_eq!(trigger.velocity, UnitValue::new(0.8).unwrap());
        assert_eq!(trigger.spatial, VoiceSpatial::ORIGIN);
        assert_eq!(
            trigger.reverb.unwrap().amount(),
            UnitValue::new(0.3).unwrap()
        );
        assert_eq!(trigger.live_note, Some(NoteNumber::new(60).unwrap()));
    }

    #[test]
    fn pitch_bend_offsets_live_synth_pitch() {
        let mut controls = ControlMap::new();
        controls.insert(
            ControlKey::PitchBend,
            ControlValue::Bipolar(SignedUnitValue::new(1.0).unwrap()),
        );

        let trigger = SynthTrigger::from_live_source(
            BuiltInSynthSource::Triangle,
            &controls,
            None,
            Some(NoteNumber::new(60).unwrap()),
            Duration::ZERO,
        )
        .unwrap();

        assert_eq!(trigger.pitch, SynthTrigger::DEFAULT_PITCH);
        assert_eq!(
            trigger.runtime_controls.pitch_bend_semitones,
            crate::application::audio::DEFAULT_PITCH_BEND_RANGE_SEMITONES
        );
    }
}
