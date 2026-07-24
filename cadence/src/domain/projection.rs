//! Rich projected transport-time output.

use crate::domain::{
    control::ControlMap,
    moment::{Moment, MomentId},
    mosaic::Mosaic,
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

#[derive(Debug, Clone, Default, PartialEq)]
/// Sorted collection of projected moments.
pub struct ProjectedMosaic {
    moments: Vec<ProjectedMoment>,
}

impl ProjectedMosaic {
    /// Creates a projected mosaic and sorts its moments by whole span.
    #[must_use]
    pub fn new(mut moments: Vec<ProjectedMoment>) -> Self {
        moments.sort_by(|left, right| {
            left.whole()
                .start()
                .cmp(&right.whole().start())
                .then(left.whole().end().cmp(&right.whole().end()))
        });

        Self { moments }
    }

    /// Returns an empty projected mosaic.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Inserts a projected moment and re-sorts the collection.
    pub fn push(&mut self, moment: ProjectedMoment) {
        self.moments.push(moment);
        self.moments.sort_by(|left, right| {
            left.whole()
                .start()
                .cmp(&right.whole().start())
                .then(left.whole().end().cmp(&right.whole().end()))
        });
    }

    /// Returns the number of projected moments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.moments.len()
    }

    /// Returns `true` when the projected mosaic is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.moments.is_empty()
    }

    /// Returns the sorted projected moment slice.
    #[must_use]
    pub fn moments(&self) -> &[ProjectedMoment] {
        &self.moments
    }

    /// Iterates over projected moments in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = &ProjectedMoment> {
        self.moments.iter()
    }

    /// Drops projection-only information and returns a plain [`Mosaic`].
    ///
    /// This is a lossy conversion: the visible span and projected controls are
    /// not preserved.
    #[must_use]
    pub fn as_mosaic(&self) -> Mosaic {
        Mosaic::new(
            self.moments
                .iter()
                .map(|moment| moment.as_moment().clone())
                .collect(),
        )
    }
}

impl IntoIterator for ProjectedMosaic {
    type Item = ProjectedMoment;
    type IntoIter = std::vec::IntoIter<ProjectedMoment>;

    fn into_iter(self) -> Self::IntoIter {
        self.moments.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{control::ControlMap, intent::Intent, prelude::Time};

    fn projected_moment(
        start: (i64, i64),
        end: (i64, i64),
        visible_start: (i64, i64),
        visible_end: (i64, i64),
        sample: &str,
        id: u64,
    ) -> ProjectedMoment {
        let moment = Moment::spanning(
            Time::new(start.0, start.1),
            Time::new(end.0, end.1),
            Intent::sample(sample),
        )
        .unwrap()
        .with_id(id);

        ProjectedMoment::new(
            moment,
            TransportSpan::new(
                Time::new(visible_start.0, visible_start.1),
                Time::new(visible_end.0, visible_end.1),
            )
            .unwrap(),
            ControlMap::new(),
        )
    }

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

    #[test]
    fn projected_mosaic_stays_sorted_by_whole_span() {
        let late = projected_moment((1, 2), (1, 1), (1, 2), (1, 1), "snare", 2);
        let early = projected_moment((0, 1), (1, 4), (0, 1), (1, 4), "kick", 1);
        let projected = ProjectedMosaic::new(vec![late, early.clone()]);

        assert_eq!(projected.moments()[0], early);
    }
}
