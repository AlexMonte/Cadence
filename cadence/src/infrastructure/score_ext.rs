//! Free-function helpers and fluent extensions for score trees.

use crate::domain::{
    control::{ControlKey, ControlTrack},
    prelude::Time,
    rational::Coord,
    score::{ControlScore, Score, WeightedScore},
    signal::Signal,
    space::Point3,
};

/// Default deterministic seed used by [`PatternExt::sometimes_by`].
const SOMETIMES_SEED: u64 = 0x5A7E_C0DE;

/// Simultaneously layers child scores.
#[must_use]
pub fn merge(scores: Vec<Score>) -> Score {
    Score::merge(scores)
}

/// Appends child scores sequentially in transport-time order.
#[must_use]
pub fn concat(scores: Vec<Score>) -> Score {
    Score::concat(scores)
}

/// Alias for [`merge`], matching Tidal/Strudel "stack" vocabulary.
#[must_use]
pub fn stack(scores: Vec<Score>) -> Score {
    merge(scores)
}

/// Slows a score by `factor` (for example `Time::new(2, 1)` doubles cycle length).
///
/// # Panics
///
/// Panics if `factor <= 0`.
#[must_use]
pub fn slow(score: Score, factor: Time) -> Score {
    assert!(factor > Time::ZERO, "slow factor must be positive");
    Score::time_scale(score, Time::ONE / factor)
}

/// Speeds a score by `factor`.
///
/// # Panics
///
/// Panics if `factor <= 0`.
#[must_use]
pub fn fast(score: Score, factor: Time) -> Score {
    Score::time_scale(score, factor)
}

/// Shifts a score in transport time.
#[must_use]
pub fn shift(score: Score, offset: Time) -> Score {
    Score::shift(score, offset)
}

/// Reflects a score inside one cycle.
#[must_use]
pub fn reflect(score: Score) -> Score {
    Score::reflect_cycle(score)
}

/// Applies a control tree to a source score.
#[must_use]
pub fn with_controls(source: Score, controls: ControlScore) -> Score {
    Score::with_controls(source, controls)
}

/// Attaches a continuous [`Signal`] to one control lane of `source`.
///
/// This builds a one-cycle signal control track for `key` and applies it via
/// [`Score::with_controls`], so the signal is carried unchanged through
/// projection and evaluated per audio frame on the audio thread.
///
/// # Panics
///
/// Panics if `key` does not accept a signal value (the modulatable lanes are
/// `Gain`, `PlaybackRate`, and `LowPassCutoff`).
#[must_use]
pub fn with_signal(source: Score, key: ControlKey, signal: Signal) -> Score {
    let track = ControlTrack::from_signal(key, signal)
        .expect("signal control lane must accept a signal value");
    Score::with_controls(source, ControlScore::track(track))
}

/// Fluent score transforms for chaining.
pub trait ScoreExt {
    /// Appends this score after `other` in transport-time order.
    #[must_use]
    fn concat(self, other: Score) -> Score;

    /// Layers this score with `other`.
    #[must_use]
    fn merge(self, other: Score) -> Score;

    /// Slows this score by `factor`.
    #[must_use]
    fn slow(self, factor: Time) -> Score;

    /// Speeds this score by `factor`.
    #[must_use]
    fn fast(self, factor: Time) -> Score;

    /// Shifts this score in transport time.
    #[must_use]
    fn shift(self, offset: Time) -> Score;

    /// Reflects this score inside one cycle.
    #[must_use]
    fn reflect(self) -> Score;

    /// Applies a control tree to this score.
    #[must_use]
    fn with_controls(self, controls: ControlScore) -> Score;

    /// Attaches a continuous [`Signal`] to one control lane of this score.
    #[must_use]
    fn with_signal(self, key: ControlKey, signal: Signal) -> Score;
}

impl ScoreExt for Score {
    fn concat(self, other: Score) -> Score {
        crate::infrastructure::score_ext::concat(vec![self, other])
    }

    fn merge(self, other: Score) -> Score {
        crate::infrastructure::score_ext::merge(vec![self, other])
    }

    fn slow(self, factor: Time) -> Score {
        slow(self, factor)
    }

    fn fast(self, factor: Time) -> Score {
        fast(self, factor)
    }

    fn shift(self, offset: Time) -> Score {
        shift(self, offset)
    }

    fn reflect(self) -> Score {
        reflect(self)
    }

    fn with_controls(self, controls: ControlScore) -> Score {
        with_controls(self, controls)
    }

    fn with_signal(self, key: ControlKey, signal: Signal) -> Score {
        with_signal(self, key, signal)
    }
}

/// Places a score hard to one lateral side of the lattice.
///
/// `lateral` is the `x` offset applied to the child's spatial position (for
/// example `-1` for hard-left, `1` for hard-right). This is the spatial twin of
/// the old stereo hard-pan: position is first-class, so `jux` spreads material
/// through space rather than writing a pan control lane.
fn place_lateral(score: Score, lateral: i64) -> Score {
    Score::space_shift(
        score,
        Point3::new(Coord::whole_number(lateral), Coord::ZERO, Coord::ZERO),
    )
}

fn probability_weights(probability: f64) -> (Time, Time) {
    const DENOMINATOR: i64 = 1_000_000;
    let clamped = probability.clamp(0.0, 1.0);
    let numerator = ((clamped * DENOMINATOR as f64).round() as i64).clamp(1, DENOMINATOR - 1);
    (
        Time::new(numerator, DENOMINATOR),
        Time::new(DENOMINATOR - numerator, DENOMINATOR),
    )
}

/// Higher-order pattern combinators that lower to the existing [`Score`] IR.
///
/// These mirror Strudel/Tidal authoring verbs. Each `f` is a structural
/// transform `Fn(Score) -> Score`; combinators apply it across cycles or layer
/// it alongside the base score, lowering to `CycleRoute`, `Merge`, or
/// `WeightedChoice` so the engine can query them with no new evaluator code.
pub trait PatternExt {
    /// Applies `f` on every `n`-th cycle, leaving the other cycles untouched.
    ///
    /// Lowers to a `CycleRoute` of length `n` where slot `0` is `f(self)` and
    /// the remaining slots are the unchanged base score. `n == 0` is a no-op.
    #[must_use]
    fn every<F: Fn(Score) -> Score>(self, n: usize, f: F) -> Score;

    /// Applies `f` on the cycles `b..a` of every `a`-cycle group.
    ///
    /// Lowers to a `CycleRoute` of length `a` where slots `b..a` are `f(self)`
    /// and slots `0..b` are the unchanged base score. `a == 0` is a no-op.
    #[must_use]
    fn whenmod<F: Fn(Score) -> Score>(self, a: usize, b: usize, f: F) -> Score;

    /// Layers `self` with `f(self)`, lowering to a `Merge`.
    #[must_use]
    fn superimpose<F: Fn(Score) -> Score>(self, f: F) -> Score;

    /// Layers `self` panned hard-left with `f(self)` panned hard-right.
    ///
    /// Lowers to a `Merge` of the two pan-controlled children.
    #[must_use]
    fn jux<F: Fn(Score) -> Score>(self, f: F) -> Score;

    /// Replaces `self` with `f(self)` on a deterministic fraction of cycles.
    ///
    /// Lowers to a per-cycle `WeightedChoice` between `f(self)` (weight
    /// `probability`) and `self` (weight `1 - probability`).
    ///
    /// Note: Strudel's `sometimesBy` is per-event; this implementation is
    /// per-cycle. A per-event variant is a planned follow-up.
    #[must_use]
    fn sometimes_by<F: Fn(Score) -> Score>(self, probability: f64, f: F) -> Score;
}

impl PatternExt for Score {
    fn every<F: Fn(Score) -> Score>(self, n: usize, f: F) -> Score {
        if n == 0 {
            return self;
        }
        let base = self;
        let mut children = Vec::with_capacity(n);
        children.push(f(base.clone()));
        children.resize(n, base);
        Score::cycle_route(children)
    }

    fn whenmod<F: Fn(Score) -> Score>(self, a: usize, b: usize, f: F) -> Score {
        if a == 0 {
            return self;
        }
        let base = self;
        let children = (0..a)
            .map(|index| {
                if index >= b {
                    f(base.clone())
                } else {
                    base.clone()
                }
            })
            .collect();
        Score::cycle_route(children)
    }

    fn superimpose<F: Fn(Score) -> Score>(self, f: F) -> Score {
        let base = self;
        Score::merge(vec![base.clone(), f(base)])
    }

    fn jux<F: Fn(Score) -> Score>(self, f: F) -> Score {
        let base = self;
        let left = place_lateral(base.clone(), -1);
        let right = place_lateral(f(base), 1);
        Score::merge(vec![left, right])
    }

    fn sometimes_by<F: Fn(Score) -> Score>(self, probability: f64, f: F) -> Score {
        let base = self;
        let (apply_weight, keep_weight) = probability_weights(probability);
        Score::weighted_choice(
            vec![
                WeightedScore::new(f(base.clone()), apply_weight),
                WeightedScore::new(base, keep_weight),
            ],
            SOMETIMES_SEED,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::stack::{cycle, sample, tile};

    #[test]
    fn slow_doubles_cycle_length() {
        let base = Score::from(cycle(vec![tile(Time::ZERO, Time::new(1, 2), sample("bd"))]));
        let slowed = slow(base, Time::new(2, 1));
        assert!(matches!(
            slowed.kind(),
            crate::domain::score::ScoreKind::TimeScale { .. }
        ));
    }

    #[test]
    fn score_ext_chains_slow() {
        let base = Score::from(cycle(vec![tile(Time::ZERO, Time::ONE, sample("hh"))]));
        let chained = base.slow(Time::new(2, 1));
        assert!(matches!(
            chained.kind(),
            crate::domain::score::ScoreKind::TimeScale { .. }
        ));
    }

    fn base_score() -> Score {
        Score::from(cycle(vec![tile(Time::ZERO, Time::ONE, sample("bd"))]))
    }

    #[test]
    fn every_lowers_to_cycle_route_with_n_children() {
        let routed = base_score().every(4, |s| s.fast(Time::new(2, 1)));
        match routed.kind() {
            crate::domain::score::ScoreKind::CycleRoute(children) => {
                assert_eq!(children.len(), 4);
                // Slot 0 is transformed (TimeScale), the rest are the base.
                assert!(matches!(
                    children[0].kind(),
                    crate::domain::score::ScoreKind::TimeScale { .. }
                ));
                assert!(matches!(
                    children[1].kind(),
                    crate::domain::score::ScoreKind::Voice { .. }
                ));
            }
            other => panic!("expected CycleRoute, got {other:?}"),
        }
    }

    #[test]
    fn whenmod_lowers_to_cycle_route_with_transform_in_tail_slots() {
        let routed = base_score().whenmod(4, 2, |s| s.fast(Time::new(2, 1)));
        match routed.kind() {
            crate::domain::score::ScoreKind::CycleRoute(children) => {
                assert_eq!(children.len(), 4);
                assert!(matches!(
                    children[0].kind(),
                    crate::domain::score::ScoreKind::Voice { .. }
                ));
                assert!(matches!(
                    children[2].kind(),
                    crate::domain::score::ScoreKind::TimeScale { .. }
                ));
            }
            other => panic!("expected CycleRoute, got {other:?}"),
        }
    }

    #[test]
    fn superimpose_lowers_to_merge_of_two() {
        let merged = base_score().superimpose(|s| s.fast(Time::new(2, 1)));
        match merged.kind() {
            crate::domain::score::ScoreKind::Merge(children) => assert_eq!(children.len(), 2),
            other => panic!("expected Merge, got {other:?}"),
        }
    }

    #[test]
    fn jux_lowers_to_merge_of_two_space_shifted_children() {
        let merged = base_score().jux(|s| s.fast(Time::new(2, 1)));
        match merged.kind() {
            crate::domain::score::ScoreKind::Merge(children) => {
                assert_eq!(children.len(), 2);
                for child in children {
                    assert!(matches!(
                        child.kind(),
                        crate::domain::score::ScoreKind::SpaceShift { .. }
                    ));
                }
            }
            other => panic!("expected Merge, got {other:?}"),
        }
    }

    #[test]
    fn sometimes_by_lowers_to_weighted_choice() {
        let chosen = base_score().sometimes_by(0.5, |s| s.fast(Time::new(2, 1)));
        match chosen.kind() {
            crate::domain::score::ScoreKind::WeightedChoice { options, .. } => {
                assert_eq!(options.len(), 2);
            }
            other => panic!("expected WeightedChoice, got {other:?}"),
        }
    }

    #[test]
    fn with_signal_wraps_in_with_controls() {
        let signal = Signal::sine().with_rate(Time::whole_number(2));
        let modulated = base_score().with_signal(ControlKey::Gain, signal);
        assert!(matches!(
            modulated.kind(),
            crate::domain::score::ScoreKind::WithControls { .. }
        ));
    }
}
