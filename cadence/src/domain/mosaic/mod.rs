//! Ordered collections of transport-time moments.

use crate::domain::{moment::Moment, span::Span};

/// Source-level mosaic transform helpers.
pub mod ops;

#[derive(Debug, Clone, Default, PartialEq)]
/// Sorted collection of transport-time moments.
pub struct Mosaic {
    moments: Vec<Moment>,
}

impl Mosaic {
    /// Creates a mosaic and sorts its moments by span.
    #[must_use]
    pub fn new(mut moments: Vec<Moment>) -> Self {
        moments.sort_by(|left, right| {
            left.span()
                .start()
                .cmp(&right.span().start())
                .then(left.span().end().cmp(&right.span().end()))
        });
        Self { moments }
    }

    /// Returns an empty mosaic.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Inserts a moment and re-sorts the mosaic.
    pub fn push(&mut self, moment: Moment) {
        self.moments.push(moment);
        self.moments.sort_by(|left, right| {
            left.span()
                .start()
                .cmp(&right.span().start())
                .then(left.span().end().cmp(&right.span().end()))
        });
    }

    /// Returns the number of moments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.moments.len()
    }

    /// Returns `true` when the mosaic contains no moments.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.moments.is_empty()
    }

    /// Returns the sorted moment slice.
    #[must_use]
    pub fn moments(&self) -> &[Moment] {
        &self.moments
    }

    /// Returns the smallest span that encloses every moment.
    #[must_use]
    pub fn bounds(&self) -> Option<Span> {
        let first = self.moments.first()?;
        let start = first.span().start();
        let end = self
            .moments
            .iter()
            .map(|moment| moment.span().end())
            .max()
            .unwrap_or(start);

        Span::new(start, end)
    }

    /// Iterates over the moments in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = &Moment> {
        self.moments.iter()
    }
}

impl IntoIterator for Mosaic {
    type Item = Moment;
    type IntoIter = std::vec::IntoIter<Moment>;

    fn into_iter(self) -> Self::IntoIter {
        self.moments.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{intent::Intent, prelude::Time};

    fn moment(start: (i64, i64), end: (i64, i64), name: &str, id: u64) -> Moment {
        Moment::spanning(
            Time::new(start.0, start.1),
            Time::new(end.0, end.1),
            Intent::sample(name),
        )
        .unwrap()
        .with_id(id)
    }

    #[test]
    fn mosaic_preserves_horizontal_succession_as_adjacent_spans() {
        let left = moment((0, 1), (1, 2), "kick", 1);
        let right = moment((1, 2), (1, 1), "snare", 2);
        let mosaic = Mosaic::new(vec![right.clone(), left.clone()]);

        assert_eq!(mosaic.moments()[0], left);
        assert_eq!(mosaic.moments()[1], right);
    }

    #[test]
    fn mosaic_preserves_vertical_alignment_as_same_start_moments() {
        let low = moment((0, 1), (1, 2), "kick", 1);
        let high = moment((0, 1), (1, 1), "hat", 2);
        let mosaic = Mosaic::new(vec![high.clone(), low.clone()]);

        assert_eq!(mosaic.len(), 2);
        assert_eq!(mosaic.moments()[0].span().start(), Time::ZERO);
        assert_eq!(mosaic.moments()[1].span().start(), Time::ZERO);
        assert_eq!(mosaic.moments()[0], low);
        assert_eq!(mosaic.moments()[1], high);
    }

    #[test]
    fn mosaic_keeps_insertion_order_for_identical_spans() {
        let first = moment((0, 1), (1, 2), "kick", 1);
        let second = moment((0, 1), (1, 2), "hat", 2);
        let mosaic = Mosaic::new(vec![first.clone(), second.clone()]);

        assert_eq!(mosaic.moments()[0], first);
        assert_eq!(mosaic.moments()[1], second);
    }

    #[test]
    fn mosaic_allows_multiple_moments_to_share_identity() {
        let first = moment((0, 1), (1, 4), "kick", 7);
        let second = moment((1, 4), (1, 2), "kick-accent", 7);
        let mosaic = Mosaic::new(vec![first.clone(), second.clone()]);

        assert_eq!(mosaic.moments()[0].id(), first.id());
        assert_eq!(mosaic.moments()[1].id(), second.id());
        assert_eq!(mosaic.moments()[0].id(), mosaic.moments()[1].id());
    }
}
