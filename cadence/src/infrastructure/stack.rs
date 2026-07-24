//! Free-function helpers for building voices and intents.

use crate::domain::{
    intent::{BuiltInSynthSource, Intent},
    prelude::Time,
    voice::{Tile, Voice},
};

/// Creates a sample intent.
#[must_use]
pub fn sample(sample_id: impl Into<String>) -> Intent {
    Intent::sample(sample_id)
}

/// Creates a synth intent.
#[must_use]
pub fn synth(source: BuiltInSynthSource) -> Intent {
    Intent::synth(source)
}

/// Creates a phase-local tile.
///
/// # Panics
///
/// Panics if the span is invalid.
#[must_use]
pub fn tile(start: Time, end: Time, intent: Intent) -> Tile {
    Tile::spanning(start, end, intent).expect("tile span must be valid")
}

/// Creates a one-cycle repeating voice from tiles.
///
/// # Panics
///
/// Panics if the tiles do not fit one cycle.
#[must_use]
pub fn cycle(tiles: Vec<Tile>) -> Voice {
    Voice::new(Time::ONE, tiles).expect("cycle voice must be valid")
}

/// Creates a repeating voice with an explicit period.
///
/// # Panics
///
/// Panics if the voice is invalid.
#[must_use]
pub fn voice(period: Time, tiles: Vec<Tile>) -> Voice {
    Voice::new(period, tiles).expect("voice must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::score::Score;

    #[test]
    fn cycle_voice_lowers_to_score() {
        let v = cycle(vec![tile(Time::ZERO, Time::new(1, 2), sample("bd"))]);
        let score = Score::from(v);
        assert!(matches!(
            score.kind(),
            crate::domain::score::ScoreKind::Voice(_)
        ));
    }
}
