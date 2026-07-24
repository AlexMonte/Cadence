//! Legacy `Arc` naming for transport-time spans.

use crate::domain::span::{Span, Transport};

/// Backwards-compatible alias for transport-time spans.
pub type Arc = Span<Transport>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::rational::Time;

    fn arc(s: (i64, i64), e: (i64, i64)) -> Arc {
        Arc::new(Time::new(s.0, s.1), Time::new(e.0, e.1)).unwrap()
    }

    #[test]
    fn arc_disjoint_returns_true() {
        let e = arc((0, 1), (1, 2));
        let i = arc((1, 2), (1, 1));
        assert!(!e.contains(&i));
    }

    #[test]
    fn arc_intersects_returns_true() {
        let e = arc((0, 1), (1, 2));
        let i = arc((1, 4), (3, 4));
        assert!(e.intersects(&i));
    }

    #[test]
    fn arc_intersection_returns_correct_arc() {
        let e = arc((0, 1), (3, 4));
        let i = arc((1, 2), (4, 4));
        assert_eq!(e.intersection(&i), Some(arc((1, 2), (3, 4))));
    }
}
