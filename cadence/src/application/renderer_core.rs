//! Score-first renderer wrapper used by scheduling and host seams.

use crate::{
    application::{
        clock::Clock,
        query::{EvaluatedEvent, evaluate_score},
        scheduler::events::{IDGenerator, scheduled_intent::ScheduledIntent},
    },
    domain::{
        control::ControlModelError, mosaic::Mosaic, projection::ProjectedMosaic, score::Score,
        span::TransportSpan, voice::Voice,
    },
};

static SCHEDULED_INTENT_ID_GENERATOR: IDGenerator = IDGenerator::new(1);

#[derive(Debug, Clone, PartialEq)]
/// Application-level score renderer.
pub struct RendererCore {
    score: Score,
}

impl RendererCore {
    /// Returns an empty renderer.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Score::empty())
    }

    /// Creates a renderer from a score tree.
    #[must_use]
    pub fn new(score: Score) -> Self {
        Self { score }
    }

    /// Convenience helper for one voice leaf.
    #[must_use]
    pub fn voice(voice: Voice) -> Self {
        Self::new(Score::from(voice))
    }

    /// Convenience helper for multiple voice leaves.
    #[must_use]
    pub fn voices(voices: Vec<Voice>) -> Self {
        Self::new(Score::merge(voices.into_iter().map(Score::from).collect()))
    }

    /// Convenience helper for already-projected transport-time material.
    #[must_use]
    pub fn mosaic(mosaic: Mosaic) -> Self {
        Self::new(Score::from(mosaic))
    }

    /// Returns the currently rendered score tree.
    #[must_use]
    pub fn score(&self) -> &Score {
        &self.score
    }

    pub(crate) fn evaluate_window(
        &self,
        window: &TransportSpan,
    ) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
        evaluate_score(&self.score, window)
    }

    /// Returns every evaluated event for `window`, including control updates.
    ///
    /// Prefer this (or [`CadenceCompiler::preview`]) when the host needs the same
    /// event list the scheduler consumes. [`Self::projected_output`] keeps only
    /// [`EvaluatedEventKind::StartVoice`] moments for backward-compatible previews.
    pub fn evaluate_window_full(
        &self,
        window: &TransportSpan,
    ) -> Result<Vec<EvaluatedEvent>, ControlModelError> {
        self.evaluate_window(window)
    }

    /// Returns the rich projected output for `window`.
    pub fn projected_output(
        &self,
        window: &TransportSpan,
    ) -> Result<ProjectedMosaic, ControlModelError> {
        Ok(ProjectedMosaic::new(
            self.evaluate_window(window)?
                .into_iter()
                .filter(|event| {
                    matches!(
                        event.kind(),
                        crate::application::query::EvaluatedEventKind::StartVoice { .. }
                    )
                })
                .map(EvaluatedEvent::into_projected)
                .collect(),
        ))
    }

    /// Returns a thin projected mosaic for compatibility code paths.
    pub fn projected_mosaic(&self, window: &TransportSpan) -> Result<Mosaic, ControlModelError> {
        Ok(self.projected_output(window)?.as_mosaic())
    }

    /// Replaces the renderer contents with another renderer's score.
    pub fn replace_renderer(&mut self, renderer: RendererCore) {
        self.score = renderer.score;
    }

    /// Replaces the renderer contents with a new score.
    pub fn replace_score(&mut self, score: Score) {
        self.score = score;
    }

    /// Replaces the renderer contents with one voice.
    pub fn replace_voice(&mut self, voice: Voice) {
        self.replace_score(Score::from(voice));
    }

    /// Replaces the renderer contents with multiple voices.
    pub fn replace_voices(&mut self, voices: Vec<Voice>) {
        self.replace_score(Score::merge(voices.into_iter().map(Score::from).collect()));
    }

    /// Replaces the renderer contents with a projected mosaic.
    pub fn replace_mosaic(&mut self, mosaic: Mosaic) {
        self.replace_score(Score::from(mosaic));
    }

    /// Projects a window and lowers it into scheduled intents using `clock`.
    pub fn render_window(
        &self,
        window: &TransportSpan,
        clock: &Clock,
    ) -> Result<Vec<ScheduledIntent>, ControlModelError> {
        Ok(self
            .evaluate_window(window)?
            .into_iter()
            .map(|event| {
                ScheduledIntent::from_evaluated(SCHEDULED_INTENT_ID_GENERATOR.next(), event, clock)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        control::{ControlKey, ControlTile, ControlTrack, ControlValue},
        intent::Intent,
        prelude::Time,
        score::{ControlScore, DegradePolicy, Score},
        span::Span,
        voice::{Repeat, Tile, Voice, ops},
    };
    use std::time::Instant;

    fn span(start: (i64, i64), end: (i64, i64)) -> Span {
        Span::new(Time::new(start.0, start.1), Time::new(end.0, end.1)).unwrap()
    }

    fn source_tile(start: (i64, i64), end: (i64, i64), sample: &str, id: u64) -> Tile {
        Tile::spanning(
            Time::new(start.0, start.1),
            Time::new(end.0, end.1),
            Intent::sample(sample),
        )
        .unwrap()
        .with_id(id)
    }

    fn voice_with_tiles(tiles: Vec<Tile>) -> Voice {
        Voice::new(Time::ONE, tiles).unwrap()
    }

    fn test_clock() -> Clock {
        Clock::new(Time::new(2, 1), Instant::now())
    }

    #[test]
    fn voice_with_one_source_tile_repeats_every_cycle_by_default() {
        let renderer = RendererCore::voice(voice_with_tiles(vec![source_tile(
            (0, 1),
            (1, 4),
            "kick",
            1,
        )]));
        let result = renderer
            .render_window(&span((0, 1), (2, 1)), &test_clock())
            .unwrap();

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].whole(), span((0, 1), (1, 4)));
        assert_eq!(result[1].whole(), span((1, 1), (5, 4)));
    }

    #[test]
    fn repeat_once_emits_only_the_first_cycle() {
        let voice = voice_with_tiles(vec![source_tile((0, 1), (1, 4), "kick", 1)])
            .with_repeat(Repeat::Once);
        let renderer = RendererCore::voice(voice);
        let result = renderer
            .render_window(&span((0, 1), (2, 1)), &test_clock())
            .unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn projected_output_carries_control_map_from_with_controls() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    ControlKey::Gain,
                    ControlValue::Ramp { from: 1.0, to: 0.0 },
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let renderer = RendererCore::new(Score::with_controls(
            Score::from(voice_with_tiles(vec![source_tile(
                (0, 1),
                (1, 1),
                "pad",
                1,
            )])),
            ControlScore::from(controls),
        ));

        let output = renderer.projected_output(&span((0, 1), (1, 1))).unwrap();

        assert!(matches!(
            output.moments()[0].controls().get(&ControlKey::Gain),
            Some(ControlValue::Ramp { .. })
        ));
    }

    #[test]
    fn projected_mosaic_drops_control_map_but_preserves_transport_view() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::ONE,
                    ControlKey::Gain,
                    ControlValue::Ramp { from: 1.0, to: 0.0 },
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let renderer = RendererCore::new(Score::with_controls(
            Score::from(voice_with_tiles(vec![source_tile(
                (0, 1),
                (1, 1),
                "pad",
                1,
            )])),
            ControlScore::from(controls),
        ));

        let rich = renderer.projected_output(&span((0, 1), (1, 1))).unwrap();
        let thin = renderer.projected_mosaic(&span((0, 1), (1, 1))).unwrap();

        assert_eq!(rich.len(), thin.len());
        assert_eq!(rich.moments()[0].whole(), thin.moments()[0].span());
    }

    #[test]
    fn warped_voice_renders_through_existing_scheduler_path() {
        let warped = ops::fast_by(
            &voice_with_tiles(vec![source_tile((0, 1), (1, 2), "bd", 7)]),
            Time::new(2, 1),
        );
        let renderer = RendererCore::voice(warped);
        let result = renderer
            .render_window(&span((0, 1), (1, 1)), &test_clock())
            .unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn degrade_affects_projected_output_deterministically() {
        let renderer = RendererCore::new(Score::degrade(
            Score::merge(vec![
                Score::from(voice_with_tiles(vec![source_tile(
                    (0, 1),
                    (1, 4),
                    "kick",
                    1,
                )])),
                Score::shift(
                    Score::from(voice_with_tiles(vec![source_tile(
                        (0, 1),
                        (1, 4),
                        "snare",
                        2,
                    )])),
                    Time::ONE,
                ),
                Score::shift(
                    Score::from(voice_with_tiles(vec![source_tile(
                        (0, 1),
                        (1, 4),
                        "hat",
                        3,
                    )])),
                    Time::new(2, 1),
                ),
            ]),
            DegradePolicy::new(Time::new(1, 2), 19),
        ));

        let first = renderer.projected_output(&span((0, 1), (4, 1))).unwrap();
        let second = renderer.projected_output(&span((0, 1), (4, 1))).unwrap();

        assert_eq!(first, second);
    }
}
