//! Mixed sample/synth audio lowering for the audio path.

use std::sync::{
    Arc, RwLock,
    mpsc::{self, Receiver, SendError, Sender, TryRecvError},
};
use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::Duration,
};

use crate::{
    application::{
        performer::Performer,
        scheduler::events::scheduled_intent::{ScheduledIntent, ScheduledIntentKind},
    },
    domain::{
        control::{
            CompressorSettings, ControlKey, ControlMap, ControlValue, DelaySettings,
            ReverbSettings, Symbol, UnitValue,
        },
        input::NoteNumber,
        intent::BuiltInSynthSource,
        signal::Signal,
        space::SpatialMotion,
    },
};

pub(crate) const DEFAULT_PITCH_BEND_RANGE_SEMITONES: f64 = 2.0;

static LIVE_VOICE_INSTANCE_ID_GENERATOR: AtomicI64 = AtomicI64::new(1_000_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Concrete voice instance tracked by the audio runtime.
pub struct VoiceInstanceId(i64);

impl VoiceInstanceId {
    /// Creates a new live-input voice identifier.
    #[must_use]
    pub fn next_live() -> Self {
        Self(LIVE_VOICE_INSTANCE_ID_GENERATOR.fetch_add(1, Ordering::Relaxed))
    }

    /// Creates a deterministic voice identifier from a raw integer.
    #[must_use]
    pub fn new(value: i64) -> Self {
        Self(value)
    }

    /// Returns the raw integer value.
    #[must_use]
    pub fn value(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// How one runtime control field should change.
pub enum RuntimeControlValue<T> {
    /// Leave the field unchanged.
    Keep,
    /// Set the field to a specific value.
    Set(T),
    /// Reset the field to its subsystem default.
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Shared runtime control state consumed by active voices.
pub struct AudioRuntimeControlState {
    /// Additional pitch bend in semitones.
    pub pitch_bend_semitones: f64,
    /// Expression gain multiplier.
    pub expression: UnitValue,
    /// Mod-wheel amount.
    pub mod_wheel: UnitValue,
    /// Sustain pedal state.
    pub sustain_pedal: bool,
}

impl Default for AudioRuntimeControlState {
    fn default() -> Self {
        Self {
            pitch_bend_semitones: 0.0,
            expression: UnitValue::new(1.0).unwrap(),
            mod_wheel: UnitValue::new(0.0).unwrap(),
            sustain_pedal: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Partial update applied to an active voice's runtime controls.
pub struct AudioRuntimeControlDelta {
    /// Change applied to pitch bend in semitones.
    pub pitch_bend_semitones: RuntimeControlValue<f64>,
    /// Change applied to expression gain.
    pub expression: RuntimeControlValue<UnitValue>,
    /// Change applied to mod-wheel depth.
    pub mod_wheel: RuntimeControlValue<UnitValue>,
    /// Change applied to sustain-pedal state.
    pub sustain_pedal: RuntimeControlValue<bool>,
    /// Change applied to the voice's linear gain multiplier.
    pub gain: RuntimeControlValue<f32>,
    /// Change applied to post-fader gain.
    pub post_gain: RuntimeControlValue<f32>,
    /// Change applied to sample playback rate.
    pub playback_rate: RuntimeControlValue<f64>,
}

impl Default for AudioRuntimeControlDelta {
    fn default() -> Self {
        Self {
            pitch_bend_semitones: RuntimeControlValue::Keep,
            expression: RuntimeControlValue::Keep,
            mod_wheel: RuntimeControlValue::Keep,
            sustain_pedal: RuntimeControlValue::Keep,
            gain: RuntimeControlValue::Keep,
            post_gain: RuntimeControlValue::Keep,
            playback_rate: RuntimeControlValue::Keep,
        }
    }
}

impl AudioRuntimeControlDelta {
    /// Builds a control delta from one live control message, if it belongs to
    /// the active-runtime control set.
    #[must_use]
    pub fn from_live_control(key: &ControlKey, value: &ControlValue) -> Option<Self> {
        let mut delta = Self::default();

        match (key, value) {
            (ControlKey::PitchBend, ControlValue::Bipolar(value)) => {
                delta.pitch_bend_semitones =
                    RuntimeControlValue::Set(value.value() * DEFAULT_PITCH_BEND_RANGE_SEMITONES);
            }
            (ControlKey::Expression, ControlValue::Unipolar(value)) => {
                delta.expression = RuntimeControlValue::Set(*value);
            }
            (ControlKey::ModWheel, ControlValue::Unipolar(value)) => {
                delta.mod_wheel = RuntimeControlValue::Set(*value);
            }
            (ControlKey::SustainPedal, ControlValue::Bool(value)) => {
                delta.sustain_pedal = RuntimeControlValue::Set(*value);
            }
            (ControlKey::PostGain, value) => {
                let Some(scalar) = control_to_non_negative_scalar(Some(value)) else {
                    return None;
                };
                delta.post_gain = RuntimeControlValue::Set(scalar as f32);
            }
            (ControlKey::Gain, value) => {
                let Some(scalar) = control_to_gain_scalar(Some(value)) else {
                    return None;
                };
                delta.gain = RuntimeControlValue::Set(scalar as f32);
            }
            _ => return None,
        }

        Some(delta)
    }

    /// Builds a partial runtime-control update from projected controls.
    ///
    /// Only keys present in `controls` are set; everything else stays [`RuntimeControlValue::Keep`]
    /// so scheduled updates do not reset unrelated parameters.
    #[must_use]
    pub fn from_runtime_controls_snapshot(controls: &ControlMap) -> Self {
        let mut delta = Self::default();

        if let Some(ControlValue::Bipolar(value)) = controls.get(&ControlKey::PitchBend) {
            delta.pitch_bend_semitones =
                RuntimeControlValue::Set(value.value() * DEFAULT_PITCH_BEND_RANGE_SEMITONES);
        }
        if let Some(ControlValue::Unipolar(value)) = controls.get(&ControlKey::Expression) {
            delta.expression = RuntimeControlValue::Set(*value);
        }
        if let Some(ControlValue::Unipolar(value)) = controls.get(&ControlKey::ModWheel) {
            delta.mod_wheel = RuntimeControlValue::Set(*value);
        }
        if let Some(ControlValue::Bool(value)) = controls.get(&ControlKey::SustainPedal) {
            delta.sustain_pedal = RuntimeControlValue::Set(*value);
        }
        if let Some(value) = control_to_gain_scalar(controls.get(&ControlKey::Gain)) {
            delta.gain = RuntimeControlValue::Set(value as f32);
        }
        if let Some(value) = control_to_non_negative_scalar(controls.get(&ControlKey::PostGain)) {
            delta.post_gain = RuntimeControlValue::Set(value as f32);
        }
        if let Some(value) = control_to_positive_scalar(controls.get(&ControlKey::PlaybackRate)) {
            delta.playback_rate = RuntimeControlValue::Set(value);
        }

        delta
    }

    /// Applies the delta to a mutable runtime state.
    pub fn apply_to(self, state: &mut AudioRuntimeControlState) {
        match self.pitch_bend_semitones {
            RuntimeControlValue::Keep => {}
            RuntimeControlValue::Set(value) => state.pitch_bend_semitones = value,
            RuntimeControlValue::Reset => state.pitch_bend_semitones = 0.0,
        }
        match self.expression {
            RuntimeControlValue::Keep => {}
            RuntimeControlValue::Set(value) => state.expression = value,
            RuntimeControlValue::Reset => state.expression = UnitValue::new(1.0).unwrap(),
        }
        match self.mod_wheel {
            RuntimeControlValue::Keep => {}
            RuntimeControlValue::Set(value) => state.mod_wheel = value,
            RuntimeControlValue::Reset => state.mod_wheel = UnitValue::new(0.0).unwrap(),
        }
        match self.sustain_pedal {
            RuntimeControlValue::Keep => {}
            RuntimeControlValue::Set(value) => state.sustain_pedal = value,
            RuntimeControlValue::Reset => state.sustain_pedal = false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Per-voice lifecycle timing facts.
pub struct VoiceLifecycle {
    /// Total gate length used before automatic release begins.
    pub gate_duration: Option<Duration>,
    /// Already-elapsed playback time when the voice starts in progress.
    pub elapsed: Duration,
    /// Remaining scheduled play duration from the current window entry.
    pub play_for: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Envelope parameters shared by sample and synth voices.
pub struct EnvelopePlan {
    /// Attack time before the voice reaches full level.
    pub attack: Duration,
    /// Decay time from peak level toward sustain.
    pub decay: Duration,
    /// Sustain level held after attack and decay complete.
    pub sustain_level: UnitValue,
    /// Release time after gate end or manual note release.
    pub release: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Gain and velocity shaping shared by sample and synth voices.
pub struct MixPlan {
    /// Velocity multiplier applied before DSP processing.
    pub velocity: UnitValue,
    /// Base gain resolved at voice start.
    pub gain: f64,
    /// Optional segment-local gain ramp.
    pub gain_ramp: Option<crate::application::sample::SampleGainRamp>,
    /// Post-DSP gain multiplier.
    pub post_gain: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Spatial placement of a voice plus the cycle clock needed to evaluate a
/// time-varying [`SpatialMotion`] from the audio thread.
///
/// Position is first-class moment data, not a control lane; the audio thread
/// resolves it per output frame and hands the coordinates to a `Spatializer`.
pub struct VoiceSpatial {
    /// Spatial trajectory resolved per output frame.
    pub motion: SpatialMotion,
    /// Transport cycle position when the owning voice started.
    pub start_cycle: f64,
    /// Transport cycles per second used to convert elapsed seconds to cycles.
    pub cps: f64,
}

impl VoiceSpatial {
    /// A static voice fixed at the lattice origin with a unit cycle clock.
    pub const ORIGIN: Self = Self {
        motion: SpatialMotion::ORIGIN,
        start_cycle: 0.0,
        cps: 1.0,
    };

    /// Resolves the position for the given number of rendered output frames.
    #[must_use]
    pub fn position_at_frame(self, rendered_frames: usize, sample_rate: u32) -> (f64, f64, f64) {
        let seconds = rendered_frames as f64 / sample_rate.max(1) as f64;
        self.motion
            .position_at(self.start_cycle + seconds * self.cps)
    }
}

impl Default for VoiceSpatial {
    fn default() -> Self {
        Self::ORIGIN
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Filter settings shared by sample and synth voices.
pub struct FilterPlan {
    /// Optional low-pass cutoff frequency in hertz.
    pub low_pass_cutoff_hz: Option<f64>,
    /// Low-pass resonance amount.
    pub low_pass_resonance: UnitValue,
    /// Optional high-pass cutoff frequency in hertz.
    pub high_pass_cutoff_hz: Option<f64>,
    /// High-pass resonance amount.
    pub high_pass_resonance: UnitValue,
}

#[derive(Debug, Clone, PartialEq)]
/// Send-effect routing shared by sample and synth voices.
pub struct SendPlan {
    /// Reverb-send configuration, if enabled.
    pub reverb: Option<ReverbSettings>,
    /// Delay-send configuration, if enabled.
    pub delay: Option<DelaySettings>,
}

#[derive(Debug, Clone, PartialEq)]
/// Dynamics settings shared by sample and synth voices.
pub struct DynamicsPlan {
    /// Compressor settings, if enabled.
    pub compressor: Option<CompressorSettings>,
}

#[derive(Debug, Clone, PartialEq)]
/// Sample-specific source transport settings.
pub struct SampleSourcePlan {
    /// Logical sample identifier to resolve.
    pub sample: String,
    /// Optional named sample-bank override.
    pub sample_bank: Option<Symbol>,
    /// Optional sample-variant selection.
    pub sample_variant: Option<usize>,
    /// Optional musical pitch used for repitching and bend.
    pub pitch: Option<f64>,
    /// Playback-rate multiplier.
    pub playback_rate: f64,
    /// Unit-range playback start position.
    pub playback_start: f64,
    /// Unit-range playback end position.
    pub playback_end: f64,
    /// Whether the sample plays in reverse.
    pub reverse: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Built-in synth source settings.
pub struct SynthSourcePlan {
    /// Built-in oscillator shape.
    pub source: BuiltInSynthSource,
    /// Base pitch in MIDI-note / semitone space.
    pub pitch: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// Source-specific payload for one audio voice.
pub enum AudioSourcePlan {
    /// Sample-backed playback source plan.
    Sample(SampleSourcePlan),
    /// Built-in synth source plan.
    Synth(SynthSourcePlan),
}

#[derive(Debug, Clone, PartialEq)]
/// One continuous signal bound to a voice control lane for per-frame
/// modulation.
///
/// The binding carries the cycle position at which the voice starts plus the
/// transport `cps` (cycles per second), so the audio thread can recover
/// cycle-relative phase from the number of rendered output frames.
pub struct SignalBinding {
    /// Control lane this signal modulates.
    pub key: ControlKey,
    /// Continuous signal evaluated per output frame.
    pub signal: Signal,
    /// Transport cycle position when the owning voice started.
    pub start_cycle: f64,
    /// Transport cycles per second used to convert elapsed seconds to cycles.
    pub cps: f64,
}

#[derive(Debug, Clone, PartialEq)]
/// First-class audio voice plan consumed by lower renderer layers.
pub struct AudioVoicePlan {
    /// Concrete voice instance this plan starts.
    pub voice_id: VoiceInstanceId,
    /// Source-specific playback settings.
    pub source: AudioSourcePlan,
    /// Lifecycle timing facts for the voice.
    pub lifecycle: VoiceLifecycle,
    /// Envelope settings owned by the voice lifecycle subsystem.
    pub envelope: EnvelopePlan,
    /// Gain and velocity settings owned by the mix subsystem.
    pub mix: MixPlan,
    /// Spatial placement and cycle clock owned by the spatializer subsystem.
    pub spatial: VoiceSpatial,
    /// Filter settings owned by the filter subsystem.
    pub filters: FilterPlan,
    /// Send-bus routing owned by the effects subsystem.
    pub sends: SendPlan,
    /// Dynamics settings owned by the dynamics subsystem.
    pub dynamics: DynamicsPlan,
    /// Initial runtime control snapshot for active-voice modulation.
    pub runtime_controls: AudioRuntimeControlState,
    /// Continuous signal bindings evaluated per output frame on the audio
    /// thread.
    pub modulations: Vec<SignalBinding>,
    /// Optional live note associated with this voice.
    pub live_note: Option<NoteNumber>,
}

#[derive(Debug, Clone, PartialEq)]
/// Audio-facing event produced after scheduling and control resolution.
pub enum ScheduledAudioEvent {
    /// Start one new voice instance.
    StartVoice(AudioVoicePlan),
    /// Update runtime control state for an existing voice.
    UpdateVoiceControls {
        /// Voice instance that should receive the update.
        voice_id: VoiceInstanceId,
        /// Runtime control delta to apply.
        delta: AudioRuntimeControlDelta,
    },
    /// Release one existing voice instance.
    ReleaseVoice(VoiceInstanceId),
}

impl AudioVoicePlan {
    /// Builds one audio voice plan from a scheduled start event.
    #[must_use]
    pub fn from_scheduled_intent_with_ambient(
        event: &ScheduledIntent,
        ambient_controls: &ControlMap,
    ) -> Option<Self> {
        let ScheduledIntentKind::StartVoice { voice_id, .. } = event.kind() else {
            return None;
        };

        match event.intent() {
            crate::domain::intent::Intent::Sample(sample) => {
                let mut controls = ambient_controls.clone();
                controls.extend(event.controls().clone());
                sample_voice_plan_from_controls(
                    voice_id,
                    sample,
                    &controls,
                    None,
                    None,
                    event.play_for,
                    Some(event),
                )
            }
            crate::domain::intent::Intent::Synth(intent) => {
                let mut controls = ambient_controls.clone();
                controls.extend(event.controls().clone());
                synth_voice_plan_from_controls(
                    voice_id,
                    intent.source,
                    &controls,
                    None,
                    None,
                    event.play_for,
                    Some(event),
                )
            }
            _ => None,
        }
    }

    /// Builds a live sample voice plan from an already-merged control map.
    #[must_use]
    pub fn from_live_sample(
        voice_id: VoiceInstanceId,
        sample: &crate::domain::intent::SampleIntent,
        controls: &ControlMap,
        velocity: Option<UnitValue>,
        live_note: Option<NoteNumber>,
        play_for: Duration,
    ) -> Option<Self> {
        sample_voice_plan_from_controls(
            voice_id, sample, controls, velocity, live_note, play_for, None,
        )
    }

    /// Builds a live synth voice plan from an already-merged control map.
    #[must_use]
    pub fn from_live_synth(
        voice_id: VoiceInstanceId,
        source: BuiltInSynthSource,
        controls: &ControlMap,
        velocity: Option<UnitValue>,
        live_note: Option<NoteNumber>,
        play_for: Duration,
    ) -> Option<Self> {
        synth_voice_plan_from_controls(
            voice_id, source, controls, velocity, live_note, play_for, None,
        )
    }
}

impl ScheduledAudioEvent {
    /// Lowers one scheduled intent into a concrete audio event.
    #[must_use]
    pub fn from_scheduled_intent_with_ambient(
        event: &ScheduledIntent,
        ambient_controls: &ControlMap,
    ) -> Option<Self> {
        match event.kind() {
            ScheduledIntentKind::StartVoice { .. } => {
                AudioVoicePlan::from_scheduled_intent_with_ambient(event, ambient_controls)
                    .map(Self::StartVoice)
            }
            ScheduledIntentKind::UpdateVoiceControls { voice_id, .. } => {
                Some(Self::UpdateVoiceControls {
                    voice_id,
                    delta: AudioRuntimeControlDelta::from_runtime_controls_snapshot(
                        event.controls(),
                    ),
                })
            }
        }
    }
}

fn sample_voice_plan_from_controls(
    voice_id: VoiceInstanceId,
    sample: &crate::domain::intent::SampleIntent,
    controls: &ControlMap,
    velocity: Option<UnitValue>,
    live_note: Option<NoteNumber>,
    play_for: Duration,
    event: Option<&ScheduledIntent>,
) -> Option<AudioVoicePlan> {
    if matches!(
        controls.get(&ControlKey::Gate),
        Some(ControlValue::Bool(false))
    ) {
        return None;
    }

    let mut playback_start =
        control_to_scalar(controls.get(&ControlKey::PlaybackStart)).unwrap_or(sample.start);
    let mut playback_end =
        control_to_scalar(controls.get(&ControlKey::PlaybackEnd)).unwrap_or(sample.end);
    let reverse = match controls.get(&ControlKey::Reverse) {
        Some(ControlValue::Bool(value)) => *value,
        _ => sample.reverse,
    };
    let mut gain_ramp = match controls.get(&ControlKey::Gain) {
        Some(ControlValue::Ramp { from, to }) => {
            Some(crate::application::sample::SampleGainRamp::new(*from, *to))
        }
        _ => None,
    };

    if let Some(event) = event {
        if let crate::application::scheduler::events::scheduled_intent::ScheduledEntry::InProgress {
            elapsed,
        } = event.entry()
        {
            let progress = (elapsed / event.duration()).value().clamp(0.0, 1.0);
            let entry_span = crate::domain::span::TransportSpan::new(
                event.entry_time(),
                event.voice_whole().end(),
            )
            .unwrap();

            gain_ramp = gain_ramp.map(|ramp| ramp.slice_for(event.voice_whole(), entry_span));

            if reverse {
                playback_end = lerp(playback_start, playback_end, 1.0 - progress);
            } else {
                playback_start = lerp(playback_start, playback_end, progress);
            }
        }
    }

    Some(AudioVoicePlan {
        voice_id,
        source: AudioSourcePlan::Sample(SampleSourcePlan {
            sample: sample.sample_id.clone(),
            sample_bank: controls
                .get(&ControlKey::SampleBank)
                .and_then(control_to_symbol),
            sample_variant: control_to_variant_index(controls.get(&ControlKey::SampleVariant)),
            pitch: control_to_scalar(controls.get(&ControlKey::Pitch)),
            playback_rate: control_to_positive_scalar(controls.get(&ControlKey::PlaybackRate))
                .unwrap_or(sample.rate),
            playback_start,
            playback_end,
            reverse,
        }),
        lifecycle: lifecycle_from_controls(controls, play_for, event),
        envelope: envelope_plan_from_controls(controls),
        mix: mix_plan_from_controls(sample.gain, controls, velocity, gain_ramp),
        spatial: voice_spatial_from_event(event),
        filters: filter_plan_from_controls(controls),
        sends: send_plan_from_controls(controls),
        dynamics: dynamics_plan_from_controls(controls),
        runtime_controls: runtime_control_state_from_controls(controls),
        modulations: signal_bindings_from_controls(controls, event),
        live_note,
    })
}

fn synth_voice_plan_from_controls(
    voice_id: VoiceInstanceId,
    source: BuiltInSynthSource,
    controls: &ControlMap,
    velocity: Option<UnitValue>,
    live_note: Option<NoteNumber>,
    play_for: Duration,
    event: Option<&ScheduledIntent>,
) -> Option<AudioVoicePlan> {
    if matches!(
        controls.get(&ControlKey::Gate),
        Some(ControlValue::Bool(false))
    ) {
        return None;
    }

    let mut gain_ramp = match controls.get(&ControlKey::Gain) {
        Some(ControlValue::Ramp { from, to }) => {
            Some(crate::application::sample::SampleGainRamp::new(*from, *to))
        }
        _ => None,
    };

    if let Some(event) = event {
        if matches!(
            event.entry(),
            crate::application::scheduler::events::scheduled_intent::ScheduledEntry::InProgress { .. }
        ) {
            let entry_span = crate::domain::span::TransportSpan::new(
                event.entry_time(),
                event.voice_whole().end(),
            )
            .unwrap();
            gain_ramp = gain_ramp.map(|ramp| ramp.slice_for(event.voice_whole(), entry_span));
        }
    }

    Some(AudioVoicePlan {
        voice_id,
        source: AudioSourcePlan::Synth(SynthSourcePlan {
            source,
            pitch: control_to_scalar(controls.get(&ControlKey::Pitch))
                .unwrap_or(crate::application::synth::SynthTrigger::DEFAULT_PITCH),
        }),
        lifecycle: lifecycle_from_controls(controls, play_for, event),
        envelope: envelope_plan_from_controls(controls),
        mix: mix_plan_from_controls(
            crate::application::synth::SynthTrigger::DEFAULT_GAIN,
            controls,
            velocity,
            gain_ramp,
        ),
        spatial: voice_spatial_from_event(event),
        filters: filter_plan_from_controls(controls),
        sends: send_plan_from_controls(controls),
        dynamics: dynamics_plan_from_controls(controls),
        runtime_controls: runtime_control_state_from_controls(controls),
        modulations: signal_bindings_from_controls(controls, event),
        live_note,
    })
}

/// Modulatable lanes that accept a continuous [`Signal`] in v1.
const MODULATABLE_SIGNAL_LANES: [ControlKey; 3] = [
    ControlKey::Gain,
    ControlKey::PlaybackRate,
    ControlKey::LowPassCutoff,
];

/// Recovers the cycle clock (`start_cycle`, `cps`) for an optional scheduled
/// event. Live voices (no event) use cycle zero at one cycle per second.
fn cycle_clock(event: Option<&ScheduledIntent>) -> (f64, f64) {
    match event {
        Some(event) => {
            let start_cycle = event.voice_whole().start().value();
            let remaining_cycles = event.remaining_duration().value();
            let seconds = event.play_for.as_secs_f64();
            let cps = if seconds > 0.0 && remaining_cycles > 0.0 {
                remaining_cycles / seconds
            } else {
                1.0
            };
            (start_cycle, cps)
        }
        None => (0.0, 1.0),
    }
}

fn voice_spatial_from_event(event: Option<&ScheduledIntent>) -> VoiceSpatial {
    let (start_cycle, cps) = cycle_clock(event);
    let motion = event
        .map(|event| event.projected().position())
        .unwrap_or(SpatialMotion::ORIGIN);
    VoiceSpatial {
        motion,
        start_cycle,
        cps,
    }
}

fn signal_bindings_from_controls(
    controls: &ControlMap,
    event: Option<&ScheduledIntent>,
) -> Vec<SignalBinding> {
    let (start_cycle, cps) = cycle_clock(event);

    MODULATABLE_SIGNAL_LANES
        .iter()
        .filter_map(|key| match controls.get(key) {
            Some(ControlValue::Signal(signal)) => Some(SignalBinding {
                key: key.clone(),
                signal: *signal,
                start_cycle,
                cps,
            }),
            _ => None,
        })
        .collect()
}

fn lifecycle_from_controls(
    controls: &ControlMap,
    play_for: Duration,
    event: Option<&ScheduledIntent>,
) -> VoiceLifecycle {
    let legato = control_to_positive_scalar(controls.get(&ControlKey::Legato)).unwrap_or(1.0);
    let clip_length =
        control_to_non_negative_scalar(controls.get(&ControlKey::ClipLength)).unwrap_or(1.0);

    if let Some(event) = event {
        return VoiceLifecycle {
            gate_duration: Some(scale_duration(
                scale_duration(event.play_for + event.elapsed_duration(), clip_length),
                legato,
            )),
            elapsed: event.elapsed_duration(),
            play_for: scale_duration(event.play_for, clip_length),
        };
    }

    VoiceLifecycle {
        gate_duration: (!play_for.is_zero()).then_some(scale_duration(
            scale_duration(play_for, clip_length),
            legato,
        )),
        elapsed: Duration::ZERO,
        play_for: scale_duration(play_for, clip_length),
    }
}

fn envelope_plan_from_controls(controls: &ControlMap) -> EnvelopePlan {
    EnvelopePlan {
        attack: duration_from_control(controls.get(&ControlKey::Attack)).unwrap_or(Duration::ZERO),
        decay: duration_from_control(controls.get(&ControlKey::Decay)).unwrap_or(Duration::ZERO),
        sustain_level: control_to_unit(controls.get(&ControlKey::Sustain))
            .unwrap_or_else(|| UnitValue::new(1.0).unwrap()),
        release: duration_from_control(controls.get(&ControlKey::Release))
            .unwrap_or(Duration::ZERO),
    }
}

fn mix_plan_from_controls(
    default_gain: f64,
    controls: &ControlMap,
    velocity: Option<UnitValue>,
    gain_ramp: Option<crate::application::sample::SampleGainRamp>,
) -> MixPlan {
    let gain = match controls.get(&ControlKey::Gain) {
        Some(ControlValue::Scalar(value)) => *value,
        Some(ControlValue::Unipolar(value)) => value.value(),
        _ => default_gain,
    };

    MixPlan {
        velocity: control_to_unit(controls.get(&ControlKey::Velocity))
            .or(velocity)
            .unwrap_or_else(|| UnitValue::new(1.0).unwrap()),
        gain,
        gain_ramp,
        post_gain: control_to_non_negative_scalar(controls.get(&ControlKey::PostGain))
            .unwrap_or(crate::application::sample::SampleTrigger::DEFAULT_POST_GAIN),
    }
}

fn filter_plan_from_controls(controls: &ControlMap) -> FilterPlan {
    FilterPlan {
        low_pass_cutoff_hz: control_to_positive_scalar(controls.get(&ControlKey::LowPassCutoff)),
        low_pass_resonance: control_to_unit(controls.get(&ControlKey::LowPassResonance))
            .unwrap_or_else(|| UnitValue::new(0.0).unwrap()),
        high_pass_cutoff_hz: control_to_positive_scalar(controls.get(&ControlKey::HighPassCutoff)),
        high_pass_resonance: control_to_unit(controls.get(&ControlKey::HighPassResonance))
            .unwrap_or_else(|| UnitValue::new(0.0).unwrap()),
    }
}

fn send_plan_from_controls(controls: &ControlMap) -> SendPlan {
    SendPlan {
        reverb: control_to_reverb(controls.get(&ControlKey::ReverbSend)),
        delay: control_to_delay(controls.get(&ControlKey::DelaySend)),
    }
}

fn dynamics_plan_from_controls(controls: &ControlMap) -> DynamicsPlan {
    DynamicsPlan {
        compressor: control_to_compressor(controls.get(&ControlKey::Compressor)),
    }
}

fn runtime_control_state_from_controls(controls: &ControlMap) -> AudioRuntimeControlState {
    let mut state = AudioRuntimeControlState::default();
    AudioRuntimeControlDelta::from_runtime_controls_snapshot(controls).apply_to(&mut state);
    state
}

fn lerp(start: f64, end: f64, progress: f64) -> f64 {
    start + (end - start) * progress
}

fn control_to_scalar(value: Option<&ControlValue>) -> Option<f64> {
    match value {
        Some(ControlValue::Scalar(value)) => Some(*value),
        _ => None,
    }
}

fn control_to_positive_scalar(value: Option<&ControlValue>) -> Option<f64> {
    control_to_scalar(value).filter(|value| value.is_finite() && *value > 0.0)
}

fn control_to_non_negative_scalar(value: Option<&ControlValue>) -> Option<f64> {
    control_to_scalar(value).filter(|value| value.is_finite() && *value >= 0.0)
}

fn control_to_gain_scalar(value: Option<&ControlValue>) -> Option<f64> {
    let scalar = match value? {
        ControlValue::Scalar(value) => *value,
        ControlValue::Unipolar(value) => value.value(),
        ControlValue::Ramp { to, .. } => *to,
        _ => return None,
    };
    (scalar.is_finite() && scalar >= 0.0).then_some(scalar)
}

fn control_to_unit(value: Option<&ControlValue>) -> Option<UnitValue> {
    match value {
        Some(ControlValue::Unipolar(value)) => Some(*value),
        Some(ControlValue::Scalar(value)) => UnitValue::new(*value),
        _ => None,
    }
}

fn control_to_symbol(value: &ControlValue) -> Option<Symbol> {
    match value {
        ControlValue::Choice(symbol) => Some(symbol.clone()),
        _ => None,
    }
}

fn control_to_variant_index(value: Option<&ControlValue>) -> Option<usize> {
    match value {
        Some(ControlValue::Scalar(value)) if value.is_finite() && *value >= 0.0 => {
            Some(value.floor() as usize)
        }
        _ => None,
    }
}

fn duration_from_control(value: Option<&ControlValue>) -> Option<Duration> {
    control_to_non_negative_scalar(value).map(Duration::from_secs_f64)
}

fn scale_duration(duration: Duration, factor: f64) -> Duration {
    if duration.is_zero() {
        return Duration::ZERO;
    }

    Duration::from_secs_f64((duration.as_secs_f64() * factor.max(0.0)).max(0.0))
}

fn control_to_reverb(value: Option<&ControlValue>) -> Option<ReverbSettings> {
    match value {
        Some(ControlValue::Reverb(settings)) => Some(settings.clone()),
        Some(ControlValue::Unipolar(amount)) => Some(ReverbSettings::new(
            *amount,
            Duration::from_secs_f64(2.0),
            UnitValue::new(0.3).unwrap(),
        )),
        Some(ControlValue::Scalar(amount)) => Some(ReverbSettings::new(
            UnitValue::new((*amount).clamp(0.0, 1.0)).unwrap(),
            Duration::from_secs_f64(2.0),
            UnitValue::new(0.3).unwrap(),
        )),
        _ => None,
    }
}

fn control_to_delay(value: Option<&ControlValue>) -> Option<DelaySettings> {
    match value {
        Some(ControlValue::Delay(settings)) => Some(settings.clone()),
        Some(ControlValue::Unipolar(amount)) => Some(DelaySettings::new(
            *amount,
            Duration::from_millis(360),
            UnitValue::new(0.35).unwrap(),
            UnitValue::new(0.2).unwrap(),
        )),
        Some(ControlValue::Scalar(amount)) => Some(DelaySettings::new(
            UnitValue::new((*amount).clamp(0.0, 1.0)).unwrap(),
            Duration::from_millis(360),
            UnitValue::new(0.35).unwrap(),
            UnitValue::new(0.2).unwrap(),
        )),
        _ => None,
    }
}

fn control_to_compressor(value: Option<&ControlValue>) -> Option<CompressorSettings> {
    match value {
        Some(ControlValue::Compressor(settings)) => Some(settings.clone()),
        _ => None,
    }
}

/// Backwards-compatible alias for the audio event sent toward the renderer
/// bridge.
pub type AudioTrigger = ScheduledAudioEvent;

#[derive(Clone)]
/// Sender half of the mixed audio-trigger queue.
pub struct AudioTriggerSender {
    sender: Sender<AudioTrigger>,
}

impl AudioTriggerSender {
    /// Sends one mixed audio trigger.
    pub fn send(&self, trigger: AudioTrigger) -> Result<(), SendError<AudioTrigger>> {
        self.sender.send(trigger)
    }

    /// Creates a performer that reads no ambient controls.
    #[must_use]
    pub fn performer(&self) -> AudioTriggerOutputPerformer {
        AudioTriggerOutputPerformer::new(self.clone(), Arc::new(RwLock::new(ControlMap::new())))
    }

    /// Creates a performer that merges scheduled controls with shared ambient
    /// controls.
    #[must_use]
    pub fn performer_with_ambient(
        &self,
        ambient_controls: Arc<RwLock<ControlMap>>,
    ) -> AudioTriggerOutputPerformer {
        AudioTriggerOutputPerformer::new(self.clone(), ambient_controls)
    }
}

/// Receiver half of the mixed audio-trigger queue.
pub struct AudioTriggerReceiver {
    receiver: Receiver<AudioTrigger>,
}

impl AudioTriggerReceiver {
    /// Attempts to receive one trigger without blocking.
    pub fn try_recv(&self) -> Result<AudioTrigger, TryRecvError> {
        self.receiver.try_recv()
    }

    /// Drains all currently queued triggers.
    #[must_use]
    pub fn drain(&self) -> Vec<AudioTrigger> {
        self.receiver.try_iter().collect()
    }
}

/// Creates a mixed audio-trigger channel.
#[must_use]
pub fn audio_trigger_channel() -> (AudioTriggerSender, AudioTriggerReceiver) {
    let (sender, receiver) = mpsc::channel();

    (
        AudioTriggerSender { sender },
        AudioTriggerReceiver { receiver },
    )
}

/// Queue-backed performer that emits either sample or synth playback commands.
pub struct AudioTriggerOutputPerformer {
    trigger_sender: AudioTriggerSender,
    ambient_controls: Arc<RwLock<ControlMap>>,
}

impl AudioTriggerOutputPerformer {
    /// Creates an output performer backed by a trigger sender and ambient
    /// control store.
    #[must_use]
    pub fn new(
        trigger_sender: AudioTriggerSender,
        ambient_controls: Arc<RwLock<ControlMap>>,
    ) -> Self {
        Self {
            trigger_sender,
            ambient_controls,
        }
    }
}

impl Performer for AudioTriggerOutputPerformer {
    fn perform(&self, event: ScheduledIntent) {
        let ambient_controls = self
            .ambient_controls
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();

        if let Some(trigger) =
            ScheduledAudioEvent::from_scheduled_intent_with_ambient(&event, &ambient_controls)
        {
            let _ = self.trigger_sender.send(trigger);
        }
    }
}

/// Performer adapter that turns scheduled audio intents into audio triggers.
pub struct AudioTriggerPerformer<F> {
    on_trigger: F,
}

impl<F> AudioTriggerPerformer<F> {
    /// Creates a performer that forwards each produced trigger into `on_trigger`.
    #[must_use]
    pub fn new(on_trigger: F) -> Self {
        Self { on_trigger }
    }
}

impl<F> Performer for AudioTriggerPerformer<F>
where
    F: Fn(AudioTrigger),
{
    fn perform(&self, event: ScheduledIntent) {
        if let Some(trigger) =
            ScheduledAudioEvent::from_scheduled_intent_with_ambient(&event, &ControlMap::new())
        {
            (self.on_trigger)(trigger);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc, time::Duration};

    use super::*;
    use crate::{
        application::{
            clock::ClockTime,
            scheduler::events::{IDGenerator, scheduled_intent::ScheduledIntent},
        },
        domain::{
            control::ControlMap,
            input::NoteNumber,
            intent::{BuiltInSynthSource, Intent, SampleIntent},
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
            std::time::Instant::now(),
            play_for,
        )
    }

    #[test]
    fn performer_emits_sample_triggers() {
        let triggers = Rc::new(RefCell::new(Vec::new()));
        let performer = AudioTriggerPerformer::new({
            let triggers = Rc::clone(&triggers);
            move |trigger| triggers.borrow_mut().push(trigger)
        });
        let event = scheduled_intent(Intent::sample("kick"), Duration::from_millis(25));

        performer.perform(event);

        let triggers = triggers.borrow();
        assert_eq!(triggers.len(), 1);
        let AudioTrigger::StartVoice(plan) = &triggers[0] else {
            panic!("expected start-voice event");
        };
        let AudioSourcePlan::Sample(source) = &plan.source else {
            panic!("expected sample source");
        };
        assert_eq!(source.sample, "kick");
        assert_eq!(plan.lifecycle.play_for, Duration::from_millis(25));
    }

    #[test]
    fn performer_emits_synth_triggers() {
        let triggers = Rc::new(RefCell::new(Vec::new()));
        let performer = AudioTriggerPerformer::new({
            let triggers = Rc::clone(&triggers);
            move |trigger| triggers.borrow_mut().push(trigger)
        });
        let event = scheduled_intent(
            Intent::synth(BuiltInSynthSource::Square),
            Duration::from_millis(25),
        );

        performer.perform(event);

        let triggers = triggers.borrow();
        assert_eq!(triggers.len(), 1);
        let AudioTrigger::StartVoice(plan) = &triggers[0] else {
            panic!("expected start-voice event");
        };
        let AudioSourcePlan::Synth(source) = &plan.source else {
            panic!("expected synth source");
        };
        assert_eq!(source.source, BuiltInSynthSource::Square);
        assert_eq!(
            source.pitch,
            crate::application::synth::SynthTrigger::DEFAULT_PITCH
        );
        assert_eq!(plan.lifecycle.play_for, Duration::from_millis(25));
    }

    #[test]
    fn output_performer_sends_triggers_into_receiver() {
        let (sender, receiver) = audio_trigger_channel();
        let performer = sender.performer();

        performer.perform(scheduled_intent(
            Intent::sample("kick"),
            Duration::from_millis(10),
        ));
        performer.perform(scheduled_intent(
            Intent::synth(BuiltInSynthSource::Sine),
            Duration::from_millis(20),
        ));

        let triggers = receiver.drain();
        assert_eq!(triggers.len(), 2);
        assert!(
            matches!(&triggers[0], AudioTrigger::StartVoice(plan) if matches!(&plan.source, AudioSourcePlan::Sample(source) if source.sample == "kick"))
        );
        assert!(
            matches!(&triggers[1], AudioTrigger::StartVoice(plan) if matches!(&plan.source, AudioSourcePlan::Synth(source) if source.source == BuiltInSynthSource::Sine))
        );
    }

    #[test]
    fn receiver_drain_preserves_trigger_order() {
        let (sender, receiver) = audio_trigger_channel();

        let sample_plan = AudioVoicePlan::from_live_sample(
            VoiceInstanceId::new(7),
            &SampleIntent::new("kick"),
            &ControlMap::new(),
            None,
            NoteNumber::new(60),
            Duration::from_millis(10),
        )
        .unwrap();
        let synth_plan = AudioVoicePlan::from_live_synth(
            VoiceInstanceId::new(8),
            BuiltInSynthSource::Triangle,
            &ControlMap::new(),
            None,
            NoteNumber::new(64),
            Duration::from_millis(20),
        )
        .unwrap();

        sender.send(AudioTrigger::StartVoice(sample_plan)).unwrap();
        sender.send(AudioTrigger::StartVoice(synth_plan)).unwrap();

        let triggers = receiver.drain();

        assert_eq!(triggers.len(), 2);
        assert!(
            matches!(&triggers[0], AudioTrigger::StartVoice(plan) if matches!(&plan.source, AudioSourcePlan::Sample(source) if source.sample == "kick"))
        );
        assert!(
            matches!(&triggers[1], AudioTrigger::StartVoice(plan) if matches!(&plan.source, AudioSourcePlan::Synth(source) if source.source == BuiltInSynthSource::Triangle))
        );
    }

    #[test]
    fn shared_subsystem_controls_lower_identically_for_sample_and_synth() {
        let mut controls = ControlMap::new();
        controls.insert(ControlKey::Pitch, ControlValue::Scalar(64.0));
        controls.insert(
            ControlKey::Velocity,
            ControlValue::Unipolar(UnitValue::new(0.8).unwrap()),
        );
        controls.insert(ControlKey::Attack, ControlValue::Scalar(0.01));
        controls.insert(ControlKey::Decay, ControlValue::Scalar(0.02));
        controls.insert(
            ControlKey::Sustain,
            ControlValue::Unipolar(UnitValue::new(0.4).unwrap()),
        );
        controls.insert(ControlKey::Release, ControlValue::Scalar(0.03));
        controls.insert(ControlKey::Gain, ControlValue::Scalar(0.6));
        controls.insert(ControlKey::PostGain, ControlValue::Scalar(1.2));
        controls.insert(ControlKey::LowPassCutoff, ControlValue::Scalar(1_400.0));
        controls.insert(
            ControlKey::LowPassResonance,
            ControlValue::Unipolar(UnitValue::new(0.2).unwrap()),
        );
        controls.insert(ControlKey::HighPassCutoff, ControlValue::Scalar(180.0));
        controls.insert(
            ControlKey::HighPassResonance,
            ControlValue::Unipolar(UnitValue::new(0.1).unwrap()),
        );
        controls.insert(
            ControlKey::ReverbSend,
            ControlValue::Unipolar(UnitValue::new(0.3).unwrap()),
        );
        controls.insert(
            ControlKey::DelaySend,
            ControlValue::Unipolar(UnitValue::new(0.2).unwrap()),
        );
        controls.insert(
            ControlKey::Compressor,
            ControlValue::Compressor(crate::domain::control::CompressorSettings::new(
                UnitValue::new(0.25).unwrap(),
                4.0,
                Duration::from_millis(5),
                Duration::from_millis(50),
            )),
        );
        controls.insert(
            ControlKey::Expression,
            ControlValue::Unipolar(UnitValue::new(0.7).unwrap()),
        );

        let sample_plan = AudioVoicePlan::from_live_sample(
            VoiceInstanceId::new(10),
            &SampleIntent::new("pad"),
            &controls,
            None,
            Some(NoteNumber::new(60).unwrap()),
            Duration::from_millis(250),
        )
        .unwrap();
        let synth_plan = AudioVoicePlan::from_live_synth(
            VoiceInstanceId::new(11),
            BuiltInSynthSource::Triangle,
            &controls,
            None,
            Some(NoteNumber::new(60).unwrap()),
            Duration::from_millis(250),
        )
        .unwrap();

        assert_eq!(sample_plan.lifecycle, synth_plan.lifecycle);
        assert_eq!(sample_plan.envelope, synth_plan.envelope);
        assert_eq!(sample_plan.mix, synth_plan.mix);
        assert_eq!(sample_plan.filters, synth_plan.filters);
        assert_eq!(sample_plan.sends, synth_plan.sends);
        assert_eq!(sample_plan.dynamics, synth_plan.dynamics);
        assert_eq!(sample_plan.runtime_controls, synth_plan.runtime_controls);
    }

    #[test]
    fn live_sample_plan_carries_sample_variant_and_clip_length() {
        let mut controls = ControlMap::new();
        controls.insert(ControlKey::SampleVariant, ControlValue::Scalar(2.0));
        controls.insert(ControlKey::ClipLength, ControlValue::Scalar(1.5));

        let plan = AudioVoicePlan::from_live_sample(
            VoiceInstanceId::new(12),
            &SampleIntent::new("kalimba"),
            &controls,
            None,
            Some(NoteNumber::new(60).unwrap()),
            Duration::from_millis(200),
        )
        .unwrap();

        let AudioSourcePlan::Sample(source) = &plan.source else {
            panic!("expected sample source");
        };
        assert_eq!(source.sample_variant, Some(2));
        assert_eq!(plan.lifecycle.play_for, Duration::from_millis(300));
        assert_eq!(
            plan.lifecycle.gate_duration,
            Some(Duration::from_millis(300))
        );
    }

    #[test]
    fn runtime_controls_snapshot_maps_post_gain_and_gain() {
        let mut controls = ControlMap::new();
        controls.insert(ControlKey::PostGain, ControlValue::Scalar(0.75));
        controls.insert(ControlKey::Gain, ControlValue::Ramp { from: 1.0, to: 0.5 });

        let delta = AudioRuntimeControlDelta::from_runtime_controls_snapshot(&controls);

        assert!(
            matches!(delta.post_gain, RuntimeControlValue::Set(v) if (v - 0.75).abs() < f32::EPSILON)
        );
        assert!(
            matches!(delta.gain, RuntimeControlValue::Set(v) if (v - 0.5).abs() < f32::EPSILON)
        );
        assert!(matches!(
            delta.pitch_bend_semitones,
            RuntimeControlValue::Keep
        ));
    }

    #[test]
    fn live_post_gain_maps_to_runtime_delta() {
        let delta = AudioRuntimeControlDelta::from_live_control(
            &ControlKey::PostGain,
            &ControlValue::Scalar(0.5),
        )
        .expect("post gain is a live runtime control");

        assert!(
            matches!(delta.post_gain, RuntimeControlValue::Set(v) if (v - 0.5).abs() < f32::EPSILON)
        );
    }
}
