//! Application-layer [`Pattern`] implementation for [`Score`].
//!
//! `Score` is the canonical structural source queried by `evaluate_score`. The
//! `Pattern` trait itself lives in the domain layer, but this implementation
//! must live in the application layer because it depends on the `pub(crate)`
//! `evaluate_score` query (the domain layer must not depend on the application
//! layer).

use crate::{
    application::query::evaluate_score,
    domain::{
        pattern::Pattern,
        projection::ProjectedMoment,
        score::Score,
        span::{Span, Transport},
    },
};

impl Pattern for Score {
    type Event = ProjectedMoment;

    fn query(&self, span: Span<Transport>) -> Vec<(Span<Transport>, Self::Event)> {
        // `Pattern::query` is infallible (Strudel-style). Control-model errors
        // here can only be programmer mistakes, so we fall back to an empty
        // result rather than propagating an error.
        match evaluate_score(self, &span) {
            Ok(events) => events
                .into_iter()
                .map(|event| {
                    let moment = event.into_projected();
                    (moment.visible(), moment)
                })
                .collect(),
            Err(error) => {
                debug_assert!(
                    false,
                    "Pattern::query for Score saw a control error: {error}"
                );
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        intent::Intent,
        prelude::Time,
        voice::{Tile, Voice},
    };

    fn span(start: (i64, i64), end: (i64, i64)) -> Span<Transport> {
        Span::new(Time::new(start.0, start.1), Time::new(end.0, end.1)).unwrap()
    }

    fn sample_voice() -> Voice {
        Voice::new(
            Time::ONE,
            vec![
                Tile::spanning(Time::ZERO, Time::new(1, 2), Intent::sample("kick"))
                    .unwrap()
                    .with_id(1),
                Tile::spanning(Time::new(1, 2), Time::ONE, Intent::sample("snare"))
                    .unwrap()
                    .with_id(2),
            ],
        )
        .unwrap()
    }

    #[test]
    fn score_query_matches_evaluate_score_visible_spans() {
        let score = Score::voice(sample_voice());
        let window = span((0, 1), (2, 1));

        let queried: Vec<_> = score.query(window).into_iter().map(|(s, _)| s).collect();
        let expected: Vec<_> = evaluate_score(&score, &window)
            .unwrap()
            .into_iter()
            .map(|event| event.projected().visible())
            .collect();

        assert_eq!(queried, expected);
        assert!(!queried.is_empty());
    }
}
