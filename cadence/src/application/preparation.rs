//! Conservative preparation budgets. Reject extreme projection work before a
//! large event list is allocated; authored musical values are never clamped.
use crate::domain::{
    arrangement::Timed,
    score::{ControlScore, ControlScoreKind, Score, ScoreKind},
};
use thiserror::Error;

// Stored arrangements may contain many mutually exclusive sections.
// This independent bound caps validation cost; MAX_WORK still caps active work.
const MAX_NODES: usize = 65_536;
const MAX_DEPTH: usize = 64;
const MAX_WORK: f64 = 16_384.0;

#[derive(Debug, Error)]
/// The score exceeds the supported audio preparation budget.
pub enum AudioScheduleError {
    /// The score has too many nested or repeated structural nodes.
    #[error(
        "audio pattern exceeds the preparation structure limit (65536 stored nodes, 64 nesting levels)"
    )]
    StructureLimit,
    /// Rates, repeats, or control combinations would produce excessive work.
    #[error(
        "audio pattern exceeds the preparation budget; reduce event density or the combined speed modifiers"
    )]
    ProjectionLimit,
}

#[derive(Debug, Clone, PartialEq)]
/// A score whose structure and one-cycle projection cost are safe for audio.
///
/// The wrapped score is private so playback can trust that the bounded
/// preparation check has already run. Cloning this value is cheap because a
/// [`Score`] shares its immutable tree.
pub struct PreparedScore(Score);

impl PreparedScore {
    /// Checks `score` once and returns the playback-ready value.
    pub fn new(score: Score) -> Result<Self, AudioScheduleError> {
        budget(score_work(&score, 1.0, false, 0, &mut 0)?)?;
        Ok(Self(score))
    }

    /// Borrows the checked score inside Cadence.
    #[must_use]
    pub(crate) fn score(&self) -> &Score {
        &self.0
    }

    /// Consumes the preparation proof inside Cadence.
    #[must_use]
    pub(crate) fn into_score(self) -> Score {
        self.0
    }
}

impl Default for PreparedScore {
    fn default() -> Self {
        Self::new(Score::empty()).expect("an empty score always fits the preparation budget")
    }
}

/// Reports the conservative one-cycle preparation cost without projecting events.
/// Stored-structure and depth limits still apply. Used for host diagnostics.
pub fn estimate_audio_preparation(score: &Score) -> Result<f64, AudioScheduleError> {
    score_work(score, 1.0, false, 0, &mut 0)
}

/// Seeking into held notes recovers onset controls over a complete source cycle.
/// This extra work is independent of the (possibly tiny) visible query window.
pub(crate) fn validate_onset_control_recovery(
    controls: &ControlScore,
    queries: usize,
) -> Result<(), AudioScheduleError> {
    if queries == 0 {
        return Ok(());
    }
    let work = control_work(controls, 1.0, false, 0, &mut 0)?;
    budget(work.max(1.0) * queries as f64)
}

fn enter(width: f64, depth: usize, nodes: &mut usize) -> Result<(), AudioScheduleError> {
    *nodes += 1;
    if *nodes > MAX_NODES || depth > MAX_DEPTH {
        return Err(AudioScheduleError::StructureLimit);
    }
    budget(width)
}
fn budget(work: f64) -> Result<(), AudioScheduleError> {
    if !work.is_finite() || work < 0.0 || work > MAX_WORK {
        Err(AudioScheduleError::ProjectionLimit)
    } else {
        Ok(())
    }
}
fn sum(
    values: impl Iterator<Item = Result<f64, AudioScheduleError>>,
) -> Result<f64, AudioScheduleError> {
    let mut work = 0.0;
    for value in values {
        work += value?;
    }
    Ok(work)
}
/// Only intersecting occurrences are queried. All sources are still validated,
/// but mutually exclusive sections must not consume simultaneous event budget.
fn arrangement_work<T>(
    segments: &[Timed<T>],
    period: f64,
    width: f64,
    mut source_work: impl FnMut(&T, f64, bool) -> Result<f64, AudioScheduleError>,
) -> Result<f64, AudioScheduleError> {
    let mut shortest = f64::INFINITY;
    let mut maximum = 0.0_f64;
    for segment in segments {
        let duration = segment.duration().value();
        shortest = shortest.min(duration);
        maximum = maximum.max(source_work(
            segment.source(),
            width.min(duration),
            duration <= 1.0,
        )?);
    }
    // The query scans section headers for each intersected arrangement period,
    // then visits at most this many occurrences (including boundary overlaps).
    let headers = segments.len() as f64 * ((width / period).ceil() + 1.0);
    let occurrences = (width / shortest).ceil() + 1.0;
    let work = headers + maximum * occurrences;
    Ok(work)
}

// Routing and sequence evaluators split their input at integer cycle boundaries.
// Their recursive children cannot cross another cycle boundary in that call.
fn cycle_windows(width: f64, within_cycle: bool) -> f64 {
    if within_cycle {
        1.0
    } else {
        width.ceil() + 1.0
    }
}

fn repetitions(width: f64, period: f64, within_cycle: bool) -> f64 {
    if within_cycle && period == 1.0 {
        1.0
    } else {
        (width / period).ceil() + 1.0
    }
}

fn slot_work<'a, T: 'a>(
    children: impl Iterator<Item = (&'a T, f64)>,
    width: f64,
    within_cycle: bool,
    mut source_work: impl FnMut(&T, f64, bool) -> Result<f64, AudioScheduleError>,
) -> Result<f64, AudioScheduleError> {
    let children: Vec<_> = children.collect();
    if let [(child, _)] = children.as_slice() {
        // The evaluator deliberately leaves a single slot unchanged.
        return source_work(child, width, within_cycle);
    }
    let total: f64 = children.iter().map(|(_, weight)| weight).sum();
    let mut maximum_cycle = 0.0;
    for (child, weight) in children {
        // A slot is stretched to its child's unit cycle. Even a partial slot
        // query is contained in that cycle, at every level of nested sequences.
        maximum_cycle += source_work(child, (width * total / weight).min(1.0), true)?;
    }
    Ok(maximum_cycle * cycle_windows(width, within_cycle))
}

fn score_work(
    score: &Score,
    width: f64,
    within_cycle: bool,
    depth: usize,
    nodes: &mut usize,
) -> Result<f64, AudioScheduleError> {
    enter(width, depth, nodes)?;
    let next = depth + 1;
    let work = match score.kind() {
        ScoreKind::QuerySource(source) => {
            let work = source.0.estimated_work(width);
            budget(work)?;
            work
        }
        // Every tile lies within its period. A window intersects at most
        // ceil(width / period) + 1 repetitions, including boundary overlaps.
        ScoreKind::Voice(voice) => {
            voice.tiles().len() as f64 * repetitions(width, voice.period().value(), within_cycle)
        }
        ScoreKind::Events(events) => events.len() as f64,
        ScoreKind::Arrange { segments, period } => arrangement_work(
            segments,
            period.value(),
            width,
            |source, window, bounded| score_work(source, window, bounded, next, nodes),
        )?,
        ScoreKind::CycleRoute(children) => {
            let mut maximum = 0.0_f64;
            for child in children {
                maximum = maximum.max(score_work(child, width.min(1.0), true, next, nodes)?);
            }
            maximum * cycle_windows(width, within_cycle)
        }
        ScoreKind::Merge(children) | ScoreKind::PriorityMerge { children, .. } => sum(children
            .iter()
            .map(|child| score_work(child, width, within_cycle, next, nodes)))?,
        ScoreKind::Concat(children) => sum(children
            .iter()
            .map(|child| score_work(child, width, false, next, nodes)))?,
        ScoreKind::CycleSlots(children) => slot_work(
            children.iter().map(|child| (child, 1.0)),
            width,
            within_cycle,
            |child, window, bounded| score_work(child, window, bounded, next, nodes),
        )?,
        ScoreKind::WeightedCycleSlots(children) => slot_work(
            children
                .iter()
                .map(|child| (child.score(), child.weight().value())),
            width,
            within_cycle,
            |child, window, bounded| score_work(child, window, bounded, next, nodes),
        )?,
        ScoreKind::WeightedChoice {
            options: children, ..
        } => sum(children
            .iter()
            .map(|child| score_work(child.score(), width, within_cycle, next, nodes)))?,
        ScoreKind::TimeScale { inner, rate } => score_work(
            inner,
            width * rate.value(),
            within_cycle && rate.value() == 1.0,
            next,
            nodes,
        )?,
        ScoreKind::Shift { inner, .. }
        | ScoreKind::ReflectCycle { inner }
        | ScoreKind::SpaceShift { inner, .. }
        | ScoreKind::SpaceScale { inner, .. }
        | ScoreKind::SpaceReflect { inner, .. }
        | ScoreKind::Degrade { inner, .. }
        | ScoreKind::Deduplicate { inner, .. } => score_work(inner, width, false, next, nodes)?,
        ScoreKind::MaskClip {
            source,
            mask: controls,
        }
        | ScoreKind::WithControls { source, controls } => {
            let source = score_work(source, width, within_cycle, next, nodes)?;
            let controls = control_work(controls, width, within_cycle, next, nodes)?;
            source.max(1.0) * controls.max(1.0)
        }
    };
    Ok(work)
}
fn control_work(
    score: &ControlScore,
    width: f64,
    within_cycle: bool,
    depth: usize,
    nodes: &mut usize,
) -> Result<f64, AudioScheduleError> {
    enter(width, depth, nodes)?;
    let next = depth + 1;
    let work = match score.kind() {
        ControlScoreKind::QuerySource(source) => {
            let work = source.0.estimated_work(width);
            budget(work)?;
            work
        }
        ControlScoreKind::Track(track) => {
            track.tiles().len() as f64 * repetitions(width, track.period().value(), within_cycle)
        }
        ControlScoreKind::Arrange { segments, period } => arrangement_work(
            segments,
            period.value(),
            width,
            |source, window, bounded| control_work(source, window, bounded, next, nodes),
        )?,
        ControlScoreKind::CycleRoute(children) => {
            let mut maximum = 0.0_f64;
            for child in children {
                maximum = maximum.max(control_work(child, width.min(1.0), true, next, nodes)?);
            }
            maximum * cycle_windows(width, within_cycle)
        }
        ControlScoreKind::Merge(children) | ControlScoreKind::PriorityMerge { children, .. } => {
            sum(children
                .iter()
                .map(|child| control_work(child, width, within_cycle, next, nodes)))?
        }
        ControlScoreKind::Concat(children) => sum(children
            .iter()
            .map(|child| control_work(child, width, false, next, nodes)))?,
        ControlScoreKind::CycleSlots(children) => slot_work(
            children.iter().map(|child| (child, 1.0)),
            width,
            within_cycle,
            |child, window, bounded| control_work(child, window, bounded, next, nodes),
        )?,
        ControlScoreKind::WeightedCycleSlots(children) => slot_work(
            children
                .iter()
                .map(|child| (child.score(), child.weight().value())),
            width,
            within_cycle,
            |child, window, bounded| control_work(child, window, bounded, next, nodes),
        )?,
        ControlScoreKind::WeightedChoice {
            options: children, ..
        } => sum(children
            .iter()
            .map(|child| control_work(child.score(), width, within_cycle, next, nodes)))?,
        ControlScoreKind::TimeScale { inner, rate } => control_work(
            inner,
            width * rate.value(),
            within_cycle && rate.value() == 1.0,
            next,
            nodes,
        )?,
        ControlScoreKind::Shift { inner, .. } | ControlScoreKind::ReflectCycle { inner } => {
            control_work(inner, width, false, next, nodes)?
        }
        ControlScoreKind::MaskClip { source, mask } => {
            control_work(source, width, within_cycle, next, nodes)?.max(1.0)
                * control_work(mask, width, within_cycle, next, nodes)?.max(1.0)
        }
    };
    Ok(work)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        intent::Intent,
        rational::Time,
        voice::{Tile, Voice},
    };
    fn note() -> Score {
        Score::from(
            Voice::new(
                Time::ONE,
                vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample("bd")).unwrap()],
            )
            .unwrap(),
        )
    }
    #[test]
    fn normal_patterns_pass_but_combined_extreme_rates_fail_before_projection() {
        assert!(PreparedScore::new(Score::time_scale(note(), Time::new(16, 1))).is_ok());
        let fast = Score::time_scale(
            Score::time_scale(note(), Time::new(1000, 1)),
            Time::new(1000, 1),
        );
        assert!(matches!(
            PreparedScore::new(fast),
            Err(AudioScheduleError::ProjectionLimit)
        ));
    }
    #[test]
    fn deeply_nested_scores_fail_without_unbounded_recursion() {
        let mut score = note();
        for _ in 0..100 {
            score = Score::shift(score, Time::ONE);
        }
        assert!(matches!(
            PreparedScore::new(score),
            Err(AudioScheduleError::StructureLimit)
        ));
    }

    #[test]
    fn long_arrangements_count_simultaneous_sections_not_the_whole_song() {
        let section = Score::time_scale(note(), Time::new(128, 1));
        let song = Score::arrange(
            (0..128)
                .map(|_| Timed::new(section.clone(), Time::new(8, 1), 1).unwrap())
                .collect(),
        )
        .unwrap();
        assert!(PreparedScore::new(song).is_ok());
        // The same sections really are too expensive when played together.
        assert!(PreparedScore::new(Score::merge(vec![section; 128])).is_err());
    }

    #[test]
    fn a_dangerous_late_section_or_tiny_occurrences_still_fail_preflight() {
        let late = Score::time_scale(note(), Time::new(100_000, 1));
        let song = Score::arrange(vec![
            Timed::new(note(), Time::new(8, 1), 1).unwrap(),
            Timed::new(late, Time::new(8, 1), 1).unwrap(),
        ])
        .unwrap();
        assert!(PreparedScore::new(song).is_err());
        let tiny = Score::arrange(vec![
            Timed::new(note(), Time::new(1, 100_000), 100_000).unwrap(),
        ])
        .unwrap();
        assert!(PreparedScore::new(tiny).is_err());
    }

    #[test]
    fn empty_sources_cannot_hide_expensive_control_or_mask_queries() {
        use crate::domain::control::{ControlKey, ControlTile, ControlTrack, ControlValue};
        let dense = ControlScore::from(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::ONE,
                        ControlKey::Gain,
                        ControlValue::Scalar(1.0)
                    )
                    .unwrap();
                    9_000
                ],
            )
            .unwrap(),
        );
        assert!(PreparedScore::new(Score::with_controls(Score::empty(), dense.clone())).is_err());
        let masked = ControlScore::mask_clip(ControlScore::empty(), dense);
        assert!(PreparedScore::new(Score::with_controls(note(), masked)).is_err());
    }

    #[test]
    fn nested_cycle_routing_counts_each_boundary_once_and_bounds_real_queries() {
        use crate::domain::span::Span;
        use crate::prelude::CadenceCompiler;

        let mut source = note();
        // There are 512 stored alternatives, but only one is queried per cycle.
        // Recharging boundary overlap inside each already-split cycle used to
        // reject this ordinary 128-note pattern as 65,536 units of work.
        for _ in 0..9 {
            source = Score::cycle_route(vec![source.clone(), source]);
        }
        let source = Score::time_scale(source, Time::new(128, 1));
        let estimate = estimate_audio_preparation(&source).unwrap();
        assert_eq!(estimate, 129.0);
        let prepared = PreparedScore::new(source).unwrap();
        for start in [Time::ZERO, Time::new(7, 13), Time::new(-5, 7)] {
            let span = Span::new(start, start + Time::ONE).unwrap();
            let report = CadenceCompiler::new().preview(&prepared, &span).unwrap();
            assert!(report.starts().count() as f64 <= estimate);
            assert!(report.starts().count() >= 128);
        }
    }

    #[test]
    fn sequence_children_use_one_cycle_but_speeds_still_expand_the_query() {
        use crate::domain::control::{ControlKey, ControlTile, ControlTrack, ControlValue};
        let controls = ControlScore::track(
            ControlTrack::new(
                Time::ONE,
                vec![
                    ControlTile::spanning(
                        Time::ZERO,
                        Time::ONE,
                        ControlKey::Velocity,
                        ControlValue::Scalar(0.8),
                    )
                    .unwrap(),
                ],
            )
            .unwrap(),
        );
        let mut hit = note();
        for _ in 0..12 {
            hit = Score::with_controls(hit, controls.clone());
        }
        // On a sequence's unit-cycle child query each repeated note/control
        // track visits exactly one repetition, rather than two at every owner.
        let sequence = Score::cycle_slots(vec![hit.clone(), hit]);
        assert_eq!(estimate_audio_preparation(&sequence).unwrap(), 4.0);
        PreparedScore::new(sequence.clone()).unwrap();

        let unsafe_child = Score::time_scale(note(), Time::new(100_000, 1));
        let sequence = Score::cycle_slots(vec![note(), unsafe_child]);
        assert!(PreparedScore::new(sequence).is_err());
        let dense = Score::time_scale(
            Score::cycle_slots(vec![note(), note()]),
            Time::new(10_000, 1),
        );
        assert!(PreparedScore::new(dense).is_err());
    }
}
