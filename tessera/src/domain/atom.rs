use serde::{Deserialize, Serialize};
use std::fmt;

use super::container::ContainerId;
use super::pattern_ir::Rational;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct NoteAtom {
    pub value: NoteValue,
    /// Original note label from authoring; used to detect invalid spellings at normalize time.
    #[serde(default)]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub octave: Option<i64>,
}

impl NoteAtom {
    pub fn new(value: impl AsRef<str>) -> Self {
        let label = value.as_ref().to_string();
        Self {
            value: NoteValue::from(value.as_ref()),
            label,
            octave: None,
        }
    }

    pub fn with_octave(mut self, octave: i64) -> Self {
        self.octave = Some(octave);
        self
    }
}

pub fn try_parse_note_value(value: &str) -> Result<NoteValue, ()> {
    match value.to_ascii_lowercase().as_str() {
        "a" => Ok(NoteValue::A),
        "b" => Ok(NoteValue::B),
        "c" => Ok(NoteValue::C),
        "d" => Ok(NoteValue::D),
        "e" => Ok(NoteValue::E),
        "f" => Ok(NoteValue::F),
        "g" => Ok(NoteValue::G),
        _ => Err(()),
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
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
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
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
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
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
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum AtomTile {
    Note(NoteAtom),
    Rest,
    Scalar(ScalarAtom),
    Operator(AtomOperatorToken),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum AtomModifier {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum MusicalValue {
    Note(NoteAtom),
    Rest,
    Scalar(ScalarAtom),
    NestedContainer(ContainerId),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub enum AtomExprKind {
    Value(MusicalValue),
    Choice(Vec<AtomExpr>),
    Parallel(Vec<AtomExpr>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
pub struct AtomExpr {
    pub kind: AtomExprKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<AtomModifier>,
}
