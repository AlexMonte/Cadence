//! Typed musical time spans.

use std::{cmp::Ordering, marker::PhantomData};

use crate::domain::rational::Time;

mod sealed {
    pub trait TimeSpace {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// Marker for phase-local time inside one repeating voice or control period.
pub struct Phase;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// Marker for transport time after source material has been placed on the
/// global timeline.
pub struct Transport;

impl sealed::TimeSpace for Phase {}
impl sealed::TimeSpace for Transport {}

/// Marker trait implemented by supported musical time spaces.
pub trait TimeSpace: sealed::TimeSpace {}

impl TimeSpace for Phase {}
impl TimeSpace for Transport {}

/// Represents a half-open interval in a particular musical time space.
///
/// `Span` defaults to transport time. Use `Span<Phase>` for tile-local spans
/// inside a repeating voice period.
///
/// ```compile_fail
/// use cadence::domain::{rational::Time, span::{Phase, Span}};
///
/// let phase_span: Span<Phase> = Span::new(Time::ZERO, Time::ONE).unwrap();
/// let transport_span: Span = Span::new(Time::ZERO, Time::ONE).unwrap();
///
/// let _ = phase_span.intersection(&transport_span);
/// ```
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Span<Space = Transport>
where
    Space: PartialEq + Eq,
{
    start: Time,
    end: Time,
    _space: PhantomData<fn() -> Space>,
}

impl<Space> Span<Space>
where
    Space: PartialEq + Eq,
{
    /// Canonical one-cycle span `[0, 1)`.
    pub const CYCLE: Self = Self {
        start: Time::ZERO,
        end: Time::ONE,
        _space: PhantomData,
    };

    /// Creates a half-open span `[start, end)`.
    ///
    /// Returns `None` when `start >= end`.
    ///
    /// ```
    /// use cadence::domain::prelude::{Span, Time, TransportSpan};
    ///
    /// let span: TransportSpan = Span::new(Time::ZERO, Time::ONE).unwrap();
    ///
    /// assert_eq!(span.start(), Time::ZERO);
    /// assert_eq!(span.end(), Time::ONE);
    /// ```
    #[must_use]
    pub fn new(start: Time, end: Time) -> Option<Self> {
        (start < end).then_some(Self {
            start,
            end,
            _space: PhantomData,
        })
    }

    /// Returns the inclusive lower bound of the span.
    #[must_use]
    pub fn start(&self) -> Time {
        self.start
    }

    /// Returns the exclusive upper bound of the span.
    #[must_use]
    pub fn end(&self) -> Time {
        self.end
    }

    /// Returns the earlier of this span's end and another piece boundary.
    ///
    /// This is mainly a convenience helper for clipping work.
    #[must_use]
    pub fn piece_end(&self, other_piece: Time) -> Time {
        self.end.min(other_piece)
    }

    /// Returns `true` when `rhs` lies entirely inside `self`.
    #[must_use]
    pub fn contains(&self, rhs: &Self) -> bool {
        self.start <= rhs.start && rhs.end <= self.end
    }

    /// Returns `true` when the half-open spans overlap.
    ///
    /// Touching at a boundary does not count as intersection.
    #[must_use]
    pub fn intersects(&self, rhs: &Self) -> bool {
        self.start < rhs.end && rhs.start < self.end
    }

    /// Returns the overlapping part of two spans, if any.
    #[must_use]
    pub fn intersection(&self, rhs: &Self) -> Option<Self> {
        self.intersects(rhs)
            .then(|| Self::new(self.start.max(rhs.start), self.end.min(rhs.end)).unwrap())
    }

    /// Shifts the span by `offset`, optionally changing the time-space marker.
    ///
    /// This does not convert units. It is a type-level relabel plus exact
    /// translation, which is useful when moving a phase span into transport
    /// time.
    ///
    /// ```
    /// use cadence::domain::prelude::{Phase, Span, Time, Transport};
    ///
    /// let phase: Span<Phase> = Span::new(Time::new(1, 4), Time::new(1, 2)).unwrap();
    /// let placed: Span<Transport> = phase.translate(Time::ONE);
    ///
    /// assert_eq!(placed.start(), Time::new(5, 4));
    /// ```
    #[must_use]
    pub fn translate<OtherSpace: PartialEq + Eq>(&self, offset: Time) -> Span<OtherSpace> {
        Span::<OtherSpace>::new(self.start + offset, self.end + offset)
            .expect("translated span must remain valid")
    }
}

impl<Space> Ord for Span<Space>
where
    Space: PartialEq + Eq,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.start.cmp(&other.start).then(self.end.cmp(&other.end))
    }
}

impl<Space> PartialOrd for Span<Space>
where
    Space: PartialEq + Eq,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Phase-local span alias.
pub type PhaseSpan = Span<Phase>;
/// Transport-time span alias.
pub type TransportSpan = Span<Transport>;

#[cfg(test)]
mod tests {
    use super::*;

    fn span<Space: PartialEq + Eq>(s: (i64, i64), e: (i64, i64)) -> Span<Space> {
        Span::new(Time::new(s.0, s.1), Time::new(e.0, e.1)).unwrap()
    }

    #[test]
    fn transport_spans_intersect_as_expected() {
        let source = span::<Transport>((0, 1), (3, 4));
        let window = span::<Transport>((1, 2), (1, 1));

        assert_eq!(source.intersection(&window), Some(span((1, 2), (3, 4))));
    }

    #[test]
    fn phase_spans_translate_into_transport_spans() {
        let phase = span::<Phase>((1, 4), (1, 2));
        let transport = phase.translate::<Transport>(Time::ONE);

        assert_eq!(transport, span::<Transport>((5, 4), (3, 2)));
    }
}
