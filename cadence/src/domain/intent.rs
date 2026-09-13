//! Typed musical intents carried by tiles and moments.

use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
/// Invalid equal-slice selection inside a sample region.
pub enum SampleSliceError {
    /// Equal slicing requires between one and 65,536 parts.
    #[error("sample slice count must be between 1 and 65536")]
    InvalidCount,
    /// The zero-based slice index must be smaller than the count.
    #[error("sample slice index must be smaller than its count")]
    InvalidIndex,
    /// The selected region must have finite ascending bounds in [0, 1].
    #[error("sample region must have finite bounds with 0 <= start < end <= 1")]
    InvalidRegion,
}

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

#[derive(Debug, Clone, PartialEq)]
/// Typed musical action to perform when a moment or tile becomes active.
pub enum Intent {
    /// Trigger sample-backed playback.
    Sample(SampleIntent),
    /// Trigger a built-in synth source.
    Synth(SynthIntent),
    /// Toggle a named target on or off.
    Toggle(ToggleIntent),
    /// Set a level-like value on a named target.
    Level(LevelIntent),
    /// Set a rate value on a named target.
    Rate(RateIntent),
    /// Set a unit-range region on a named target.
    Region(RegionIntent),
    /// Open or close a gate on a named target.
    Gate(GateIntent),
    /// Select one named option on a named target.
    Select(SelectIntent),
}

impl Intent {
    /// Creates a sample intent with default playback settings.
    #[must_use]
    pub fn sample(sample_id: impl Into<String>) -> Self {
        Self::Sample(SampleIntent::new(sample_id))
    }

    /// Creates a synth intent.
    #[must_use]
    pub fn synth(source: BuiltInSynthSource) -> Self {
        Self::Synth(SynthIntent::new(source))
    }

    /// Creates a synth with a named set of envelope, filter and gain defaults.
    /// Authored controls override these defaults; note pitch stays independent.
    #[must_use]
    pub fn synth_preset(preset: SynthPreset) -> Self {
        Self::Synth(SynthIntent::preset(preset))
    }

    /// Creates a toggle intent.
    #[must_use]
    pub fn toggle(target: impl Into<String>, enabled: bool) -> Self {
        Self::Toggle(ToggleIntent::new(target, enabled))
    }

    /// Creates a level intent.
    #[must_use]
    pub fn level(target: impl Into<String>, value: f64) -> Self {
        Self::Level(LevelIntent::new(target, value))
    }

    /// Creates a rate intent.
    #[must_use]
    pub fn rate(target: impl Into<String>, value: f64) -> Self {
        Self::Rate(RateIntent::new(target, value))
    }

    /// Creates a region intent.
    #[must_use]
    pub fn region(target: impl Into<String>, start: f64, end: f64) -> Self {
        Self::Region(RegionIntent::new(target, start, end))
    }

    /// Creates a gate intent.
    #[must_use]
    pub fn gate(target: impl Into<String>, open: bool) -> Self {
        Self::Gate(GateIntent::new(target, open))
    }

    /// Creates a selection intent.
    #[must_use]
    pub fn select(target: impl Into<String>, option: impl Into<String>) -> Self {
        Self::Select(SelectIntent::new(target, option))
    }
}

impl Eq for Intent {}

impl Hash for Intent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Sample(intent) => {
                0_u8.hash(state);
                intent.hash(state);
            }
            Self::Synth(intent) => {
                1_u8.hash(state);
                intent.hash(state);
            }
            Self::Toggle(intent) => {
                2_u8.hash(state);
                intent.hash(state);
            }
            Self::Level(intent) => {
                3_u8.hash(state);
                intent.hash(state);
            }
            Self::Rate(intent) => {
                5_u8.hash(state);
                intent.hash(state);
            }
            Self::Region(intent) => {
                6_u8.hash(state);
                intent.hash(state);
            }
            Self::Gate(intent) => {
                7_u8.hash(state);
                intent.hash(state);
            }
            Self::Select(intent) => {
                8_u8.hash(state);
                intent.hash(state);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Built-in synth waveforms supported directly by the crate.
pub enum BuiltInSynthSource {
    /// Pure sine wave.
    Sine,
    /// Square wave with polynomial correction at its discontinuities.
    Square,
    /// Sawtooth wave with polynomial correction at its discontinuity.
    Saw,
    /// Triangle wave with polynomial correction at its corners.
    Triangle,
    /// Deterministic white noise. Pitch does not change this unpitched source.
    Noise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Small, stable sound definitions; controls can override each default.
pub enum SynthPreset {
    /// Filtered saw with a quick attack and short release for bass lines.
    Bass,
    /// Soft triangle with a slow attack and release for sustained harmony.
    Pad,
    /// High-passed noise with a short decay and zero sustain for percussion.
    Percussion,
}

impl SynthPreset {
    /// The oscillator selected by this sound definition.
    #[must_use]
    pub const fn source(self) -> BuiltInSynthSource {
        match self {
            Self::Bass => BuiltInSynthSource::Saw,
            Self::Pad => BuiltInSynthSource::Triangle,
            Self::Percussion => BuiltInSynthSource::Noise,
        }
    }

    /// Explicit defaults for control-thread lowering and host inspection.
    /// Times are seconds, cutoffs are hertz and gain/sustain are linear.
    #[must_use]
    pub fn controls(self) -> crate::domain::control::ControlMap {
        use crate::domain::control::{ControlKey as K, ControlValue as V};
        let (attack, decay, sustain, release, gain, cutoff, resonance) = match self {
            Self::Bass => (0.004, 0.12, 0.55, 0.06, 0.3, 900.0, 0.15),
            Self::Pad => (0.15, 0.2, 0.65, 0.4, 0.25, 3_500.0, 0.0),
            Self::Percussion => (0.001, 0.12, 0.0, 0.015, 0.3, 9_000.0, 0.0),
        };
        let mut controls = [
            (K::Attack, V::Scalar(attack)),
            (K::Decay, V::Scalar(decay)),
            (K::Sustain, V::Scalar(sustain)),
            (K::Release, V::Scalar(release)),
            (K::Gain, V::Scalar(gain)),
            (K::LowPassCutoff, V::Scalar(cutoff)),
            (K::LowPassResonance, V::Scalar(resonance)),
        ]
        .into_iter()
        .collect::<crate::domain::control::ControlMap>();
        if matches!(self, Self::Percussion) {
            controls.insert(K::HighPassCutoff, V::Scalar(1_800.0));
        }
        controls
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Intent to trigger one built-in synth source.
pub struct SynthIntent {
    /// Which built-in source to use.
    pub source: BuiltInSynthSource,
    /// Optional sound defaults, applied below ambient and authored controls.
    pub preset: Option<SynthPreset>,
    /// Ordered per-voice processing, after control-derived filters and compression.
    pub inserts: super::inserts::InsertChain,
    /// Instrument envelope, gain, and sends, below authored controls.
    pub sound_defaults: super::sound::SoundDefaults,
}

impl Eq for SynthIntent {}

impl Hash for SynthIntent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.source.hash(state);
        self.preset.hash(state);
        self.inserts.hash(state);
        self.sound_defaults.hash(state);
    }
}

impl SynthIntent {
    /// Creates a synth intent.
    #[must_use]
    pub fn new(source: BuiltInSynthSource) -> Self {
        Self {
            source,
            preset: None,
            inserts: Default::default(),
            sound_defaults: Default::default(),
        }
    }

    /// Creates one stable sound definition without changing note pitch.
    #[must_use]
    pub fn preset(preset: SynthPreset) -> Self {
        Self {
            source: preset.source(),
            preset: Some(preset),
            inserts: Default::default(),
            sound_defaults: Default::default(),
        }
    }

    /// Sound defaults before ambient and authored controls are overlaid.
    #[must_use]
    pub fn default_controls(&self) -> crate::domain::control::ControlMap {
        let mut controls = self.preset.map(SynthPreset::controls).unwrap_or_default();
        controls.extend(self.sound_defaults.controls());
        controls
    }

    /// Replaces instrument defaults without changing authored controls.
    pub fn with_sound_defaults(mut self, defaults: super::sound::SoundDefaults) -> Self {
        self.sound_defaults = defaults;
        self
    }

    /// Replaces the immutable ordered insert chain for subsequent voices.
    pub fn with_inserts(mut self, inserts: super::inserts::InsertChain) -> Self {
        self.inserts = inserts;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Intent to trigger one decoded sample with source-level playback settings.
pub struct SampleIntent {
    /// Logical sample identifier.
    pub sample_id: String,
    /// Base gain multiplier.
    pub gain: f64,
    /// Base playback rate, which must stay positive.
    pub rate: f64,
    /// Unit-range start position.
    pub start: f64,
    /// Unit-range end position.
    pub end: f64,
    /// Whether playback should run backward through the region.
    pub reverse: bool,
    /// Ordered per-voice processing, after control-derived filters and compression.
    pub inserts: super::inserts::InsertChain,
    /// Instrument envelope, gain, and sends, below authored controls.
    pub sound_defaults: super::sound::SoundDefaults,
}

impl Eq for SampleIntent {}

impl Hash for SampleIntent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.sample_id.hash(state);
        self.gain.to_bits().hash(state);
        self.rate.to_bits().hash(state);
        self.start.to_bits().hash(state);
        self.end.to_bits().hash(state);
        self.reverse.hash(state);
        self.inserts.hash(state);
        self.sound_defaults.hash(state);
    }
}

impl SampleIntent {
    /// Default gain multiplier.
    pub const DEFAULT_GAIN: f64 = 1.0;
    /// Default playback rate.
    pub const DEFAULT_RATE: f64 = 1.0;
    /// Default unit-range region start.
    pub const DEFAULT_START: f64 = 0.0;
    /// Default unit-range region end.
    pub const DEFAULT_END: f64 = 1.0;

    /// Creates a sample intent with playable defaults.
    #[must_use]
    pub fn new(sample_id: impl Into<String>) -> Self {
        Self {
            sample_id: sample_id.into(),
            gain: Self::DEFAULT_GAIN,
            rate: Self::DEFAULT_RATE,
            start: Self::DEFAULT_START,
            end: Self::DEFAULT_END,
            reverse: false,
            inserts: Default::default(),
            sound_defaults: Default::default(),
        }
    }

    /// Instrument defaults below ambient and authored controls.
    pub fn default_controls(&self) -> crate::domain::control::ControlMap {
        self.sound_defaults.controls()
    }

    /// Replaces the base gain.
    ///
    /// # Panics
    ///
    /// Panics if `gain` is negative or not finite.
    #[must_use]
    pub fn gain(mut self, gain: f64) -> Self {
        assert_non_negative("sample gain", gain);
        self.gain = gain;
        self
    }

    /// Replaces instrument defaults without changing authored controls.
    pub fn with_sound_defaults(mut self, defaults: super::sound::SoundDefaults) -> Self {
        self.sound_defaults = defaults;
        self
    }

    /// Replaces the immutable ordered insert chain for subsequent voices.
    pub fn with_inserts(mut self, inserts: super::inserts::InsertChain) -> Self {
        self.inserts = inserts;
        self
    }

    /// Replaces the base playback rate.
    ///
    /// # Panics
    ///
    /// Panics if `rate <= 0` or not finite.
    #[must_use]
    pub fn rate(mut self, rate: f64) -> Self {
        assert_positive("sample rate", rate);
        self.rate = rate;
        self
    }

    /// Replaces the unit-range playback region.
    ///
    /// # Panics
    ///
    /// Panics unless `0.0 <= start < end <= 1.0` with finite bounds.
    #[must_use]
    pub fn region(mut self, start: f64, end: f64) -> Self {
        assert_unit_region("sample region", start, end);
        self.start = start;
        self.end = end;
        self
    }

    /// Selects one zero-based equal slice of the current region.
    ///
    /// This changes source bounds only; note timing and playback rate remain
    /// unchanged. Use the `Fit` control to fit the slice to its authored slot.
    pub fn slice(mut self, index: u32, count: u32) -> Result<Self, SampleSliceError> {
        if !(1..=65_536).contains(&count) {
            return Err(SampleSliceError::InvalidCount);
        }
        if index >= count {
            return Err(SampleSliceError::InvalidIndex);
        }
        if !self.start.is_finite()
            || !self.end.is_finite()
            || self.start < 0.0
            || self.start >= self.end
            || self.end > 1.0
        {
            return Err(SampleSliceError::InvalidRegion);
        }
        let width = self.end - self.start;
        let start = self.start + width * f64::from(index) / f64::from(count);
        let end = self.start + width * f64::from(index + 1) / f64::from(count);
        if end <= start {
            return Err(SampleSliceError::InvalidRegion);
        }
        self.start = start;
        self.end = end;
        Ok(self)
    }

    /// Sets whether playback should run in reverse.
    #[must_use]
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = reverse;
        self
    }

    #[cfg(test)]
    pub(crate) fn assert_valid(&self) {
        assert_non_negative("sample gain", self.gain);
        assert_positive("sample rate", self.rate);
        assert_unit_region("sample region", self.start, self.end);
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Intent to toggle a named target.
pub struct ToggleIntent {
    /// Name of the target to change.
    pub target: String,
    /// New enabled state.
    pub enabled: bool,
}

impl Eq for ToggleIntent {}

impl Hash for ToggleIntent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.enabled.hash(state);
    }
}

impl ToggleIntent {
    /// Creates a toggle intent.
    #[must_use]
    pub fn new(target: impl Into<String>, enabled: bool) -> Self {
        Self {
            target: target.into(),
            enabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Intent to set a level-like scalar on a named target.
pub struct LevelIntent {
    /// Name of the target to change.
    pub target: String,
    /// New level value.
    pub value: f64,
}

impl Eq for LevelIntent {}

impl Hash for LevelIntent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.value.to_bits().hash(state);
    }
}

impl LevelIntent {
    /// Creates a level intent.
    ///
    /// # Panics
    ///
    /// Panics if `value` is not finite.
    #[must_use]
    pub fn new(target: impl Into<String>, value: f64) -> Self {
        assert_finite("level value", value);
        Self {
            target: target.into(),
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Intent to set playback or transport rate on a named target.
pub struct RateIntent {
    /// Name of the target to change.
    pub target: String,
    /// Positive rate value.
    pub value: f64,
}

impl Eq for RateIntent {}

impl Hash for RateIntent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.value.to_bits().hash(state);
    }
}

impl RateIntent {
    /// Creates a rate intent.
    ///
    /// # Panics
    ///
    /// Panics if `value <= 0` or not finite.
    #[must_use]
    pub fn new(target: impl Into<String>, value: f64) -> Self {
        assert_positive("rate value", value);
        Self {
            target: target.into(),
            value,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Intent to select a unit-range region on a named target.
pub struct RegionIntent {
    /// Name of the target to change.
    pub target: String,
    /// Unit-range region start.
    pub start: f64,
    /// Unit-range region end.
    pub end: f64,
}

impl Eq for RegionIntent {}

impl Hash for RegionIntent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.start.to_bits().hash(state);
        self.end.to_bits().hash(state);
    }
}

impl RegionIntent {
    /// Creates a region intent.
    ///
    /// # Panics
    ///
    /// Panics unless `0.0 <= start < end <= 1.0` with finite bounds.
    #[must_use]
    pub fn new(target: impl Into<String>, start: f64, end: f64) -> Self {
        assert_unit_region("region value", start, end);
        Self {
            target: target.into(),
            start,
            end,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Intent to open or close a named gate.
pub struct GateIntent {
    /// Name of the target to change.
    pub target: String,
    /// New gate state.
    pub open: bool,
}

impl Eq for GateIntent {}

impl Hash for GateIntent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.open.hash(state);
    }
}

impl GateIntent {
    /// Creates a gate intent.
    #[must_use]
    pub fn new(target: impl Into<String>, open: bool) -> Self {
        Self {
            target: target.into(),
            open,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Intent to choose one named option on a target.
pub struct SelectIntent {
    /// Name of the target to change.
    pub target: String,
    /// Option to select.
    pub option: String,
}

impl Eq for SelectIntent {}

impl Hash for SelectIntent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.target.hash(state);
        self.option.hash(state);
    }
}

impl SelectIntent {
    /// Creates a selection intent.
    #[must_use]
    pub fn new(target: impl Into<String>, option: impl Into<String>) -> Self {
        Self {
            target: target.into(),
            option: option.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic;

    #[test]
    fn sample_intent_defaults_to_playable_values() {
        let intent = SampleIntent::new("kick");

        assert_eq!(intent.sample_id, "kick");
        assert_eq!(intent.gain, SampleIntent::DEFAULT_GAIN);
        assert_eq!(intent.rate, SampleIntent::DEFAULT_RATE);
        assert_eq!(intent.start, SampleIntent::DEFAULT_START);
        assert_eq!(intent.end, SampleIntent::DEFAULT_END);
        assert!(!intent.reverse);
    }

    #[test]
    fn typed_intents_do_not_require_string_control_maps() {
        let intent = Intent::level("filter", 0.75);

        match intent {
            Intent::Level(level) => {
                assert_eq!(level.target, "filter");
                assert_eq!(level.value, 0.75);
            }
            other => panic!("expected level intent, got {other:?}"),
        }
    }

    #[test]
    fn synth_intent_carries_a_typed_source() {
        let intent = Intent::synth(BuiltInSynthSource::Triangle);

        match intent {
            Intent::Synth(synth) => assert_eq!(synth.source, BuiltInSynthSource::Triangle),
            other => panic!("expected synth intent, got {other:?}"),
        }
    }

    #[test]
    fn sample_intent_accepts_edge_values_inside_the_supported_range() {
        let intent = SampleIntent::new("kick")
            .gain(0.0)
            .rate(0.25)
            .region(0.0, 1.0);

        intent.assert_valid();
        assert_eq!(intent.gain, 0.0);
        assert_eq!(intent.rate, 0.25);
        assert_eq!(intent.start, 0.0);
        assert_eq!(intent.end, 1.0);
    }

    #[test]
    fn sample_intent_rate_rejects_non_positive_values() {
        assert!(
            panic::catch_unwind(|| {
                let _ = SampleIntent::new("kick").rate(0.0);
            })
            .is_err()
        );
    }

    #[test]
    fn sample_intent_region_rejects_invalid_bounds() {
        assert!(
            panic::catch_unwind(|| {
                let _ = SampleIntent::new("kick").region(0.8, 0.8);
            })
            .is_err()
        );
        assert!(
            panic::catch_unwind(|| {
                let _ = SampleIntent::new("kick").region(0.2, f64::INFINITY);
            })
            .is_err()
        );
    }

    #[test]
    fn typed_numeric_intents_reject_invalid_values() {
        assert!(panic::catch_unwind(|| Intent::level("filter", f64::NAN)).is_err());
        assert!(panic::catch_unwind(|| Intent::rate("transport", f64::INFINITY)).is_err());
        assert!(panic::catch_unwind(|| Intent::region("slice", -0.1, 0.5)).is_err());
    }
}
