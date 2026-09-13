//! Compact timed source uses. Repetitions are queried, never eagerly expanded.
use crate::domain::{
    rational::Time,
    score::{ControlScore, Score},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
/// An arrangement cannot be represented within the supported exact-time bounds.
pub enum ArrangementError {
    /// Durations must be positive, at most one million cycles, denominator at most one million.
    #[error(
        "arrangement duration must be positive, at most 1000000 cycles, with denominator at most 1000000"
    )]
    InvalidDuration,
    /// Every use must occur at least once.
    #[error("arrangement repeats must be positive")]
    InvalidRepeats,
    /// An arrangement needs between one and 1024 segments.
    #[error("arrangement requires between 1 and 1024 segments")]
    SegmentLimit,
    /// Total length must fit the same exact bounds as an individual duration.
    #[error("arrangement total duration exceeds its exact-time bounds")]
    DurationLimit,
}

#[derive(Debug, Clone, PartialEq)]
/// A source used for a fixed duration, restarting at local zero on each repeat.
pub struct Timed<T> {
    source: T,
    duration: Time,
    repeats: u32,
}

/// Timed source score; each repeat preserves its internal speed.
pub type TimedScore = Timed<Score>;
/// Timed control score with the same local-clock reset as a source score.
pub type TimedControlScore = Timed<ControlScore>;

impl<T> Timed<T> {
    /// Validates one compact source use without expanding its repetitions.
    pub fn new(source: T, duration: Time, repeats: u32) -> Result<Self, ArrangementError> {
        if duration.numerator() <= 0
            || duration.denominator() > 1_000_000
            || duration.value() > 1_000_000.0
        {
            return Err(ArrangementError::InvalidDuration);
        }
        if repeats == 0 {
            return Err(ArrangementError::InvalidRepeats);
        }
        let result = Self {
            source,
            duration,
            repeats,
        };
        period(std::slice::from_ref(&result))?;
        Ok(result)
    }
    /// Source queried at local time zero for each occurrence.
    pub fn source(&self) -> &T {
        &self.source
    }
    /// Duration of one occurrence, in cycles.
    pub fn duration(&self) -> Time {
        self.duration
    }
    /// Number of local-clock restarts.
    pub fn repeats(&self) -> u32 {
        self.repeats
    }
}

pub(crate) fn period<T>(segments: &[Timed<T>]) -> Result<Time, ArrangementError> {
    if segments.is_empty() || segments.len() > 1024 {
        return Err(ArrangementError::SegmentLimit);
    }
    let (mut num, mut den) = (0_i128, 1_i128);
    for segment in segments {
        let next_num = i128::from(segment.duration.numerator()) * i128::from(segment.repeats);
        let next_den = i128::from(segment.duration.denominator());
        num = num * next_den + next_num * den;
        den *= next_den;
        let (mut a, mut b) = (num, den);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        num /= a;
        den /= a;
        if den > 1_000_000 || num > den * 1_000_000 {
            return Err(ArrangementError::DurationLimit);
        }
    }
    Ok(Time::new(num as i64, den as i64))
}
