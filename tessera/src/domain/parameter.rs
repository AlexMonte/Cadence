//! Typed first-release parameter contracts. Hosts bind these to their sound engine
//! and verify support; this crate does not depend on a particular audio runtime.
use super::{FieldValue, Rational};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterKey {
    Fast,
    Slow,
    Late,
    Gain,
    Attack,
    Decay,
    Release,
    Pan,
    Delay,
    Reverb,
    Compressor,
    Velocity,
    ClipLength,
    PostGain,
    PitchBend,
    Expression,
    Transpose,
    Gate,
    Legato,
    Sustain,
    LowPassCutoff,
    LowPassResonance,
    HighPassCutoff,
    HighPassResonance,
    SampleBank,
    SampleVariant,
    PlaybackRate,
    PlaybackStart,
    PlaybackEnd,
    Reverse,
    Fit,
    Loop,
    Slice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterUnit {
    RateRatio,
    LinearGain,
    StereoPosition,
    PitchBendAmount,
    Seconds,
    Semitones,
    Boolean,
    DurationRatio,
    UnitLevel,
    Hertz,
    BankName,
    VariantIndex,
    SourcePosition,
    EffectSettings,
    SliceSelection,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterDomain {
    Positive,
    NonNegative,
    AnyScalar,
    SemitoneOffset,
    UnitInterval,
    SignedUnitInterval,
    Boolean,
    VariantIndex,
    NonEmptySymbol,
    PlaybackRate,
    EffectSettings,
    SliceSelection,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterTiming {
    PatternTime,
    Onset,
    VoiceLifecycle,
    SegmentSampled,
    ContinuousRuntime,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterMerge {
    Multiply,
    Add,
    Override,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterSource {
    Pattern,
    SharedSound,
    SampleOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterSpec {
    pub key: ParameterKey,
    pub label: &'static str,
    pub unit: ParameterUnit,
    pub domain: ParameterDomain,
    pub default: Option<FieldValue>,
    /// Suggested editing range; validation uses `domain`, not these UI limits.
    pub editor_range: Option<(Rational, Rational)>,
    pub timing: ParameterTiming,
    pub merge: ParameterMerge,
    pub source: ParameterSource,
    pub accepts_pattern: bool,
    /// The host maps this typed contract to an engine key with this canonical name.
    pub host_control_name: Option<&'static str>,
}

impl ParameterKey {
    pub const ALL: &'static [Self] = &[
        Self::Fast,
        Self::Slow,
        Self::Late,
        Self::Gain,
        Self::Attack,
        Self::Decay,
        Self::Release,
        Self::Pan,
        Self::Delay,
        Self::Reverb,
        Self::Compressor,
        Self::Velocity,
        Self::ClipLength,
        Self::PostGain,
        Self::PitchBend,
        Self::Expression,
        Self::Transpose,
        Self::Gate,
        Self::Legato,
        Self::Sustain,
        Self::LowPassCutoff,
        Self::LowPassResonance,
        Self::HighPassCutoff,
        Self::HighPassResonance,
        Self::SampleBank,
        Self::SampleVariant,
        Self::PlaybackRate,
        Self::PlaybackStart,
        Self::PlaybackEnd,
        Self::Reverse,
        Self::Fit,
        Self::Loop,
        Self::Slice,
    ];

    pub fn control_key(self) -> Option<super::ControlKeyIr> {
        use super::ControlKeyIr as C;
        Some(match self {
            Self::Fast | Self::Slow | Self::Late | Self::Slice => return None,
            Self::Gain => C::Gain,
            Self::Attack => C::Attack,
            Self::Decay => C::Decay,
            Self::Release => C::Release,
            Self::Pan => C::Pan,
            Self::Delay => C::DelaySend,
            Self::Reverb => C::ReverbSend,
            Self::Compressor => C::Compressor,
            Self::Velocity => C::Velocity,
            Self::ClipLength => C::ClipLength,
            Self::PostGain => C::PostGain,
            Self::PitchBend => C::PitchBend,
            Self::Expression => C::Expression,

            Self::Transpose => C::Transpose,
            Self::Gate => C::Gate,
            Self::Legato => C::Legato,
            Self::Sustain => C::Sustain,
            Self::LowPassCutoff => C::LowPassCutoff,
            Self::LowPassResonance => C::LowPassResonance,
            Self::HighPassCutoff => C::HighPassCutoff,
            Self::HighPassResonance => C::HighPassResonance,
            Self::SampleBank => C::SampleBank,
            Self::SampleVariant => C::SampleVariant,
            Self::PlaybackRate => C::PlaybackRate,
            Self::PlaybackStart => C::PlaybackStart,
            Self::PlaybackEnd => C::PlaybackEnd,
            Self::Reverse => C::Reverse,
            Self::Fit => C::Fit,
            Self::Loop => C::Loop,
        })
    }

    pub fn spec(self) -> ParameterSpec {
        use ParameterDomain as D;
        use ParameterMerge as M;
        use ParameterSource as S;
        use ParameterTiming as T;
        use ParameterUnit as U;
        let r = Rational::from_integer;
        let scalar = |value| Some(FieldValue::rational(value));
        let (label, unit, domain, default, range, timing, merge, source, pattern, host) = match self
        {
            Self::Late => (
                "Timing offset (cycles)",
                U::DurationRatio,
                D::AnyScalar,
                scalar(r(0)),
                Some((r(-1), r(1))),
                T::PatternTime,
                M::Add,
                S::Pattern,
                false,
                None,
            ),
            Self::Fast => (
                "Fast",
                U::RateRatio,
                D::Positive,
                scalar(r(1)),
                Some((Rational::new(1, 4), r(4))),
                T::PatternTime,
                M::Multiply,
                S::Pattern,
                false,
                None,
            ),
            Self::Slow => (
                "Slow",
                U::RateRatio,
                D::Positive,
                scalar(r(1)),
                Some((Rational::new(1, 4), r(4))),
                T::PatternTime,
                M::Multiply,
                S::Pattern,
                false,
                None,
            ),
            Self::Gain => (
                "Gain",
                U::LinearGain,
                D::NonNegative,
                scalar(r(1)),
                Some((r(0), r(2))),
                T::SegmentSampled,
                M::Multiply,
                S::SharedSound,
                true,
                Some("gain"),
            ),
            Self::Attack | Self::Decay | Self::Release => (
                match self {
                    Self::Attack => "Attack",
                    Self::Decay => "Decay",
                    _ => "Release",
                },
                U::Seconds,
                D::NonNegative,
                scalar(r(0)),
                Some((r(0), r(2))),
                T::VoiceLifecycle,
                M::Override,
                S::SharedSound,
                true,
                Some(match self {
                    Self::Attack => "attack",
                    Self::Decay => "decay",
                    _ => "release",
                }),
            ),
            Self::Pan => (
                "Pan",
                U::StereoPosition,
                D::SignedUnitInterval,
                scalar(r(0)),
                Some((r(-1), r(1))),
                T::ContinuousRuntime,
                M::Add,
                S::SharedSound,
                true,
                Some("pan"),
            ),
            Self::Velocity => (
                "Velocity",
                U::UnitLevel,
                D::UnitInterval,
                scalar(r(1)),
                Some((r(0), r(1))),
                T::Onset,
                M::Multiply,
                S::SharedSound,
                true,
                Some("velocity"),
            ),
            Self::ClipLength => (
                "Clip length",
                U::DurationRatio,
                D::NonNegative,
                scalar(r(1)),
                Some((r(0), r(2))),
                T::Onset,
                M::Override,
                S::SharedSound,
                true,
                Some("clip_length"),
            ),
            Self::PostGain => (
                "Post-effects gain",
                U::LinearGain,
                D::NonNegative,
                scalar(r(1)),
                Some((r(0), r(2))),
                T::ContinuousRuntime,
                M::Multiply,
                S::SharedSound,
                true,
                Some("post_gain"),
            ),
            Self::PitchBend => (
                "Pitch bend",
                U::PitchBendAmount,
                D::SignedUnitInterval,
                scalar(r(0)),
                Some((r(-1), r(1))),
                T::ContinuousRuntime,
                M::Add,
                S::SharedSound,
                true,
                Some("pitch_bend"),
            ),
            Self::Expression => (
                "Expression",
                U::UnitLevel,
                D::UnitInterval,
                scalar(r(1)),
                Some((r(0), r(1))),
                T::ContinuousRuntime,
                M::Multiply,
                S::SharedSound,
                true,
                Some("expression"),
            ),
            Self::Delay => (
                "Delay",
                U::EffectSettings,
                D::EffectSettings,
                Some(FieldValue::Delay {
                    value: super::DelayParameters::default(),
                }),
                None,
                T::SegmentSampled,
                M::Override,
                S::SharedSound,
                true,
                Some("delay_send"),
            ),
            Self::Reverb => (
                "Reverb",
                U::EffectSettings,
                D::EffectSettings,
                Some(FieldValue::Reverb {
                    value: super::ReverbParameters::default(),
                }),
                None,
                T::SegmentSampled,
                M::Override,
                S::SharedSound,
                true,
                Some("reverb_send"),
            ),
            Self::Compressor => (
                "Compressor",
                U::EffectSettings,
                D::EffectSettings,
                Some(FieldValue::Compressor {
                    value: super::CompressorParameters::default(),
                }),
                None,
                T::SegmentSampled,
                M::Override,
                S::SharedSound,
                true,
                Some("compressor"),
            ),
            Self::Transpose => (
                "Transpose",
                U::Semitones,
                D::SemitoneOffset,
                scalar(r(0)),
                Some((r(-24), r(24))),
                T::ContinuousRuntime,
                M::Add,
                S::SharedSound,
                true,
                Some("transpose"),
            ),
            Self::Gate => (
                "Gate",
                U::Boolean,
                D::Boolean,
                Some(FieldValue::bool(true)),
                Some((r(0), r(1))),
                T::SegmentSampled,
                M::Override,
                S::SharedSound,
                true,
                Some("gate"),
            ),
            Self::Legato => (
                "Note length",
                U::DurationRatio,
                D::Positive,
                scalar(r(1)),
                Some((Rational::new(1, 4), r(2))),
                T::Onset,
                M::Override,
                S::SharedSound,
                true,
                Some("legato"),
            ),
            Self::Sustain => (
                "Sustain level",
                U::UnitLevel,
                D::UnitInterval,
                scalar(r(1)),
                Some((r(0), r(1))),
                T::VoiceLifecycle,
                M::Override,
                S::SharedSound,
                true,
                Some("sustain"),
            ),
            Self::LowPassCutoff | Self::HighPassCutoff => (
                if self == Self::LowPassCutoff {
                    "Low-pass cutoff"
                } else {
                    "High-pass cutoff"
                },
                U::Hertz,
                D::Positive,
                scalar(r(if self == Self::LowPassCutoff {
                    20_000
                } else {
                    20
                })),
                Some((r(20), r(20_000))),
                T::SegmentSampled,
                M::Override,
                S::SharedSound,
                true,
                Some(if self == Self::LowPassCutoff {
                    "lowpass_cutoff"
                } else {
                    "highpass_cutoff"
                }),
            ),
            Self::LowPassResonance | Self::HighPassResonance => (
                if self == Self::LowPassResonance {
                    "Low-pass resonance"
                } else {
                    "High-pass resonance"
                },
                U::UnitLevel,
                D::UnitInterval,
                scalar(r(0)),
                Some((r(0), r(1))),
                T::SegmentSampled,
                M::Override,
                S::SharedSound,
                true,
                Some(if self == Self::LowPassResonance {
                    "lowpass_resonance"
                } else {
                    "highpass_resonance"
                }),
            ),
            Self::SampleBank => (
                "Sample bank",
                U::BankName,
                D::NonEmptySymbol,
                None,
                None,
                T::Onset,
                M::Override,
                S::SampleOnly,
                false,
                Some("sample_bank"),
            ),
            Self::SampleVariant => (
                "Sample variant",
                U::VariantIndex,
                D::VariantIndex,
                scalar(r(0)),
                Some((r(0), r(127))),
                T::Onset,
                M::Override,
                S::SampleOnly,
                true,
                Some("sample_variant"),
            ),
            Self::PlaybackRate => (
                "Sample rate",
                U::RateRatio,
                D::PlaybackRate,
                scalar(r(1)),
                Some((Rational::new(1, 4), r(4))),
                T::SegmentSampled,
                M::Override,
                S::SampleOnly,
                true,
                Some("playback_rate"),
            ),
            Self::PlaybackStart => (
                "Sample start",
                U::SourcePosition,
                D::UnitInterval,
                scalar(r(0)),
                Some((r(0), r(1))),
                T::Onset,
                M::Override,
                S::SampleOnly,
                true,
                Some("playback_start"),
            ),
            Self::PlaybackEnd => (
                "Sample end",
                U::SourcePosition,
                D::UnitInterval,
                scalar(r(1)),
                Some((r(0), r(1))),
                T::Onset,
                M::Override,
                S::SampleOnly,
                true,
                Some("playback_end"),
            ),
            Self::Reverse | Self::Fit | Self::Loop => (
                match self {
                    Self::Reverse => "Reverse sample",
                    Self::Fit => "Fit to note",
                    _ => "Loop sample",
                },
                U::Boolean,
                D::Boolean,
                Some(FieldValue::bool(false)),
                Some((r(0), r(1))),
                T::Onset,
                M::Override,
                S::SampleOnly,
                true,
                Some(match self {
                    Self::Reverse => "reverse",
                    Self::Fit => "fit",
                    _ => "loop",
                }),
            ),
            Self::Slice => (
                "Sample slice",
                U::SliceSelection,
                D::SliceSelection,
                Some(FieldValue::Slice {
                    index: 0,
                    count: 16,
                }),
                None,
                T::Onset,
                M::Override,
                S::SampleOnly,
                false,
                None,
            ),
        };
        ParameterSpec {
            key: self,
            label,
            unit,
            domain,
            default,
            editor_range: range,
            timing,
            merge,
            source,
            accepts_pattern: pattern,
            host_control_name: host,
        }
    }
}

impl ParameterSpec {
    pub fn validate(&self, value: &FieldValue) -> Result<(), &'static str> {
        if let FieldValue::Modulation { value } = value {
            return value.validate_for(self.key);
        }
        if self.domain == ParameterDomain::EffectSettings {
            return match (self.key, value) {
                (ParameterKey::Delay, FieldValue::Delay { value }) => value.validate(),
                (ParameterKey::Reverb, FieldValue::Reverb { value }) => value.validate(),
                (ParameterKey::Compressor, FieldValue::Compressor { value }) => value.validate(),
                _ => Err("Keep each effect's complete settings together."),
            };
        }
        if self.domain == ParameterDomain::SliceSelection {
            return match value {
                FieldValue::Slice { index, count }
                    if (1..=65_536).contains(count) && index < count =>
                {
                    Ok(())
                }
                _ => Err(
                    "A slice needs a count between 1 and 65536 and a zero-based index below that count.",
                ),
            };
        }
        if self.domain == ParameterDomain::NonEmptySymbol {
            return match value {
                FieldValue::Symbol { value } if !value.trim().is_empty() => Ok(()),
                _ => Err("Choose a non-empty sample bank identifier."),
            };
        }
        if self.domain == ParameterDomain::Boolean && matches!(value, FieldValue::Bool { .. }) {
            return Ok(());
        }
        let FieldValue::Rational { value } = value else {
            return Err("This parameter requires a numeric value.");
        };
        if value.denominator <= 0 {
            return Err("A parameter rational must have a positive denominator.");
        }
        if self.key == ParameterKey::Late
            && (*value < Rational::from_integer(-1024) || *value > Rational::from_integer(1024))
        {
            return Err("Timing offset must be within 1024 cycles.");
        }
        let valid = match self.domain {
            ParameterDomain::Positive => *value > Rational::zero(),
            ParameterDomain::NonNegative => *value >= Rational::zero(),
            ParameterDomain::AnyScalar => true,
            ParameterDomain::SemitoneOffset => {
                *value >= Rational::from_integer(-127) && *value <= Rational::from_integer(127)
            }
            ParameterDomain::UnitInterval => {
                *value >= Rational::zero() && *value <= Rational::one()
            }
            ParameterDomain::SignedUnitInterval => {
                *value >= Rational::from_integer(-1) && *value <= Rational::one()
            }
            ParameterDomain::Boolean => *value == Rational::zero() || *value == Rational::one(),
            ParameterDomain::VariantIndex => {
                value.denominator == 1 && (0..=i64::from(u32::MAX)).contains(&value.numerator)
            }
            ParameterDomain::PlaybackRate => {
                *value > Rational::zero() && *value <= Rational::from_integer(65_536)
            }
            ParameterDomain::NonEmptySymbol
            | ParameterDomain::SliceSelection
            | ParameterDomain::EffectSettings => unreachable!(),
        };
        if valid {
            Ok(())
        } else {
            Err(match self.domain {
                ParameterDomain::Positive => "This value must be greater than zero.",
                ParameterDomain::NonNegative => "This value must be zero or greater.",
                ParameterDomain::UnitInterval => "This level must be between zero and one.",
                ParameterDomain::SignedUnitInterval => "This value must be between -1 and 1.",
                ParameterDomain::SemitoneOffset => {
                    "Transpose must be between -127 and 127 semitones."
                }
                ParameterDomain::Boolean => "A switch accepts only zero (off) or one (on).",
                ParameterDomain::PlaybackRate => {
                    "Sample rate must be greater than zero and at most 65536."
                }
                ParameterDomain::VariantIndex => {
                    "Sample variant must be a non-negative whole number."
                }
                _ => "Invalid parameter value.",
            })
        }
    }
}
