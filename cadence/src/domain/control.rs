//! Typed control keys, values, and repeating control tracks.

use std::collections::BTreeMap;
use std::time::Duration;

use thiserror::Error;

use crate::domain::{
    prelude::Time,
    signal::Signal,
    span::{Phase, Span},
    voice::Repeat,
};

fn assert_finite(label: &str, value: f64) {
    assert!(value.is_finite(), "{label} must be finite");
}

fn assert_positive(label: &str, value: f64) {
    assert!(
        value.is_finite() && value > 0.0,
        "{label} must be finite and > 0.0"
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// High-level value kind expected by one control lane.
pub enum ControlValueKind {
    /// Boolean on/off value.
    Bool,
    /// Finite scalar value.
    Scalar,
    /// Unit-range scalar value.
    Unipolar,
    /// Signed unit-range scalar value.
    Bipolar,
    /// Named symbolic choice.
    Choice,
    /// Continuous signal source evaluated per audio frame.
    Signal,
    /// Reverb-send settings.
    ReverbSettings,
    /// Delay-send settings.
    DelaySettings,
    /// Compressor settings.
    CompressorSettings,
    /// Free-form custom shape.
    Custom,
}

impl ControlValueKind {
    /// Returns a human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Scalar => "scalar",
            Self::Unipolar => "unipolar",
            Self::Bipolar => "bipolar",
            Self::Choice => "choice",
            Self::Signal => "signal",
            Self::ReverbSettings => "reverb_settings",
            Self::DelaySettings => "delay_settings",
            Self::CompressorSettings => "compressor_settings",
            Self::Custom => "custom",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// When one control is evaluated relative to voice playback.
pub enum ControlTiming {
    /// Read once when an event begins.
    Onset,
    /// Read once into a voice-owned lifecycle subsystem.
    VoiceLifecycle,
    /// Re-sampled when projection slices the event into segments.
    SegmentSampled,
    /// Evaluated continuously while the voice is running.
    ContinuousRuntime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// How multiple values for the same control should compose.
pub enum ControlMerge {
    /// Values multiply together.
    Multiply,
    /// Values sum together.
    Add,
    /// Exactly one value may be active.
    Override,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Which source/runtime paths a control is valid for.
pub enum ControlSupport {
    /// Works on both sample and synth sources.
    Shared,
    /// Works only on sample-backed playback.
    SampleOnly,
    /// Works only on synth-backed playback.
    SynthOnly,
    /// Valid only for live input/runtime state.
    LiveInputOnly,
}

impl ControlSupport {
    /// Returns whether this control may be applied to sample playback.
    #[must_use]
    pub fn supports_sample(self) -> bool {
        matches!(self, Self::Shared | Self::SampleOnly)
    }

    /// Returns whether this control may be applied to synth playback.
    #[must_use]
    pub fn supports_synth(self) -> bool {
        matches!(self, Self::Shared | Self::SynthOnly)
    }

    /// Returns whether this control may be used as a live-input state control.
    #[must_use]
    pub fn supports_live_input(self) -> bool {
        matches!(self, Self::Shared | Self::LiveInputOnly)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// First-class metadata describing one control lane.
pub struct ControlSpec {
    canonical_name: &'static str,
    value_kind: ControlValueKind,
    timing: ControlTiming,
    merge: ControlMerge,
    support: ControlSupport,
    rampable: bool,
}

impl ControlSpec {
    /// Creates a control spec.
    pub const fn new(
        canonical_name: &'static str,
        value_kind: ControlValueKind,
        timing: ControlTiming,
        merge: ControlMerge,
        support: ControlSupport,
        rampable: bool,
    ) -> Self {
        Self {
            canonical_name,
            value_kind,
            timing,
            merge,
            support,
            rampable,
        }
    }

    /// Returns the canonical public name.
    #[must_use]
    pub fn canonical_name(self) -> &'static str {
        self.canonical_name
    }

    /// Returns the high-level value kind.
    #[must_use]
    pub fn value_kind(self) -> ControlValueKind {
        self.value_kind
    }

    /// Returns when the control is evaluated.
    #[must_use]
    pub fn timing(self) -> ControlTiming {
        self.timing
    }

    /// Returns how overlapping controls should compose.
    #[must_use]
    pub fn merge(self) -> ControlMerge {
        self.merge
    }

    /// Returns which source/runtime paths support the control.
    #[must_use]
    pub fn support(self) -> ControlSupport {
        self.support
    }

    /// Returns whether the control may use ramp values.
    #[must_use]
    pub fn rampable(self) -> bool {
        self.rampable
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
/// Typed model errors for control construction, validation, and resolution.
pub enum ControlModelError {
    /// A host pattern source rejected a query or returned invalid source data.
    #[error("pattern query failed: {0}")]
    QuerySource(String),
    /// A timed arrangement query exceeds bounded iteration or exact-time limits.
    #[error("arrangement query exceeds its work or time bounds")]
    ArrangementQueryLimit,
    /// One control name is not part of the current core model.
    #[error("unknown control name `{0}`")]
    UnknownControlName(String),
    /// A control value shape does not match the lane.
    #[error("control `{key}` does not accept value kind `{found}`")]
    InvalidControlValue {
        /// Canonical control name.
        key: &'static str,
        /// Actual value kind label.
        found: &'static str,
    },
    /// One control value violates a scalar/range invariant.
    #[error("control `{key}` is invalid: {reason}")]
    InvalidControlValueRange {
        /// Canonical control name.
        key: &'static str,
        /// Human-readable reason.
        reason: &'static str,
    },
    /// A control was used against the wrong source/runtime path.
    #[error("control `{key}` is not supported for {source_kind}")]
    UnsupportedControlForSource {
        /// Canonical control name.
        key: &'static str,
        /// Source/runtime label.
        source_kind: &'static str,
    },
    /// Multiple override-style controls overlap on the same source segment.
    #[error("control `{key}` has conflicting overlapping override values")]
    ConflictingOverrideControls {
        /// Canonical control name.
        key: &'static str,
    },
    /// Control-tile phase starts before zero.
    #[error("control tile phase must start at or after zero")]
    InvalidTilePhaseStart,
    /// Control-track period must be positive.
    #[error("control track period must be positive")]
    InvalidTrackPeriod,
    /// One control tile lies outside its containing period.
    #[error("control tile phase must stay within the control-track period")]
    TileOutsideTrackPeriod,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Small symbolic string used for named control targets and options.
pub struct Symbol(String);

impl Symbol {
    /// Creates a symbol from owned or borrowed text.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Symbol {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for Symbol {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Finite scalar constrained to the inclusive range `[0.0, 1.0]`.
pub struct UnitValue(f64);
impl Eq for UnitValue {}

impl UnitValue {
    /// Creates a unit value.
    ///
    /// Returns `None` when `value` is not finite or lies outside `[0.0, 1.0]`.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && (0.0..=1.0).contains(&value)).then_some(Self(value))
    }

    /// Returns the raw scalar value.
    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Finite scalar constrained to the inclusive range `[-1.0, 1.0]`.
pub struct SignedUnitValue(f64);
impl Eq for SignedUnitValue {}

impl SignedUnitValue {
    /// Creates a signed unit value.
    ///
    /// Returns `None` when `value` is not finite or lies outside
    /// `[-1.0, 1.0]`.
    #[must_use]
    pub fn new(value: f64) -> Option<Self> {
        (value.is_finite() && (-1.0..=1.0).contains(&value)).then_some(Self(value))
    }

    /// Returns the raw scalar value.
    #[must_use]
    pub fn value(self) -> f64 {
        self.0
    }
}

/// An additive pan sum. Authored offsets are bipolar; nested sums remain exact
/// until stereo rendering clamps the final position to [-1, 1].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PanControl(f64);
impl Eq for PanControl {}

impl PanControl {
    /// Creates a pan offset from a validated bipolar value.
    pub fn new(value: SignedUnitValue) -> Self {
        Self(value.value())
    }

    /// Returns the combined offset before the renderer clamps it.
    pub fn value(self) -> f64 {
        self.0
    }

    pub(crate) fn add(self, other: Self) -> Result<Self, ControlModelError> {
        let value = self.0 + other.0;
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(ControlModelError::InvalidControlValueRange {
                key: "pan",
                reason: "combined pan must be finite",
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Canonical control lanes understood by the runtime.
pub enum ControlKey {
    /// Boolean gate open/closed state.
    Gate,
    /// Sample-bank selection.
    SampleBank,
    /// Explicit sample-variant selection.
    SampleVariant,
    /// Absolute musical pitch in MIDI-note space.
    Pitch,
    /// Additive transposition in signed semitones, independent of base pitch.
    Transpose,
    /// Note velocity in unit range.
    Velocity,
    /// Legato amount or ratio.
    Legato,
    /// Envelope attack time.
    Attack,
    /// Envelope decay time.
    Decay,
    /// Envelope sustain level.
    Sustain,
    /// Envelope release time.
    Release,
    /// Linear gain amount.
    Gain,
    /// Playback rate multiplier.
    PlaybackRate,
    /// Event-relative clip-length multiplier.
    ClipLength,
    /// Unit-range playback start position.
    PlaybackStart,
    /// Unit-range playback end position.
    PlaybackEnd,
    /// Reverse playback toggle.
    Reverse,
    /// Fit the selected source region to its authored slot by changing rate.
    /// Explicit rate and pitch offsets are applied afterward and can change its duration.
    Fit,
    /// Repeat the selected source region until its note envelope ends.
    Loop,
    /// Low-pass cutoff frequency.
    LowPassCutoff,
    /// Low-pass resonance / Q.
    LowPassResonance,
    /// High-pass cutoff frequency.
    HighPassCutoff,
    /// High-pass resonance / Q.
    HighPassResonance,
    /// Post-effect gain.
    PostGain,
    /// Additive stereo pan offset, independent of spatial distance.
    Pan,
    /// Reverb send.
    ReverbSend,
    /// Delay send.
    DelaySend,
    /// Compressor settings.
    Compressor,
    /// Named selector lane.
    Select(Symbol),
    /// Normalized live pitch bend in [-1, 1], currently mapped to +/-2 semitones.
    PitchBend,
    /// Mod-wheel amount.
    ModWheel,
    /// Expression amount.
    Expression,
    /// Sustain-pedal state.
    SustainPedal,
    /// Host-defined control lane.
    Custom(Symbol),
}

impl ControlKey {
    /// Returns the first-class metadata for this control lane.
    #[must_use]
    pub fn spec(&self) -> ControlSpec {
        match self {
            Self::Gate => ControlSpec::new(
                "gate",
                ControlValueKind::Bool,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::SampleBank => ControlSpec::new(
                "sample_bank",
                ControlValueKind::Choice,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::SampleOnly,
                false,
            ),
            Self::SampleVariant => ControlSpec::new(
                "sample_variant",
                ControlValueKind::Scalar,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::SampleOnly,
                false,
            ),
            Self::Pitch => ControlSpec::new(
                "pitch",
                ControlValueKind::Scalar,
                ControlTiming::Onset,
                ControlMerge::Add,
                ControlSupport::Shared,
                false,
            ),
            Self::Transpose => ControlSpec::new(
                "transpose",
                ControlValueKind::Scalar,
                ControlTiming::ContinuousRuntime,
                ControlMerge::Add,
                ControlSupport::Shared,
                true,
            ),
            Self::Velocity => ControlSpec::new(
                "velocity",
                ControlValueKind::Unipolar,
                ControlTiming::Onset,
                ControlMerge::Multiply,
                ControlSupport::Shared,
                false,
            ),
            Self::Legato => ControlSpec::new(
                "legato",
                ControlValueKind::Scalar,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::Attack => ControlSpec::new(
                "attack",
                ControlValueKind::Scalar,
                ControlTiming::VoiceLifecycle,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::Decay => ControlSpec::new(
                "decay",
                ControlValueKind::Scalar,
                ControlTiming::VoiceLifecycle,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::Sustain => ControlSpec::new(
                "sustain",
                ControlValueKind::Unipolar,
                ControlTiming::VoiceLifecycle,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::Release => ControlSpec::new(
                "release",
                ControlValueKind::Scalar,
                ControlTiming::VoiceLifecycle,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::Gain => ControlSpec::new(
                "gain",
                ControlValueKind::Scalar,
                ControlTiming::SegmentSampled,
                ControlMerge::Multiply,
                ControlSupport::Shared,
                true,
            ),
            Self::PlaybackRate => ControlSpec::new(
                "playback_rate",
                ControlValueKind::Scalar,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::SampleOnly,
                false,
            ),
            Self::ClipLength => ControlSpec::new(
                "clip_length",
                ControlValueKind::Scalar,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::PlaybackStart => ControlSpec::new(
                "playback_start",
                ControlValueKind::Scalar,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::SampleOnly,
                false,
            ),
            Self::PlaybackEnd => ControlSpec::new(
                "playback_end",
                ControlValueKind::Scalar,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::SampleOnly,
                false,
            ),
            Self::Reverse => ControlSpec::new(
                "reverse",
                ControlValueKind::Bool,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::SampleOnly,
                false,
            ),
            Self::Fit => ControlSpec::new(
                "fit",
                ControlValueKind::Bool,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::SampleOnly,
                false,
            ),
            Self::Loop => ControlSpec::new(
                "loop",
                ControlValueKind::Bool,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::SampleOnly,
                false,
            ),
            Self::LowPassCutoff => ControlSpec::new(
                "lowpass_cutoff",
                ControlValueKind::Scalar,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::LowPassResonance => ControlSpec::new(
                "lowpass_resonance",
                ControlValueKind::Scalar,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::HighPassCutoff => ControlSpec::new(
                "highpass_cutoff",
                ControlValueKind::Scalar,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::HighPassResonance => ControlSpec::new(
                "highpass_resonance",
                ControlValueKind::Scalar,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::PostGain => ControlSpec::new(
                "post_gain",
                ControlValueKind::Scalar,
                ControlTiming::ContinuousRuntime,
                ControlMerge::Multiply,
                ControlSupport::Shared,
                false,
            ),
            Self::Pan => ControlSpec::new(
                "pan",
                ControlValueKind::Bipolar,
                ControlTiming::ContinuousRuntime,
                ControlMerge::Add,
                ControlSupport::Shared,
                false,
            ),
            Self::ReverbSend => ControlSpec::new(
                "reverb_send",
                ControlValueKind::ReverbSettings,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::DelaySend => ControlSpec::new(
                "delay_send",
                ControlValueKind::DelaySettings,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::Compressor => ControlSpec::new(
                "compressor",
                ControlValueKind::CompressorSettings,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::Select(_) => ControlSpec::new(
                "select",
                ControlValueKind::Choice,
                ControlTiming::Onset,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
            Self::PitchBend => ControlSpec::new(
                "pitch_bend",
                ControlValueKind::Bipolar,
                ControlTiming::ContinuousRuntime,
                ControlMerge::Add,
                ControlSupport::Shared,
                false,
            ),
            Self::ModWheel => ControlSpec::new(
                "mod_wheel",
                ControlValueKind::Unipolar,
                ControlTiming::ContinuousRuntime,
                ControlMerge::Override,
                ControlSupport::LiveInputOnly,
                false,
            ),
            Self::Expression => ControlSpec::new(
                "expression",
                ControlValueKind::Unipolar,
                ControlTiming::ContinuousRuntime,
                ControlMerge::Multiply,
                ControlSupport::Shared,
                false,
            ),
            Self::SustainPedal => ControlSpec::new(
                "sustain_pedal",
                ControlValueKind::Bool,
                ControlTiming::ContinuousRuntime,
                ControlMerge::Override,
                ControlSupport::LiveInputOnly,
                false,
            ),
            Self::Custom(_) => ControlSpec::new(
                "custom",
                ControlValueKind::Custom,
                ControlTiming::SegmentSampled,
                ControlMerge::Override,
                ControlSupport::Shared,
                false,
            ),
        }
    }

    /// Returns the canonical public name.
    #[must_use]
    pub fn canonical_name(&self) -> &'static str {
        self.spec().canonical_name()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Reverb-style send settings.
pub struct ReverbSettings {
    amount: UnitValue,
    decay: Duration,
    damping: UnitValue,
}

impl ReverbSettings {
    /// Creates reverb settings.
    #[must_use]
    pub fn new(amount: UnitValue, decay: Duration, damping: UnitValue) -> Self {
        assert!(!decay.is_zero(), "reverb decay must be > 0");
        Self {
            amount,
            decay,
            damping,
        }
    }

    /// Returns the wet-send amount.
    #[must_use]
    pub fn amount(&self) -> UnitValue {
        self.amount
    }

    /// Returns the reverb decay time.
    #[must_use]
    pub fn decay(&self) -> Duration {
        self.decay
    }

    /// Returns the high-frequency damping amount.
    #[must_use]
    pub fn damping(&self) -> UnitValue {
        self.damping
    }
}
impl Eq for ReverbSettings {}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Delay-send settings.
pub struct DelaySettings {
    amount: UnitValue,
    time: Duration,
    feedback: UnitValue,
    damping: UnitValue,
}

impl DelaySettings {
    /// Creates delay settings.
    ///
    /// # Panics
    ///
    /// Panics if `time` is zero.
    #[must_use]
    pub fn new(amount: UnitValue, time: Duration, feedback: UnitValue, damping: UnitValue) -> Self {
        assert!(!time.is_zero(), "delay time must be > 0");

        Self {
            amount,
            time,
            feedback,
            damping,
        }
    }

    /// Returns the wet-send amount.
    #[must_use]
    pub fn amount(&self) -> UnitValue {
        self.amount
    }

    /// Returns the delay time.
    #[must_use]
    pub fn time(&self) -> Duration {
        self.time
    }

    /// Returns the feedback amount.
    #[must_use]
    pub fn feedback(&self) -> UnitValue {
        self.feedback
    }

    /// Returns the damping amount.
    #[must_use]
    pub fn damping(&self) -> UnitValue {
        self.damping
    }
}
impl Eq for DelaySettings {}

#[derive(Debug, Clone, Copy, PartialEq)]
/// Compressor settings.
pub struct CompressorSettings {
    threshold: UnitValue,
    ratio: f64,
    knee_db: f64,
    attack: Duration,
    release: Duration,
}

impl CompressorSettings {
    /// Creates compressor settings.
    ///
    /// # Panics
    ///
    /// Panics if `ratio <= 0`, `attack == 0`, or `release == 0`.
    #[must_use]
    pub fn new(threshold: UnitValue, ratio: f64, attack: Duration, release: Duration) -> Self {
        assert_positive("compressor ratio", ratio);
        assert!(!attack.is_zero(), "compressor attack must be > 0");
        assert!(!release.is_zero(), "compressor release must be > 0");

        Self {
            threshold,
            ratio,
            knee_db: 0.0,
            attack,
            release,
        }
    }

    /// Sets the width of the soft transition, in decibels (0 is a hard knee).
    /// Panics unless the width is finite and within 0..=40 dB.
    #[must_use]
    pub fn with_knee_db(mut self, knee_db: f64) -> Self {
        assert!(
            knee_db.is_finite() && (0.0..=40.0).contains(&knee_db),
            "compressor knee must be 0 to 40 dB"
        );
        self.knee_db = knee_db;
        self
    }

    /// Returns the transition width in decibels.
    #[must_use]
    pub fn knee_db(&self) -> f64 {
        self.knee_db
    }

    /// Returns the compression threshold.
    #[must_use]
    pub fn threshold(&self) -> UnitValue {
        self.threshold
    }

    /// Returns the compression ratio.
    #[must_use]
    pub fn ratio(&self) -> f64 {
        self.ratio
    }

    /// Returns the attack time.
    #[must_use]
    pub fn attack(&self) -> Duration {
        self.attack
    }

    /// Returns the release time.
    #[must_use]
    pub fn release(&self) -> Duration {
        self.release
    }
}
impl Eq for CompressorSettings {}

/// Bounded additive transposition descriptor evaluated in musical cycle time.
/// Scalars, ramps and up to eight signal components retain their shared owner.
#[derive(Debug, Clone, PartialEq)]
pub struct SemitoneControl {
    offset: f64,
    signals: Vec<Signal>,
}
impl Default for SemitoneControl {
    fn default() -> Self {
        Self {
            offset: 0.0,
            signals: Vec::new(),
        }
    }
}
impl SemitoneControl {
    /// Moves this control to a later occurrence, preserving local signal time.
    pub(crate) fn shifted(mut self, offset: Time) -> Self {
        for signal in &mut self.signals {
            *signal = signal.with_phase(signal.phase() - signal.rate() * offset);
        }
        self
    }
    /// Creates a signed semitone offset in the supported [-127, 127] range.
    pub fn constant(offset: f64) -> Result<Self, ControlModelError> {
        let control = Self {
            offset,
            ..Self::default()
        };
        control.validate()?;
        Ok(control)
    }
    /// Creates a continuous semitone modulation.
    pub fn signal(signal: Signal) -> Result<Self, ControlModelError> {
        let control = Self {
            signals: vec![signal],
            ..Self::default()
        };
        control.validate()?;
        Ok(control)
    }
    /// Adds controls while retaining each continuous component.
    pub fn combine(self, other: Self) -> Result<Self, ControlModelError> {
        let mut result = self;
        result.offset += other.offset;
        if result.signals.len() + other.signals.len() > 8 {
            return Err(ControlModelError::InvalidControlValueRange {
                key: "transpose",
                reason: "transpose supports at most eight simultaneous signal components",
            });
        }
        result.signals.extend(other.signals);
        result.validate()?;
        Ok(result)
    }
    /// Resolves the offset at an absolute transport cycle, without allocating.
    pub fn eval(&self, cycle: f64) -> f64 {
        self.offset
            + self
                .signals
                .iter()
                .map(|signal| signal.eval(cycle))
                .sum::<f64>()
    }
    /// Checks the complete range before audio rendering.
    pub fn validate(&self) -> Result<(), ControlModelError> {
        let mut low = self.offset;
        let mut high = self.offset;
        for signal in &self.signals {
            if !signal.bias().is_finite() || !signal.depth().is_finite() {
                return Err(ControlModelError::InvalidControlValueRange {
                    key: "transpose",
                    reason: "transpose signal bias and depth must be finite",
                });
            }
            let (a, b) = if signal.waveform() == crate::domain::signal::Waveform::Ramp {
                (signal.bias(), signal.bias() + signal.depth())
            } else {
                (
                    signal.bias() - signal.depth().abs(),
                    signal.bias() + signal.depth().abs(),
                )
            };
            low += a.min(b);
            high += a.max(b);
        }
        if !low.is_finite() || !high.is_finite() || low < -127.0 || high > 127.0 {
            return Err(ControlModelError::InvalidControlValueRange {
                key: "transpose",
                reason: "combined transpose must stay finite and within [-127, 127] semitones",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Runtime value carried by a control lane.
pub enum ControlValue {
    /// Boolean on/off value.
    Bool(bool),
    /// Arbitrary finite scalar value.
    Scalar(f64),
    /// Linear ramp from one scalar to another.
    Ramp {
        /// Ramp start value.
        from: f64,
        /// Ramp end value.
        to: f64,
    },
    /// Unit-range scalar.
    Unipolar(UnitValue),
    /// Signed unit-range scalar.
    Bipolar(SignedUnitValue),
    /// Named symbolic choice.
    Choice(Symbol),
    /// Signal evaluated at note onset for Velocity, or per audio frame for
    /// continuous controls.
    ///
    /// Signal-valued lanes are attached once to a projected moment (like an
    /// attach-once value) and are not boundary-sliced during projection.
    Signal(Signal),
    /// Resolved additive semitone control used by the Transpose lane.
    Semitones(SemitoneControl),
    /// Resolved additive stereo pan, clamped only by the renderer.
    Pan(PanControl),
    /// Reverb-send settings.
    Reverb(ReverbSettings),
    /// Delay-send settings.
    Delay(DelaySettings),
    /// Compressor settings.
    Compressor(CompressorSettings),
}
impl Eq for ControlValue {}

impl ControlValue {
    /// Returns the concrete value kind label.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Scalar(_) => "scalar",
            Self::Ramp { .. } => "ramp",
            Self::Unipolar(_) => "unipolar",
            Self::Bipolar(_) => "bipolar",
            Self::Choice(_) => "choice",
            Self::Signal(_) => "signal",
            Self::Semitones(_) => "semitones",
            Self::Pan(_) => "pan",
            Self::Reverb(_) => "reverb_settings",
            Self::Delay(_) => "delay_settings",
            Self::Compressor(_) => "compressor_settings",
        }
    }

    /// Validates that this value shape matches the given key.
    pub fn validate_for(&self, key: &ControlKey) -> Result<(), ControlModelError> {
        match (key, self) {
            (
                ControlKey::Gate
                | ControlKey::Reverse
                | ControlKey::SustainPedal
                | ControlKey::Fit
                | ControlKey::Loop,
                Self::Bool(_),
            ) => Ok(()),
            (ControlKey::SampleBank, Self::Choice(_)) => Ok(()),
            (ControlKey::Pitch, Self::Scalar(value)) => {
                assert_finite("pitch control", *value);
                Ok(())
            }
            (ControlKey::Transpose, Self::Scalar(value)) => {
                SemitoneControl::constant(*value).map(|_| ())
            }
            (ControlKey::Transpose, Self::Ramp { from, to }) => {
                SemitoneControl::constant(*from)?;
                SemitoneControl::constant(*to)?;
                Ok(())
            }
            (ControlKey::Transpose, Self::Signal(signal)) => {
                SemitoneControl::signal(*signal).map(|_| ())
            }
            (ControlKey::Transpose, Self::Semitones(control)) => control.validate(),
            (ControlKey::Pan, Self::Bipolar(_) | Self::Pan(_)) => Ok(()),
            (ControlKey::Velocity, Self::Unipolar(_)) => Ok(()),
            (ControlKey::Velocity, Self::Signal(signal)) => {
                let (low, high) = if signal.waveform() == crate::domain::signal::Waveform::Ramp {
                    let a = signal.bias();
                    let b = a + signal.depth();
                    (a.min(b), a.max(b))
                } else {
                    (
                        signal.bias() - signal.depth().abs(),
                        signal.bias() + signal.depth().abs(),
                    )
                };
                if signal.bias().is_finite()
                    && signal.depth().is_finite()
                    && low >= 0.0
                    && high <= 1.0
                {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "velocity signal must stay finite and within [0.0, 1.0]",
                    })
                }
            }
            (ControlKey::Velocity, Self::Scalar(value)) => {
                if value.is_finite() && (0.0..=1.0).contains(value) {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "velocity must be finite and within [0.0, 1.0]",
                    })
                }
            }
            (ControlKey::Legato, Self::Scalar(value)) => {
                if value.is_finite() && *value > 0.0 {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "legato must be finite and > 0.0",
                    })
                }
            }
            (ControlKey::Attack | ControlKey::Decay | ControlKey::Release, Self::Scalar(value)) => {
                if value.is_finite() && *value >= 0.0 {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "envelope time must be finite and >= 0.0",
                    })
                }
            }
            (ControlKey::Sustain, Self::Unipolar(_)) => Ok(()),
            (ControlKey::Sustain, Self::Scalar(value)) => {
                if value.is_finite() && (0.0..=1.0).contains(value) {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "sustain must be finite and within [0.0, 1.0]",
                    })
                }
            }
            (ControlKey::Gain, Self::Scalar(value)) => {
                if value.is_finite() && *value >= 0.0 {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "gain must be finite and >= 0.0",
                    })
                }
            }
            (ControlKey::Gain, Self::Ramp { from, to }) => {
                if from.is_finite() && *from >= 0.0 && to.is_finite() && *to >= 0.0 {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "gain ramp endpoints must be finite and >= 0.0",
                    })
                }
            }
            (ControlKey::Gain, Self::Unipolar(_)) => Ok(()),
            (ControlKey::PlaybackRate, Self::Scalar(value)) => {
                if value.is_finite() && *value > 0.0 && *value <= 65_536.0 {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "playback rate must be finite, positive and at most 65536",
                    })
                }
            }
            (ControlKey::SampleVariant | ControlKey::ClipLength, Self::Scalar(value)) => {
                if value.is_finite() && *value >= 0.0 {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "clip and variant values must be finite and >= 0.0",
                    })
                }
            }
            (ControlKey::PlaybackStart | ControlKey::PlaybackEnd, Self::Scalar(value)) => {
                if value.is_finite() && (0.0..=1.0).contains(value) {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "playback positions must be finite and within [0.0, 1.0]",
                    })
                }
            }
            (ControlKey::LowPassCutoff | ControlKey::HighPassCutoff, Self::Scalar(value)) => {
                if value.is_finite() && *value > 0.0 {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "filter cutoff must be finite and > 0.0",
                    })
                }
            }
            (ControlKey::LowPassResonance | ControlKey::HighPassResonance, Self::Unipolar(_)) => {
                Ok(())
            }
            (ControlKey::LowPassResonance | ControlKey::HighPassResonance, Self::Scalar(value)) => {
                if value.is_finite() && (0.0..=1.0).contains(value) {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "filter resonance must be finite and within [0.0, 1.0]",
                    })
                }
            }
            (ControlKey::PostGain, Self::Scalar(value)) => {
                if value.is_finite() && *value >= 0.0 {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "post_gain must be finite and >= 0.0",
                    })
                }
            }
            (ControlKey::ReverbSend, Self::Unipolar(_)) => Ok(()),
            (ControlKey::ReverbSend, Self::Scalar(value)) => {
                if value.is_finite() && (0.0..=1.0).contains(value) {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "reverb_send must be finite and within [0.0, 1.0]",
                    })
                }
            }
            (ControlKey::ReverbSend, Self::Reverb(_)) => Ok(()),
            (ControlKey::DelaySend, Self::Unipolar(_)) => Ok(()),
            (ControlKey::DelaySend, Self::Scalar(value)) => {
                if value.is_finite() && (0.0..=1.0).contains(value) {
                    Ok(())
                } else {
                    Err(ControlModelError::InvalidControlValueRange {
                        key: key.canonical_name(),
                        reason: "delay_send must be finite and within [0.0, 1.0]",
                    })
                }
            }
            (ControlKey::DelaySend, Self::Delay(_)) => Ok(()),
            (ControlKey::Compressor, Self::Compressor(_)) => Ok(()),
            (
                ControlKey::Gain | ControlKey::PlaybackRate | ControlKey::LowPassCutoff,
                Self::Signal(_),
            ) => Ok(()),
            (ControlKey::Select(_), Self::Choice(_)) => Ok(()),
            (ControlKey::PitchBend, Self::Bipolar(_)) => Ok(()),
            (ControlKey::ModWheel | ControlKey::Expression, Self::Unipolar(_)) => Ok(()),
            (ControlKey::Custom(_), value) => match value {
                Self::Bool(_)
                | Self::Unipolar(_)
                | Self::Bipolar(_)
                | Self::Choice(_)
                | Self::Signal(_)
                | Self::Semitones(_)
                | Self::Pan(_)
                | Self::Reverb(_)
                | Self::Delay(_)
                | Self::Compressor(_) => Ok(()),
                Self::Scalar(value) => {
                    if value.is_finite() {
                        Ok(())
                    } else {
                        Err(ControlModelError::InvalidControlValueRange {
                            key: key.canonical_name(),
                            reason: "custom scalar must be finite",
                        })
                    }
                }
                Self::Ramp { from, to } => {
                    if from.is_finite() && to.is_finite() {
                        Ok(())
                    } else {
                        Err(ControlModelError::InvalidControlValueRange {
                            key: key.canonical_name(),
                            reason: "custom ramp endpoints must be finite",
                        })
                    }
                }
            },
            _ => Err(ControlModelError::InvalidControlValue {
                key: key.canonical_name(),
                found: self.kind_label(),
            }),
        }
    }
}

/// Mapping from control keys to typed control values.
pub type ControlMap = BTreeMap<ControlKey, ControlValue>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Stable identifier for a control tile.
pub struct ControlTileId(u64);

impl ControlTileId {
    /// Creates a control-tile identifier from a raw integer.
    #[must_use]
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw integer value.
    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }
}

impl From<u64> for ControlTileId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Stable identifier for a control track.
pub struct ControlTrackId(u64);

impl ControlTrackId {
    /// Creates a control-track identifier from a raw integer.
    #[must_use]
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw integer value.
    #[must_use]
    pub fn value(self) -> u64 {
        self.0
    }
}

impl From<u64> for ControlTrackId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// One phase-local control value inside a repeating control track.
pub struct ControlTile {
    id: Option<ControlTileId>,
    phase: Span<Phase>,
    key: ControlKey,
    value: ControlValue,
}

impl ControlTile {
    /// Creates a control tile.
    ///
    /// Returns an error when the phase starts before zero or the value shape
    /// does not match the key.
    pub fn new(
        phase: Span<Phase>,
        key: ControlKey,
        value: ControlValue,
    ) -> Result<Self, ControlModelError> {
        if phase.start() < Time::ZERO {
            return Err(ControlModelError::InvalidTilePhaseStart);
        }

        value.validate_for(&key)?;

        Ok(Self {
            id: None,
            phase,
            key,
            value,
        })
    }

    /// Creates a control tile from raw phase-local bounds.
    pub fn spanning(
        start: Time,
        end: Time,
        key: ControlKey,
        value: ControlValue,
    ) -> Result<Self, ControlModelError> {
        Self::new(
            Span::new(start, end).ok_or(ControlModelError::InvalidTilePhaseStart)?,
            key,
            value,
        )
    }

    /// Attaches a stable identifier to the tile.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<ControlTileId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Returns the optional tile identifier.
    #[must_use]
    pub fn id(&self) -> Option<ControlTileId> {
        self.id
    }

    /// Returns the phase-local span occupied by this control tile.
    #[must_use]
    pub fn phase(&self) -> Span<Phase> {
        self.phase
    }

    /// Returns the control lane being driven.
    #[must_use]
    pub fn key(&self) -> &ControlKey {
        &self.key
    }

    /// Returns the control value.
    #[must_use]
    pub fn value(&self) -> &ControlValue {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Repeating source of control values aligned to a period.
pub struct ControlTrack {
    id: Option<ControlTrackId>,
    period: Time,
    repeat: Repeat,
    tiles: Vec<ControlTile>,
}

impl ControlTrack {
    /// Creates a repeating control track.
    pub fn new(period: Time, mut tiles: Vec<ControlTile>) -> Result<Self, ControlModelError> {
        if period <= Time::ZERO {
            return Err(ControlModelError::InvalidTrackPeriod);
        }

        if tiles
            .iter()
            .any(|tile| tile.phase().start() < Time::ZERO || tile.phase().end() > period)
        {
            return Err(ControlModelError::TileOutsideTrackPeriod);
        }

        tiles.sort_by(|left, right| {
            left.phase()
                .start()
                .cmp(&right.phase().start())
                .then(left.phase().end().cmp(&right.phase().end()))
                .then(left.key().cmp(right.key()))
        });

        Ok(Self {
            id: None,
            period,
            repeat: Repeat::Forever,
            tiles,
        })
    }

    /// Builds a one-cycle control track that attaches a continuous [`Signal`]
    /// to `key` for the whole period.
    ///
    /// Signal lanes are attach-once values: the single tile spans the entire
    /// `[0, 1)` period and is carried through projection without slicing.
    pub fn from_signal(key: ControlKey, signal: Signal) -> Result<Self, ControlModelError> {
        Self::new(
            Time::ONE,
            vec![ControlTile::spanning(
                Time::ZERO,
                Time::ONE,
                key,
                ControlValue::Signal(signal),
            )?],
        )
    }

    /// Attaches a stable identifier to the track.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<ControlTrackId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Replaces the repetition policy.
    #[must_use]
    pub fn with_repeat(mut self, repeat: Repeat) -> Self {
        self.repeat = repeat;
        self
    }

    /// Returns the optional track identifier.
    #[must_use]
    pub fn id(&self) -> Option<ControlTrackId> {
        self.id
    }

    /// Returns the length of one control period.
    #[must_use]
    pub fn period(&self) -> Time {
        self.period
    }

    /// Returns how the track repeats after one period.
    #[must_use]
    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    /// Returns the tiles in sorted phase order.
    #[must_use]
    pub fn tiles(&self) -> &[ControlTile] {
        &self.tiles
    }

    /// Iterates over control tiles in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = &ControlTile> {
        self.tiles.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_value_enforces_range() {
        assert!(UnitValue::new(0.5).is_some());
        assert!(UnitValue::new(-0.1).is_none());
        assert!(UnitValue::new(1.1).is_none());
    }

    #[test]
    fn control_tile_rejects_key_value_mismatch() {
        let tile = ControlTile::spanning(
            Time::ZERO,
            Time::ONE,
            ControlKey::Gain,
            ControlValue::Bool(true),
        );

        assert!(matches!(
            tile,
            Err(ControlModelError::InvalidControlValue { .. })
        ));
    }

    #[test]
    fn control_track_accepts_tiles_inside_period() {
        let track = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    ControlKey::Gain,
                    ControlValue::Ramp { from: 1.0, to: 0.0 },
                )
                .unwrap(),
            ],
        )
        .unwrap();

        assert_eq!(track.tiles().len(), 1);
        assert_eq!(track.repeat(), Repeat::Forever);
    }

    #[test]
    fn control_specs_report_canonical_names() {
        assert_eq!(ControlKey::PlaybackRate.canonical_name(), "playback_rate");
        assert_eq!(ControlKey::LowPassCutoff.canonical_name(), "lowpass_cutoff");
    }

    #[test]
    fn song_controls_accept_expected_value_shapes() {
        let tile = ControlTile::spanning(
            Time::ZERO,
            Time::ONE,
            ControlKey::ReverbSend,
            ControlValue::Reverb(ReverbSettings::new(
                UnitValue::new(0.4).unwrap(),
                Duration::from_secs_f64(2.0),
                UnitValue::new(0.3).unwrap(),
            )),
        )
        .unwrap();

        assert!(matches!(tile.key(), ControlKey::ReverbSend));
    }
}
