//! Host-facing score preview and projection service.

use crate::{
    application::{EvaluatedEvent, EvaluatedEventKind, renderer_core::RendererCore},
    domain::{control::ControlModelError, span::TransportSpan},
    infrastructure::PreparedScore,
};

/// Rich projection output for one transport window.
#[derive(Debug, Clone, PartialEq)]
pub struct PreviewReport {
    /// Transport window that was projected.
    pub window: TransportSpan,
    /// Complete ordered event list, including control updates.
    pub events: Vec<EvaluatedEvent>,
}

impl PreviewReport {
    /// Returns evaluated events that start a new voice instance.
    pub fn starts(&self) -> impl Iterator<Item = &EvaluatedEvent> {
        self.events
            .iter()
            .filter(|event| matches!(event.kind(), EvaluatedEventKind::StartVoice { .. }))
    }

    /// Returns evaluated events that update an existing voice instance.
    pub fn control_updates(&self) -> impl Iterator<Item = &EvaluatedEvent> {
        self.events
            .iter()
            .filter(|event| matches!(event.kind(), EvaluatedEventKind::UpdateVoiceControls { .. }))
    }
}

/// Score-first preview and projection service for host crates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CadenceCompiler;

impl CadenceCompiler {
    /// Creates a compiler instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Projects a prepared score over `window` without scheduling side effects.
    pub fn preview(
        &self,
        score: &PreparedScore,
        window: &TransportSpan,
    ) -> Result<PreviewReport, ControlModelError> {
        let renderer = RendererCore::new(score.score().clone());
        let events = renderer.evaluate_window(window)?;
        Ok(PreviewReport {
            window: *window,
            events,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::EvaluatedEventKind,
        domain::{
            control::{ControlKey, ControlTile, ControlTrack, ControlValue, SignedUnitValue},
            intent::Intent,
            prelude::Time,
            score::{ControlScore, Score},
            span::Span,
            voice::{Tile, Voice},
        },
        infrastructure::stack::{cycle, sample, tile},
    };

    #[test]
    fn preview_reports_projected_moments() {
        let score = Score::from(cycle(vec![tile(
            Time::ZERO,
            Time::new(1, 2),
            sample("kick"),
        )]));
        let window = Span::new(Time::ZERO, Time::ONE).unwrap();
        let prepared = PreparedScore::new(score).unwrap();
        let report = CadenceCompiler::new().preview(&prepared, &window).unwrap();
        assert_eq!(report.starts().count(), 1);
    }

    #[test]
    fn preview_evaluated_includes_continuous_runtime_control_updates() {
        let controls = ControlTrack::new(
            Time::ONE,
            vec![
                ControlTile::spanning(
                    Time::ZERO,
                    Time::new(1, 2),
                    ControlKey::PitchBend,
                    ControlValue::Bipolar(SignedUnitValue::new(0.0).unwrap()),
                )
                .unwrap(),
                ControlTile::spanning(
                    Time::new(1, 2),
                    Time::ONE,
                    ControlKey::PitchBend,
                    ControlValue::Bipolar(SignedUnitValue::new(1.0).unwrap()),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let voice = Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample("pad")).unwrap()],
        )
        .unwrap();
        let score = Score::with_controls(Score::from(voice), ControlScore::from(controls));
        let window = Span::new(Time::ZERO, Time::ONE).unwrap();
        let prepared = PreparedScore::new(score).unwrap();
        let report = CadenceCompiler::new().preview(&prepared, &window).unwrap();

        assert_eq!(report.events.len(), 2);
        assert_eq!(report.starts().count(), 1);
        assert_eq!(report.control_updates().count(), 1);
        assert!(matches!(
            report.events[0].kind(),
            EvaluatedEventKind::StartVoice { .. }
        ));
        assert!(matches!(
            report.events[1].kind(),
            EvaluatedEventKind::UpdateVoiceControls { .. }
        ));
    }
}
