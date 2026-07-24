//! The [`Pattern`] trait: a uniform "query a span" authoring/combinator seam.
//!
//! `Pattern` mirrors Strudel's "everything is a function of time" idea without
//! replacing the structural `Score` IR. A pattern is anything that, given a
//! transport-time window, can produce the events visible inside it. Querying is
//! infallible (Strudel-style); structural query errors that can only arise from
//! programmer mistakes are absorbed by the concrete implementations.
//!
//! Two implementations ship with the crate:
//! - [`Signal`](crate::domain::signal::Signal) is a continuous source: a query
//!   returns a single event covering the window, sampled at the window
//!   midpoint.
//! - `Score` (implemented in the application layer) is the canonical structural
//!   source queried by `evaluate_score`.

use crate::domain::{
    prelude::Time,
    signal::Signal,
    span::{Span, Transport},
};

/// A queryable source of transport-time events.
///
/// Implementors map a query window to the events visible inside it. The
/// associated [`Pattern::Event`] type lets continuous sources yield scalars and
/// structural sources yield richer projected moments.
pub trait Pattern {
    /// The value produced for each event in a query.
    type Event;

    /// Returns the `(visible_span, event)` pairs visible in `span`.
    fn query(&self, span: Span<Transport>) -> Vec<(Span<Transport>, Self::Event)>;
}

impl Pattern for Signal {
    type Event = f64;

    fn query(&self, span: Span<Transport>) -> Vec<(Span<Transport>, Self::Event)> {
        // Continuous signals sample once at the window midpoint, matching
        // Strudel continuous-signal query semantics.
        let midpoint = (span.start() + span.end()) / Time::whole_number(2);
        vec![(span, self.eval(midpoint.value()))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: (i64, i64), end: (i64, i64)) -> Span<Transport> {
        Span::new(Time::new(start.0, start.1), Time::new(end.0, end.1)).unwrap()
    }

    #[test]
    fn signal_query_samples_window_midpoint() {
        let signal = Signal::sine();
        let window = span((0, 1), (1, 2));
        let events = signal.query(window);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, window);
        // Midpoint of [0, 1/2) is 1/4 cycle -> sine peak.
        assert!((events[0].1 - signal.eval(0.25)).abs() < 1e-12);
    }
}
