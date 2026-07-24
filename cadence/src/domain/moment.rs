//! Thin transport-time musical events.

use crate::domain::{intent::Intent, prelude::Time, space::SpatialMotion, span::Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Stable identifier for a projected or transport-time moment.
pub struct MomentId(u64);

impl MomentId {
    /// Creates a moment identifier from a raw integer.
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

impl From<u64> for MomentId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq)]
/// One transport-time event with a span, typed intent, and spatial position.
pub struct Moment {
    id: Option<MomentId>,
    span: Span,
    intent: Intent,
    position: SpatialMotion,
}

impl Moment {
    /// Creates a moment over an already-validated transport span.
    ///
    /// The position defaults to a static origin; attach a trajectory with
    /// [`Moment::with_position`].
    #[must_use]
    pub fn new(span: Span, intent: Intent) -> Self {
        Self {
            id: None,
            span,
            intent,
            position: SpatialMotion::ORIGIN,
        }
    }

    /// Creates a moment from raw transport-time bounds.
    ///
    /// Returns `None` when `start >= end`.
    #[must_use]
    pub fn spanning(start: Time, end: Time, intent: Intent) -> Option<Self> {
        Some(Self::new(Span::new(start, end)?, intent))
    }

    /// Attaches a stable identity to the moment.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<MomentId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Attaches a spatial trajectory to the moment.
    #[must_use]
    pub fn with_position(mut self, position: SpatialMotion) -> Self {
        self.position = position;
        self
    }

    /// Returns the optional stable moment identifier.
    #[must_use]
    pub fn id(&self) -> Option<MomentId> {
        self.id
    }

    /// Returns the transport span occupied by the moment.
    #[must_use]
    pub fn span(&self) -> Span {
        self.span
    }

    /// Returns the musical intent carried by the moment.
    #[must_use]
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    /// Returns the spatial trajectory carried by the moment.
    #[must_use]
    pub fn position(&self) -> SpatialMotion {
        self.position
    }

    /// Returns the exact duration of the moment.
    #[must_use]
    pub fn duration(&self) -> Time {
        self.span.end() - self.span.start()
    }

    /// Returns whether the moment occupies positive time.
    ///
    /// Valid moments always have positive duration because zero-length spans are
    /// rejected on construction.
    #[must_use]
    pub fn is_sustained(&self) -> bool {
        self.duration() > Time::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::intent::SampleIntent;

    #[test]
    fn spanning_rejects_zero_length_spans() {
        let moment = Moment::spanning(
            Time::ZERO,
            Time::ZERO,
            Intent::Sample(SampleIntent::new("kick")),
        );

        assert!(moment.is_none());
    }

    #[test]
    fn spanning_rejects_negative_spans() {
        let moment = Moment::spanning(
            Time::ONE,
            Time::ZERO,
            Intent::Sample(SampleIntent::new("kick")),
        );

        assert!(moment.is_none());
    }

    #[test]
    fn sustained_moment_preserves_its_intent_and_duration() {
        let moment = Moment::spanning(Time::new(1, 4), Time::new(3, 4), Intent::gate("mute", true))
            .unwrap()
            .with_id(7_u64);

        assert_eq!(moment.id(), Some(MomentId::new(7)));
        assert_eq!(moment.duration(), Time::new(1, 2));
        assert!(moment.is_sustained());
        assert!(matches!(moment.intent(), Intent::Gate(_)));
    }
}
