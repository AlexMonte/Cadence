//! Repeating voice source model.

use crate::domain::{
    intent::Intent,
    prelude::Time,
    space::SpatialMotion,
    span::{Phase, Span},
};

/// Source-level voice transform helpers.
pub mod ops;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Stable identifier for a voice.
pub struct VoiceId(u64);

impl VoiceId {
    /// Creates a voice identifier from a raw integer.
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

impl From<u64> for VoiceId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// Stable identifier for a tile inside a voice.
pub struct TileId(u64);

impl TileId {
    /// Creates a tile identifier from a raw integer.
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

impl From<u64> for TileId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
/// Repetition policy for a source voice or control track.
pub enum Repeat {
    /// Repeat forever.
    #[default]
    Forever,
    /// Play a single period once.
    Once,
    /// Repeat for a fixed number of periods.
    Count(u32),
    /// Repeat until the transport reaches the given time.
    Until(Time),
}

#[derive(Debug, Clone, PartialEq)]
/// One phase-local intent inside a repeating voice period.
pub struct Tile {
    id: Option<TileId>,
    phase: Span<Phase>,
    intent: Intent,
    position: SpatialMotion,
}

impl Tile {
    /// Creates a tile inside a voice period.
    ///
    /// Returns `None` when the tile starts before phase zero. The position
    /// defaults to a static origin; attach a trajectory with
    /// [`Tile::with_position`].
    #[must_use]
    pub fn new(phase: Span<Phase>, intent: Intent) -> Option<Self> {
        (phase.start() >= Time::ZERO).then_some(Self {
            id: None,
            phase,
            intent,
            position: SpatialMotion::ORIGIN,
        })
    }

    /// Creates a tile from raw phase-local bounds.
    ///
    /// Returns `None` when the span is invalid or starts before phase zero.
    ///
    /// ```
    /// use cadence::domain::prelude::{Intent, Tile, Time};
    ///
    /// let tile = Tile::spanning(Time::ZERO, Time::new(1, 2), Intent::sample("kick")).unwrap();
    ///
    /// assert_eq!(tile.phase().start(), Time::ZERO);
    /// ```
    #[must_use]
    pub fn spanning(start: Time, end: Time, intent: Intent) -> Option<Self> {
        Some(Self::new(Span::new(start, end)?, intent)?)
    }

    /// Attaches a stable identifier to the tile.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<TileId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Attaches a spatial trajectory to the tile.
    #[must_use]
    pub fn with_position(mut self, position: SpatialMotion) -> Self {
        self.position = position;
        self
    }

    /// Returns the optional tile identifier.
    #[must_use]
    pub fn id(&self) -> Option<TileId> {
        self.id
    }

    /// Returns the spatial trajectory carried by the tile.
    #[must_use]
    pub fn position(&self) -> SpatialMotion {
        self.position
    }

    /// Returns the phase-local span occupied by the tile.
    #[must_use]
    pub fn phase(&self) -> Span<Phase> {
        self.phase
    }

    /// Returns the musical intent carried by the tile.
    #[must_use]
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Repeating source voice made of phase-local tiles.
pub struct Voice {
    id: Option<VoiceId>,
    period: Time,
    repeat: Repeat,
    tiles: Vec<Tile>,
}

impl Voice {
    /// Creates a repeating voice.
    ///
    /// Returns `None` when:
    /// - `period <= 0`
    /// - any tile starts before phase zero
    /// - any tile ends after the voice period
    ///
    /// Tiles are stored sorted by phase span.
    ///
    /// ```
    /// use cadence::domain::prelude::{Intent, Tile, Time, Voice};
    ///
    /// let voice = Voice::new(
    ///     Time::ONE,
    ///     vec![Tile::spanning(Time::ZERO, Time::new(1, 2), Intent::sample("kick")).unwrap()],
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(voice.period(), Time::ONE);
    /// ```
    #[must_use]
    pub fn new(period: Time, mut tiles: Vec<Tile>) -> Option<Self> {
        if period <= Time::ZERO {
            return None;
        }

        if tiles
            .iter()
            .any(|tile| tile.phase().start() < Time::ZERO || tile.phase().end() > period)
        {
            return None;
        }

        tiles.sort_by(|left, right| {
            left.phase()
                .start()
                .cmp(&right.phase().start())
                .then(left.phase().end().cmp(&right.phase().end()))
        });

        Some(Self {
            id: None,
            period,
            repeat: Repeat::Forever,
            tiles,
        })
    }

    /// Attaches a stable identifier to the voice.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<VoiceId>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Replaces the repetition policy.
    #[must_use]
    pub fn with_repeat(mut self, repeat: Repeat) -> Self {
        self.repeat = repeat;
        self
    }

    /// Returns the optional voice identifier.
    #[must_use]
    pub fn id(&self) -> Option<VoiceId> {
        self.id
    }

    /// Returns the length of one voice period.
    #[must_use]
    pub fn period(&self) -> Time {
        self.period
    }

    /// Returns how the voice repeats after one period.
    #[must_use]
    pub fn repeat(&self) -> Repeat {
        self.repeat
    }

    /// Returns the tiles in sorted phase order.
    #[must_use]
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// Iterates over tiles in sorted phase order.
    pub fn iter(&self) -> impl Iterator<Item = &Tile> {
        self.tiles.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_tile(start: (i64, i64), end: (i64, i64), sample: &str, id: u64) -> Tile {
        Tile::spanning(
            Time::new(start.0, start.1),
            Time::new(end.0, end.1),
            Intent::sample(sample),
        )
        .unwrap()
        .with_id(id)
    }

    #[test]
    fn tile_rejects_negative_phase_start() {
        let tile = Tile::spanning(Time::new(-1, 4), Time::new(1, 4), Intent::sample("kick"));

        assert!(tile.is_none());
    }

    #[test]
    fn voice_rejects_non_positive_period() {
        assert!(Voice::new(Time::ZERO, vec![]).is_none());
    }

    #[test]
    fn voice_rejects_tiles_that_escape_the_period() {
        let voice = Voice::new(
            Time::new(1, 2),
            vec![source_tile((0, 1), (1, 1), "kick", 1)],
        );

        assert!(voice.is_none());
    }

    #[test]
    fn voice_accepts_tiles_that_start_at_zero_and_end_at_the_period() {
        let voice = Voice::new(Time::ONE, vec![source_tile((0, 1), (1, 1), "kick", 1)]).unwrap();

        assert_eq!(
            voice.tiles()[0].phase(),
            Span::new(Time::ZERO, Time::ONE).unwrap()
        );
    }

    #[test]
    fn voice_defaults_to_forever_repeat_and_sorts_tiles() {
        let late = source_tile((1, 2), (3, 4), "snare", 2);
        let early = source_tile((0, 1), (1, 4), "kick", 1);
        let voice = Voice::new(Time::ONE, vec![late.clone(), early.clone()])
            .unwrap()
            .with_id(9_u64);

        assert_eq!(voice.id(), Some(VoiceId::new(9)));
        assert_eq!(voice.repeat(), Repeat::Forever);
        assert_eq!(voice.tiles()[0], early);
        assert_eq!(voice.tiles()[1], late);
    }
}
