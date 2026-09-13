//! Owned harmonic and rhythm settings. These lower into ordinary notes and cycle routes.
use serde::{Deserialize, Serialize};

use super::{NoteAtom, Rational, SignedAccidental};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScaleMode {
    #[default]
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    MajorPentatonic,
    MinorPentatonic,
    Chromatic,
}

impl ScaleMode {
    pub const ALL: [Self; 10] = [
        Self::Major,
        Self::Minor,
        Self::Dorian,
        Self::Phrygian,
        Self::Lydian,
        Self::Mixolydian,
        Self::Locrian,
        Self::MajorPentatonic,
        Self::MinorPentatonic,
        Self::Chromatic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Major => "Major",
            Self::Minor => "Natural minor",
            Self::Dorian => "Dorian",
            Self::Phrygian => "Phrygian",
            Self::Lydian => "Lydian",
            Self::Mixolydian => "Mixolydian",
            Self::Locrian => "Locrian",
            Self::MajorPentatonic => "Major pentatonic",
            Self::MinorPentatonic => "Minor pentatonic",
            Self::Chromatic => "Chromatic",
        }
    }

    pub fn intervals(self) -> &'static [i64] {
        match self {
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Self::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Self::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Self::MajorPentatonic => &[0, 2, 4, 7, 9],
            Self::MinorPentatonic => &[0, 3, 5, 7, 10],
            Self::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }

    /// Compact tile caption; the inspector uses the full label.
    pub fn short_label(self) -> &'static str {
        match self {
            Self::Major => "maj",
            Self::Minor => "min",
            Self::Dorian => "dor",
            Self::Phrygian => "phr",
            Self::Lydian => "lyd",
            Self::Mixolydian => "mix",
            Self::Locrian => "loc",
            Self::MajorPentatonic => "M5",
            Self::MinorPentatonic => "m5",
            Self::Chromatic => "chr",
        }
    }
}

/// Degree zero is the root. Negative degrees descend through the same scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleParameters {
    /// MIDI pitch, including octave (C4 = 60).
    pub root: u8,
    pub mode: ScaleMode,
}

impl Default for ScaleParameters {
    fn default() -> Self {
        Self {
            root: 60,
            mode: ScaleMode::Major,
        }
    }
}

impl ScaleParameters {
    pub fn validate(self) -> Result<(), &'static str> {
        if self.root > 127 {
            Err("Scale root must be a MIDI pitch from 0 to 127.")
        } else {
            Ok(())
        }
    }

    pub fn pitch(self, degree: Rational) -> Result<u8, &'static str> {
        self.validate()?;
        if degree.denominator != 1 {
            return Err("Scale degrees must be whole numbers.");
        }
        let intervals = self.mode.intervals();
        let count = intervals.len() as i64;
        let pitch = degree
            .numerator
            .div_euclid(count)
            .checked_mul(12)
            .and_then(|octaves| octaves.checked_add(i64::from(self.root)))
            .and_then(|root| {
                root.checked_add(intervals[degree.numerator.rem_euclid(count) as usize])
            })
            .filter(|pitch| (0..=127).contains(pitch))
            .ok_or("Scale degree is outside the playable MIDI range (0 to 127).")?;
        Ok(pitch as u8)
    }

    pub fn note(self, degree: Rational) -> Result<NoteAtom, &'static str> {
        let pitch = self.pitch(degree)?;
        let (name, sharp) = match pitch % 12 {
            0 => ("c", false),
            1 => ("c", true),
            2 => ("d", false),
            3 => ("d", true),
            4 => ("e", false),
            5 => ("f", false),
            6 => ("f", true),
            7 => ("g", false),
            8 => ("g", true),
            9 => ("a", false),
            10 => ("a", true),
            _ => ("b", false),
        };
        let note = NoteAtom::new(name).with_octave(i64::from(pitch / 12) - 1);
        Ok(if sharp {
            note.with_accidental(SignedAccidental::Sharp)
        } else {
            note
        })
    }

    pub fn root_label(self) -> String {
        let names = [
            "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
        ];
        format!(
            "{}{}",
            names[usize::from(self.root % 12)],
            i16::from(self.root / 12) - 1
        )
    }
}

/// Each list advances once per local cycle and wraps independently.
/// The combined repeat period is bounded so authored patterns cannot explode at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EuclidPatternParameters {
    pub pulses: Vec<u32>,
    pub steps: Vec<u32>,
    pub rotations: Vec<i32>,
}

impl Default for EuclidPatternParameters {
    fn default() -> Self {
        Self {
            pulses: vec![3, 1],
            steps: vec![8],
            rotations: vec![0, 2],
        }
    }
}

impl EuclidPatternParameters {
    pub const MAX_PERIOD: usize = 128;
    pub fn period(&self) -> Result<usize, &'static str> {
        let mut period = 1usize;
        for length in [self.pulses.len(), self.steps.len(), self.rotations.len()] {
            if length == 0 || length > Self::MAX_PERIOD {
                return Err("Each Euclidean input needs 1 to 128 cycle values.");
            }
            let (mut a, mut b) = (period, length);
            while b != 0 {
                (a, b) = (b, a % b);
            }
            period = period / a * length;
            if period > Self::MAX_PERIOD {
                return Err("The Euclidean input lists must repeat together within 128 cycles.");
            }
        }
        Ok(period)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        for cycle in 0..self.period()? {
            let (pulses, steps, _) = self.at_cycle(cycle);
            if steps == 0 || steps > 1024 || pulses > steps {
                return Err(
                    "Every Euclidean cycle needs 1 to 1024 steps and no more pulses than steps.",
                );
            }
        }
        Ok(())
    }

    /// Read after validation, which guarantees nonempty lists.
    pub(crate) fn at_cycle(&self, cycle: usize) -> (u32, u32, i32) {
        (
            self.pulses[cycle % self.pulses.len()],
            self.steps[cycle % self.steps.len()],
            self.rotations[cycle % self.rotations.len()],
        )
    }
}
