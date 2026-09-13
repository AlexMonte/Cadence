//! Rich projected transport-time output.

use crate::domain::{
    control::ControlMap,
    moment::{Moment, MomentId},
    span::TransportSpan,
};

#[derive(Debug, Clone, PartialEq)]
/// One projected moment with both its full span and the currently visible span.
///
/// Projection may clip a repeating source event to the current render window.
/// `ProjectedMoment` keeps both the original whole span and the clipped visible
/// span, plus any projected control values that apply at that point.
pub struct ProjectedMoment {
    moment: Moment,
    visible: TransportSpan,
    controls: ControlMap,
}

impl ProjectedMoment {
    /// Creates a projected moment.
    ///
    /// # Panics
    ///
    /// Panics if `visible` is not fully contained inside the whole moment span.
    #[must_use]
    pub fn new(moment: Moment, visible: TransportSpan, controls: ControlMap) -> Self {
        assert!(
            moment.span().contains(&visible),
            "visible span must be contained by the whole moment span"
        );

        Self {
            moment,
            visible,
            controls,
        }
    }

    /// Returns the full transport span of the underlying moment.
    #[must_use]
    pub fn whole(&self) -> TransportSpan {
        self.moment.span()
    }

    /// Returns the clipped span that is visible in the current projection.
    #[must_use]
    pub fn visible(&self) -> TransportSpan {
        self.visible
    }

    /// Returns the musical intent carried by the projected moment.
    #[must_use]
    pub fn intent(&self) -> &crate::domain::intent::Intent {
        self.moment.intent()
    }

    /// Returns the spatial trajectory carried by the projected moment.
    #[must_use]
    pub fn position(&self) -> crate::domain::space::SpatialMotion {
        self.moment.position()
    }

    /// Returns the projected controls active for this moment.
    #[must_use]
    pub fn controls(&self) -> &ControlMap {
        &self.controls
    }

    /// Returns the optional stable identity of the underlying moment.
    #[must_use]
    pub fn id(&self) -> Option<MomentId> {
        self.moment.id()
    }

    /// Returns the underlying moment view.
    #[must_use]
    pub fn as_moment(&self) -> &Moment {
        &self.moment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{control::ControlMap, intent::Intent, prelude::Time};

    #[test]
    fn projected_moment_requires_visible_inside_whole() {
        let whole = Moment::spanning(Time::ZERO, Time::ONE, Intent::sample("kick")).unwrap();
        let invalid_visible = TransportSpan::new(Time::new(-1, 4), Time::new(1, 2)).unwrap();

        assert!(
            std::panic::catch_unwind(|| {
                let _ = ProjectedMoment::new(whole, invalid_visible, ControlMap::new());
            })
            .is_err()
        );
    }
}
