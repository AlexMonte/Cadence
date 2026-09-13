use serde::{Deserialize, Serialize};
use std::fmt;

use super::container::ContainerId;
use super::pattern_ir::Rational;
use super::stack::SignedAccidental;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteAtom {
    pub value: NoteValue,
    /// Original note label from authoring; used to detect invalid spellings at normalize time.
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub octave: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accidental: Option<SignedAccidental>,
}

impl NoteAtom {
    pub fn new(value: impl AsRef<str>) -> Self {
        let label = value.as_ref().to_string();
        Self {
            value: NoteValue::from(value.as_ref()),
            label,
            octave: None,
            accidental: None,
        }
    }

    pub fn with_accidental(mut self, accidental: SignedAccidental) -> Self {
        self.accidental = Some(accidental);
        self
    }

    /// Spelling of the pitch class, independent of sample/instrument identity.
    pub fn pitch_label(&self) -> String {
        let suffix = match self.accidental {
            Some(SignedAccidental::Sharp) => "#",
            Some(SignedAccidental::Flat) => "b",
            _ => "",
        };
        format!("{}{suffix}", self.value)
    }

    /// MIDI-style semitone coordinate. B# and Cb cross octave boundaries naturally.
    pub fn semitone(&self, default_octave: i64) -> i64 {
        let natural = match self.value {
            NoteValue::C => 0,
            NoteValue::D => 2,
            NoteValue::E => 4,
            NoteValue::F => 5,
            NoteValue::G => 7,
            NoteValue::A => 9,
            NoteValue::B => 11,
        };
        let accidental = match self.accidental {
            Some(SignedAccidental::Sharp) => 1,
            Some(SignedAccidental::Flat) => -1,
            _ => 0,
        };
        (self.octave.unwrap_or(default_octave) + 1) * 12 + natural + accidental
    }

    pub fn with_octave(mut self, octave: i64) -> Self {
        self.octave = Some(octave);
        self
    }
}

pub fn try_parse_note_value(value: &str) -> Option<NoteValue> {
    match value.to_ascii_lowercase().as_str() {
        "a" => Some(NoteValue::A),
        "b" => Some(NoteValue::B),
        "c" => Some(NoteValue::C),
        "d" => Some(NoteValue::D),
        "e" => Some(NoteValue::E),
        "f" => Some(NoteValue::F),
        "g" => Some(NoteValue::G),
        _ => None,
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum NoteValue {
    #[default]
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

impl From<&str> for NoteValue {
    fn from(value: &str) -> Self {
        match value.to_ascii_lowercase().as_str() {
            "a" => Self::A,
            "b" => Self::B,
            "c" => Self::C,
            "d" => Self::D,
            "e" => Self::E,
            "f" => Self::F,
            "g" => Self::G,
            _ => Self::A,
        }
    }
}

impl fmt::Display for NoteValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
            Self::E => "e",
            Self::F => "f",
            Self::G => "g",
        };
        f.write_str(value)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScalarAtom {
    pub value: Rational,
}

impl ScalarAtom {
    pub fn integer(value: i64) -> Self {
        Self {
            value: Rational::from_integer(value),
        }
    }

    pub fn rational(numerator: i64, denominator: i64) -> Self {
        Self {
            value: Rational::new(numerator, denominator),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomOperatorToken {
    Fast,
    Slow,
    Elongate,
    Replicate,
    Degrade,
    Choice,
    Parallel,
    Euclid,
    EuclidRot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AtomTile {
    Note(NoteAtom),
    /// Host-owned named sound, distinct from chromatic pitch spelling.
    Sound(String),
    Rest,
    Scalar(ScalarAtom),
    Operator(AtomOperatorToken),
    /// A number explicitly owned by the note pitch.
    Octave(i64),
    Accidental(SignedAccidental),
    /// A complete modifier group; moving it preserves operand ownership.
    Modifier(AtomModifier),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomModifier {
    /// Interpret numeric event values as zero-based positions in a musical scale.
    Scale(super::ScaleParameters),
    /// Cycle-varying pulse, step and rotation inputs; fixed Euclid groups remain supported.
    EuclidPattern(super::EuclidPatternParameters),
    Modulation {
        parameter: super::ParameterKey,
        value: super::ModulationParameters,
    },
    Delay(super::DelayParameters),
    Reverb(super::ReverbParameters),
    Compressor(super::CompressorParameters),

    Velocity(Rational),
    ClipLength(Rational),
    PostGain(Rational),
    PitchBend(Rational),
    Expression(Rational),

    Gain(Rational),
    Attack(Rational),
    Decay(Rational),
    Release(Rational),
    Transpose(Rational),
    Pan(Rational),
    HighPassCutoff(Rational),
    HighPassResonance(Rational),

    Gate(bool),
    Legato(Rational),
    Sustain(Rational),
    LowPassCutoff(Rational),
    LowPassResonance(Rational),
    SampleBank(String),
    SampleVariant(u32),
    PlaybackRate(Rational),
    PlaybackStart(Rational),
    PlaybackEnd(Rational),
    Reverse(bool),
    Fit(bool),
    Loop(bool),
    /// One zero-based equal slice; both operands belong to this tile.
    Slice {
        index: u32,
        count: u32,
    },
    /// Reverse event order and timing inside each pattern cycle.
    Rev,
    /// Shift a pattern by a signed number of cycles.
    Late(Rational),
    Fast(Rational),
    Slow(Rational),
    Elongate(Rational),
    Replicate(u32),
    Degrade(Option<Rational>),
    Euclid {
        pulses: u32,
        steps: u32,
    },
    EuclidRot {
        pulses: u32,
        steps: u32,
        rotation: i32,
    },
}

impl AtomModifier {
    pub fn effect_value(&self) -> Option<super::EffectValue> {
        Some(match self {
            Self::Modulation { parameter, value } => super::EffectValue::Modulation {
                parameter: *parameter,
                value: *value,
            },
            Self::Delay(value) => super::EffectValue::Delay(*value),
            Self::Reverb(value) => super::EffectValue::Reverb(*value),
            Self::Compressor(value) => super::EffectValue::Compressor(*value),
            _ => return None,
        })
    }

    /// The typed parameter role owned by this group, when it is in the catalog.
    pub fn parameter_key(&self) -> Option<super::ParameterKey> {
        use super::ParameterKey as P;
        Some(match self {
            Self::Gain(_) => P::Gain,
            Self::Attack(_) => P::Attack,
            Self::Decay(_) => P::Decay,
            Self::Release(_) => P::Release,
            Self::Transpose(_) => P::Transpose,
            Self::Pan(_) => P::Pan,
            Self::Modulation { parameter, .. } => *parameter,
            Self::Delay(_) => P::Delay,
            Self::Reverb(_) => P::Reverb,
            Self::Compressor(_) => P::Compressor,

            Self::Velocity(_) => P::Velocity,
            Self::ClipLength(_) => P::ClipLength,
            Self::PostGain(_) => P::PostGain,
            Self::PitchBend(_) => P::PitchBend,
            Self::Expression(_) => P::Expression,

            Self::HighPassCutoff(_) => P::HighPassCutoff,
            Self::HighPassResonance(_) => P::HighPassResonance,
            Self::Gate(_) => P::Gate,
            Self::Legato(_) => P::Legato,
            Self::Sustain(_) => P::Sustain,
            Self::LowPassCutoff(_) => P::LowPassCutoff,
            Self::LowPassResonance(_) => P::LowPassResonance,
            Self::SampleBank(_) => P::SampleBank,
            Self::SampleVariant(_) => P::SampleVariant,
            Self::PlaybackRate(_) => P::PlaybackRate,
            Self::PlaybackStart(_) => P::PlaybackStart,
            Self::PlaybackEnd(_) => P::PlaybackEnd,
            Self::Reverse(_) => P::Reverse,
            Self::Fit(_) => P::Fit,
            Self::Loop(_) => P::Loop,
            Self::Slice { .. } => P::Slice,
            Self::Fast(_) => P::Fast,
            Self::Late(_) => P::Late,
            Self::Slow(_) => P::Slow,
            _ => return None,
        })
    }

    /// An owned value is distinct from a free-standing numeric tile.
    pub fn parameter_value(&self) -> Option<super::FieldValue> {
        use super::FieldValue as V;
        Some(match self {
            Self::Modulation { value, .. } => V::Modulation { value: *value },
            Self::Delay(value) => V::Delay { value: *value },
            Self::Reverb(value) => V::Reverb { value: *value },
            Self::Compressor(value) => V::Compressor { value: *value },
            Self::Gate(value) | Self::Reverse(value) | Self::Fit(value) | Self::Loop(value) => {
                V::bool(*value)
            }
            Self::Slice { index, count } => V::Slice {
                index: *index,
                count: *count,
            },
            Self::Gain(value)
            | Self::Attack(value)
            | Self::Decay(value)
            | Self::Release(value)
            | Self::Transpose(value)
            | Self::Pan(value)
            | Self::Velocity(value)
            | Self::ClipLength(value)
            | Self::PostGain(value)
            | Self::PitchBend(value)
            | Self::Expression(value)
            | Self::HighPassCutoff(value)
            | Self::HighPassResonance(value)
            | Self::Legato(value)
            | Self::Sustain(value)
            | Self::LowPassCutoff(value)
            | Self::LowPassResonance(value)
            | Self::Late(value)
            | Self::Fast(value)
            | Self::Slow(value)
            | Self::PlaybackRate(value)
            | Self::PlaybackStart(value)
            | Self::PlaybackEnd(value) => V::rational(*value),
            Self::SampleBank(value) => V::symbol(value),
            Self::SampleVariant(value) => V::rational(Rational::from_integer(i64::from(*value))),
            _ => return None,
        })
    }

    /// Replaces only the operand; the modifier's role cannot change through editing.
    pub fn with_parameter_value(&self, value: super::FieldValue) -> Result<Self, &'static str> {
        use super::{FieldValue as V, ParameterKey as P};
        let key = self
            .parameter_key()
            .ok_or("This modifier has no editable catalog value.")?;
        key.spec().validate(&value)?;
        Ok(match (key, value) {
            (parameter, V::Modulation { value }) => Self::Modulation { parameter, value },
            (P::Delay, V::Delay { value }) => Self::Delay(value),
            (P::Reverb, V::Reverb { value }) => Self::Reverb(value),
            (P::Compressor, V::Compressor { value }) => Self::Compressor(value),

            (P::Gate, V::Bool { value }) => Self::Gate(value),
            (P::Gate, V::Rational { value }) => Self::Gate(!value.is_zero()),
            (P::Reverse, V::Bool { value }) => Self::Reverse(value),
            (P::Reverse, V::Rational { value }) => Self::Reverse(!value.is_zero()),
            (P::Fit, V::Bool { value }) => Self::Fit(value),
            (P::Fit, V::Rational { value }) => Self::Fit(!value.is_zero()),
            (P::Loop, V::Bool { value }) => Self::Loop(value),
            (P::Loop, V::Rational { value }) => Self::Loop(!value.is_zero()),
            (P::Slice, V::Slice { index, count }) => Self::Slice { index, count },
            (P::PlaybackRate, V::Rational { value }) => Self::PlaybackRate(value),
            (P::PlaybackStart, V::Rational { value }) => Self::PlaybackStart(value),
            (P::PlaybackEnd, V::Rational { value }) => Self::PlaybackEnd(value),
            (P::Gain, V::Rational { value }) => Self::Gain(value),
            (P::Attack, V::Rational { value }) => Self::Attack(value),
            (P::Decay, V::Rational { value }) => Self::Decay(value),
            (P::Release, V::Rational { value }) => Self::Release(value),
            (P::Transpose, V::Rational { value }) => Self::Transpose(value),
            (P::Pan, V::Rational { value }) => Self::Pan(value),
            (P::Velocity, V::Rational { value }) => Self::Velocity(value),
            (P::ClipLength, V::Rational { value }) => Self::ClipLength(value),
            (P::PostGain, V::Rational { value }) => Self::PostGain(value),
            (P::PitchBend, V::Rational { value }) => Self::PitchBend(value),
            (P::Expression, V::Rational { value }) => Self::Expression(value),

            (P::HighPassCutoff, V::Rational { value }) => Self::HighPassCutoff(value),
            (P::HighPassResonance, V::Rational { value }) => Self::HighPassResonance(value),
            (P::Legato, V::Rational { value }) => Self::Legato(value),
            (P::Sustain, V::Rational { value }) => Self::Sustain(value),
            (P::LowPassCutoff, V::Rational { value }) => Self::LowPassCutoff(value),
            (P::LowPassResonance, V::Rational { value }) => Self::LowPassResonance(value),
            (P::SampleBank, V::Symbol { value }) => Self::SampleBank(value),
            (P::SampleVariant, V::Rational { value }) => {
                Self::SampleVariant(value.numerator as u32)
            }
            (P::Fast, V::Rational { value }) => Self::Fast(value),
            (P::Late, V::Rational { value }) => Self::Late(value),
            (P::Slow, V::Rational { value }) => Self::Slow(value),
            _ => return Err("This value does not belong to this modifier role."),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MusicalValue {
    Effect(super::EffectValue),
    Note(NoteAtom),
    Sound(String),
    Rest,
    Scalar(ScalarAtom),
    NestedContainer(ContainerId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomExprKind {
    Value(MusicalValue),
    Choice(Vec<AtomExpr>),
    Parallel(Vec<AtomExpr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomExpr {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_node: Option<super::NodeId>,
    pub kind: AtomExprKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<AtomModifier>,
}
