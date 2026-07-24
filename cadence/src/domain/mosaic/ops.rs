//! Source-level transforms for projected transport-time mosaics.

use crate::domain::{moment::Moment, mosaic::Mosaic, prelude::Time, span::Span};

/// Layers multiple mosaics without changing their timing.
#[must_use]
pub fn overlay(layers: impl IntoIterator<Item = Mosaic>) -> Mosaic {
    Mosaic::new(layers.into_iter().flat_map(Mosaic::into_iter).collect())
}

/// Places mosaics back-to-back based on their bounds.
///
/// Empty mosaics are skipped.
#[must_use]
pub fn chain(segments: impl IntoIterator<Item = Mosaic>) -> Mosaic {
    let mut offset = Time::ZERO;
    let mut chained = Vec::new();

    for segment in segments {
        let Some(bounds) = segment.bounds() else {
            continue;
        };
        let shift = offset - bounds.start();
        let segment_duration = bounds.end() - bounds.start();

        chained.extend(segment.into_iter().map(|moment| translate(moment, shift)));
        offset = offset + segment_duration;
    }

    Mosaic::new(chained)
}

/// Splits one moment into `parts` equal sub-moments.
///
/// # Panics
///
/// Panics if `parts == 0`.
#[must_use]
pub fn subdivide(moment: &Moment, parts: usize) -> Mosaic {
    assert!(parts > 0, "subdivide requires at least one part");

    let part_duration = moment.duration() / Time::whole_number(parts as i64);

    Mosaic::new(
        (0..parts)
            .map(|index| {
                let offset = part_duration * Time::whole_number(index as i64);
                let start = moment.span().start() + offset;
                let end = start + part_duration;
                clone_with_span(moment, Span::new(start, end).unwrap())
            })
            .collect(),
    )
}

/// Scales every moment span by `factor`.
///
/// # Panics
///
/// Panics if `factor <= 0`.
#[must_use]
pub fn stretch(mosaic: &Mosaic, factor: Time) -> Mosaic {
    assert!(factor > Time::ZERO, "stretch factor must be positive");

    Mosaic::new(
        mosaic
            .iter()
            .map(|moment| {
                let start = moment.span().start() * factor;
                let end = moment.span().end() * factor;
                clone_with_span(moment, Span::new(start, end).unwrap())
            })
            .collect(),
    )
}

/// Inverse of [`stretch`].
///
/// # Panics
///
/// Panics if `factor <= 0`.
#[must_use]
pub fn shrink(mosaic: &Mosaic, factor: Time) -> Mosaic {
    assert!(factor > Time::ZERO, "shrink factor must be positive");
    stretch(mosaic, Time::ONE / factor)
}

/// Reflects each moment inside `container`.
#[must_use]
pub fn mirror(mosaic: &Mosaic, container: Span) -> Mosaic {
    Mosaic::new(
        mosaic
            .iter()
            .map(|moment| {
                let start = container.start() + (container.end() - moment.span().end());
                let end = container.start() + (container.end() - moment.span().start());
                clone_with_span(moment, Span::new(start, end).unwrap())
            })
            .collect(),
    )
}

/// Clips each moment to the regions where it overlaps any gate span.
#[must_use]
pub fn gate_by_overlap(mosaic: &Mosaic, gates: &[Span]) -> Mosaic {
    Mosaic::new(
        mosaic
            .iter()
            .flat_map(|moment| {
                gates.iter().filter_map(move |gate| {
                    let visible = moment.span().intersection(gate)?;
                    Some(clone_with_span(moment, visible))
                })
            })
            .collect(),
    )
}

fn translate(moment: Moment, offset: Time) -> Moment {
    let start = moment.span().start() + offset;
    let end = moment.span().end() + offset;
    clone_with_span(&moment, Span::new(start, end).unwrap())
}

fn clone_with_span(moment: &Moment, span: Span) -> Moment {
    let mut cloned = Moment::new(span, moment.intent().clone());
    if let Some(id) = moment.id() {
        cloned = cloned.with_id(id);
    }
    cloned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{intent::Intent, prelude::Time};

    fn sample_moment(start: (i64, i64), end: (i64, i64), sample: &str, id: u64) -> Moment {
        Moment::spanning(
            Time::new(start.0, start.1),
            Time::new(end.0, end.1),
            Intent::sample(sample),
        )
        .unwrap()
        .with_id(id)
    }

    #[test]
    fn chain_turns_horizontal_segments_into_adjacent_time() {
        let first = Mosaic::new(vec![sample_moment((2, 1), (5, 2), "kick", 1)]);
        let second = Mosaic::new(vec![sample_moment((4, 1), (5, 1), "snare", 2)]);
        let chained = chain(vec![first, second]);

        assert_eq!(chained.moments().len(), 2);
        assert_eq!(
            chained.moments()[0].span(),
            Span::new(Time::ZERO, Time::new(1, 2)).unwrap()
        );
        assert_eq!(
            chained.moments()[1].span(),
            Span::new(Time::new(1, 2), Time::new(3, 2)).unwrap()
        );
    }

    #[test]
    fn overlay_preserves_simultaneous_moments() {
        let low = Mosaic::new(vec![sample_moment((0, 1), (1, 2), "kick", 1)]);
        let high = Mosaic::new(vec![sample_moment((0, 1), (1, 2), "hat", 2)]);
        let layered = overlay(vec![low, high]);

        assert_eq!(layered.len(), 2);
        assert_eq!(layered.moments()[0].span().start(), Time::ZERO);
        assert_eq!(layered.moments()[1].span().start(), Time::ZERO);
    }

    #[test]
    fn subdivide_partitions_a_moment_evenly() {
        let moment = sample_moment((0, 1), (1, 1), "kick", 7);

        let halves = subdivide(&moment, 2);
        let thirds = subdivide(&moment, 3);
        let quarters = subdivide(&moment, 4);

        assert_eq!(
            halves.moments()[1].span(),
            Span::new(Time::new(1, 2), Time::ONE).unwrap()
        );
        assert_eq!(
            thirds.moments()[1].span(),
            Span::new(Time::new(1, 3), Time::new(2, 3)).unwrap()
        );
        assert_eq!(
            quarters.moments()[3].span(),
            Span::new(Time::new(3, 4), Time::ONE).unwrap()
        );
        assert!(
            quarters
                .iter()
                .all(|moment| moment.id() == Some(7_u64.into()))
        );
    }

    #[test]
    fn stretch_and_mirror_transform_spans_deterministically() {
        let mosaic = Mosaic::new(vec![sample_moment((1, 4), (1, 2), "vox", 9)]);
        let stretched = stretch(&mosaic, Time::new(2, 1));
        let mirrored = mirror(&stretched, Span::new(Time::ZERO, Time::new(2, 1)).unwrap());

        assert_eq!(
            stretched.moments()[0].span(),
            Span::new(Time::new(1, 2), Time::ONE).unwrap()
        );
        assert_eq!(
            mirrored.moments()[0].span(),
            Span::new(Time::ONE, Time::new(3, 2)).unwrap()
        );
    }

    #[test]
    fn gate_by_overlap_clips_moments_to_gate_spans() {
        let source = Mosaic::new(vec![
            sample_moment((0, 1), (1, 1), "pad", 4),
            sample_moment((1, 1), (2, 1), "lead", 5),
        ]);
        let gates = [Span::new(Time::new(1, 4), Time::new(3, 4)).unwrap()];
        let gated = gate_by_overlap(&source, &gates);

        assert_eq!(gated.len(), 1);
        assert_eq!(
            gated.moments()[0].span(),
            Span::new(Time::new(1, 4), Time::new(3, 4)).unwrap()
        );
        assert_eq!(gated.moments()[0].id(), Some(4_u64.into()));
    }
}
