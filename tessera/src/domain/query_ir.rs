//! Bounded evaluation used by previews. Musical structures stay recursive until queried.
use super::pattern_ir::*;

fn unit() -> CycleSpan {
    CycleSpan::new(CycleTime(Rational::zero()), CycleDuration(Rational::one()))
}
fn floor(value: Rational) -> i64 {
    value.numerator.div_euclid(value.denominator)
}
fn intersects(a: CycleSpan, b: CycleSpan) -> bool {
    a.start < b.end() && b.start < a.end()
}
fn intersection(a: CycleSpan, b: CycleSpan) -> Option<CycleSpan> {
    let start = a.start.max(b.start);
    let end = a.end().min(b.end());
    (start < end).then(|| CycleSpan::new(start, CycleDuration(end.0 - start.0)))
}
fn cycles(span: CycleSpan) -> impl Iterator<Item = i64> {
    let end = span.end().0;
    floor(span.start.0)..(floor(end) + i64::from(end.denominator != 1))
}
fn in_window(stream: PatternStream, span: CycleSpan) -> PatternStream {
    PatternStream::new(
        stream
            .events
            .into_iter()
            .filter(|event| intersects(event.span, span))
            .collect(),
    )
}
fn slots(children: &[(&PatternNodeIr, Rational)], span: CycleSpan) -> PatternStream {
    if children.len() == 1 {
        return query(children[0].0, span);
    }
    let total = children
        .iter()
        .fold(Rational::zero(), |sum, (_, weight)| sum + *weight);
    if total <= Rational::zero() {
        return PatternStream::default();
    }
    let mut events = Vec::new();
    for cycle in cycles(span) {
        let origin = Rational::from_integer(cycle);
        let mut offset = Rational::zero();
        for (child, weight) in children {
            let width = *weight / total;
            let start = origin + offset;
            offset = offset + width;
            let slot = CycleSpan::new(CycleTime(start), CycleDuration(width));
            let Some(visible) = intersection(slot, span) else {
                continue;
            };
            let inner = CycleSpan::new(
                CycleTime(origin + (visible.start.0 - start) / width),
                CycleDuration(visible.duration.0 / width),
            );
            events.extend(query(child, inner).events.into_iter().map(|mut event| {
                let map = |time: Rational, is_end: bool| {
                    let cycle = floor(time) - i64::from(is_end && time.denominator == 1);
                    let cycle_origin = Rational::from_integer(cycle);
                    cycle_origin + (start - origin) + (time - cycle_origin) * width
                };
                let event_start = map(event.span.start.0, false);
                let event_end = map(event.span.end().0, true);
                event.remap_onset(CycleSpan::new(
                    CycleTime(event_start),
                    CycleDuration(event_end - event_start),
                ));
                event
            }));
        }
    }
    let mut unique = Vec::new();
    for event in events {
        if !unique.contains(&event) {
            unique.push(event);
        }
    }
    PatternStream::new(unique)
}

fn arrange(segments: &[TimedPatternIr], span: CycleSpan) -> PatternStream {
    let Some(period) = arrangement_period(segments).filter(|period| *period > Rational::zero())
    else {
        return PatternStream::default();
    };
    // Compute scaled bounds independently: adding two large fractions just
    // to find an integer loop index needlessly overflows the i64 rational API.
    let first_period = floor(span.start.0 / period);
    let period_end = span.end().0 / period;
    let last_period = floor(period_end) + i64::from(period_end.denominator != 1);
    let mut events = Vec::new();
    for period_index in first_period..last_period {
        let mut segment_start = Rational::from_integer(period_index) * period;
        for segment in segments {
            let extent = segment.duration * Rational::from_integer(i64::from(segment.repeats));
            let segment_span = CycleSpan::new(CycleTime(segment_start), CycleDuration(extent));
            if let Some(visible) = intersection(span, segment_span) {
                // Skip repeats outside the query; a large repeat count should not
                // require materializing every earlier occurrence to seek later.
                let first_repeat = floor((visible.start.0 - segment_start) / segment.duration);
                let repeat_end = (visible.end().0 - segment_start) / segment.duration;
                let last_repeat = floor(repeat_end) + i64::from(repeat_end.denominator != 1);
                for repeat in first_repeat..last_repeat {
                    let origin = segment_start + Rational::from_integer(repeat) * segment.duration;
                    let occurrence =
                        CycleSpan::new(CycleTime(origin), CycleDuration(segment.duration));
                    let Some(window) = intersection(span, occurrence) else {
                        continue;
                    };
                    let local_window = window.shift_by(CycleDuration(Rational::zero() - origin));
                    let bounds = CycleSpan::new(
                        CycleTime(Rational::zero()),
                        CycleDuration(segment.duration),
                    );
                    for mut event in query(&segment.node, local_window).events {
                        let Some(clipped) = intersection(event.span, bounds) else {
                            continue;
                        };
                        event.remap_onset(clipped);
                        event = event.shift_by(CycleDuration(origin));
                        events.push(event);
                    }
                }
            }
            segment_start = segment_start + extent;
        }
    }
    PatternStream::new(events)
}

pub(super) fn query(node: &PatternNodeIr, span: CycleSpan) -> PatternStream {
    if span.duration.0 <= Rational::zero() {
        return PatternStream::default();
    }
    let stream = match node {
        PatternNodeIr::FlowProjection {
            control,
            inputs,
            output,
        } => super::flow_query::query(control, inputs, output, span),
        PatternNodeIr::EventStream(node) => in_window(node.stream.clone(), span),
        PatternNodeIr::CycleEventStream(node) => {
            let mut events = Vec::new();
            for event in &node.stream.events {
                // Include a held whole event whose onset precedes the window.
                let first = floor(span.start.0 - event.span.end().0) + 1;
                let end = span.end().0 - event.span.start.0;
                let last = floor(end) + i64::from(end.denominator != 1);
                for cycle in first..last {
                    let mut occurrence = event.clone();
                    occurrence.span = occurrence
                        .span
                        .shift_by(CycleDuration(Rational::from_integer(cycle)));
                    events.push(occurrence);
                }
            }
            PatternStream::new(events)
        }
        PatternNodeIr::ControlStream(node) => query(
            &PatternNodeIr::cycle_event_stream(node.stream.to_pattern_stream()),
            span,
        ),
        PatternNodeIr::ScalarStream(node) => query(
            &PatternNodeIr::cycle_event_stream(node.stream.to_pattern_stream()),
            span,
        ),
        PatternNodeIr::Merge { children } => {
            PatternStream::layer(children.iter().map(|child| query(child, span)).collect())
        }
        PatternNodeIr::PriorityMerge { children, policy } => {
            let mut accepted = Vec::<PatternEvent>::new();
            for child in children {
                for candidate in query(child, span).events {
                    if !accepted.iter().any(|existing| match policy.conflict {
                        PriorityConflictIr::SameWholeStartAndValue => {
                            existing.span.start == candidate.span.start
                                && existing.value == candidate.value
                        }
                        PriorityConflictIr::SameWholeSpanAndValue => {
                            existing.span == candidate.span && existing.value == candidate.value
                        }
                        PriorityConflictIr::WholeSpanOverlap => {
                            intersects(existing.span, candidate.span)
                        }
                    }) {
                        accepted.push(candidate);
                    }
                }
            }
            PatternStream::new(accepted)
        }
        PatternNodeIr::CycleRoute { children } => {
            let mut events = Vec::new();
            if !children.is_empty() {
                for cycle in cycles(span) {
                    let window = intersection(
                        span,
                        unit().shift_by(CycleDuration(Rational::from_integer(cycle))),
                    )
                    .unwrap();
                    events.extend(
                        query(
                            &children[cycle.rem_euclid(children.len() as i64) as usize],
                            window,
                        )
                        .events,
                    );
                }
            }
            PatternStream::new(events)
        }
        PatternNodeIr::Arrange { segments } => arrange(segments, span),
        PatternNodeIr::Sequence { children } => slots(
            &children
                .iter()
                .map(|child| (child.node.as_ref(), child.weight))
                .collect::<Vec<_>>(),
            span,
        ),
        PatternNodeIr::CycleSlots { children } => slots(
            &children
                .iter()
                .map(|child| (child, Rational::one()))
                .collect::<Vec<_>>(),
            span,
        ),
        PatternNodeIr::TimeScale { inner, factor } if *factor > Rational::zero() => {
            let mapped = CycleSpan::new(
                CycleTime(span.start.0 / *factor),
                CycleDuration(span.duration.0 / *factor),
            );
            PatternStream::new(
                query(inner, mapped)
                    .events
                    .into_iter()
                    .map(|event| event.scale_relative_to(CycleTime(Rational::zero()), *factor))
                    .collect(),
            )
        }
        PatternNodeIr::TimeScale { inner, .. } => query(inner, span),
        PatternNodeIr::Shift { inner, offset } => query(
            inner,
            span.shift_by(CycleDuration(Rational::zero() - offset.0)),
        )
        .shift_by(*offset),
        PatternNodeIr::ReflectCycle { inner } => {
            let mut events = Vec::new();
            for cycle in cycles(span) {
                let origin = CycleTime(Rational::from_integer(cycle));
                let end = CycleTime(origin.0 + Rational::one());
                let window =
                    intersection(span, CycleSpan::new(origin, CycleDuration(Rational::one())))
                        .unwrap();
                events.extend(
                    query(inner, window.reverse_within(origin, end))
                        .events
                        .into_iter()
                        .map(|event| event.reverse_within(origin, end)),
                );
            }
            PatternStream::new(events)
        }
        PatternNodeIr::SpaceShift { inner, offset } => query(inner, span).space_shift(*offset),
        PatternNodeIr::SpaceScale { inner, factor } => query(inner, span).space_scale(*factor),
        PatternNodeIr::SpaceReflect { inner, axis } => query(inner, span).space_reflect(*axis),
        PatternNodeIr::Degrade {
            inner,
            keep_probability,
            seed,
        } => query(inner, span).degrade_by_policy(*keep_probability, *seed),
        PatternNodeIr::Deduplicate { inner, policy } => {
            let mut events = query(inner, span).events;
            if policy.winner == DeduplicateWinnerIr::Last {
                events.reverse();
            }
            let mut kept = Vec::<PatternEvent>::new();
            for candidate in events {
                if !kept.iter().any(|existing| match policy.key {
                    // Each IR event occurrence is a distinct authored lifecycle.
                    // Value policies explicitly collapse different occurrences.
                    DeduplicateKeyIr::Lifecycle => false,
                    DeduplicateKeyIr::WholeSpanAndValue => {
                        existing.span == candidate.span && existing.value == candidate.value
                    }
                    DeduplicateKeyIr::StartAndValue => {
                        existing.span.start == candidate.span.start
                            && existing.value == candidate.value
                    }
                }) {
                    kept.push(candidate);
                }
            }
            if policy.winner == DeduplicateWinnerIr::Last {
                kept.reverse();
            }
            PatternStream::new(kept)
        }
        PatternNodeIr::WeightedChoice { options, seed } => {
            let total = options
                .iter()
                .fold(Rational::zero(), |sum, item| sum + item.weight);
            let mut events = Vec::new();
            if total > Rational::zero() {
                for cycle in cycles(span) {
                    let window = intersection(
                        span,
                        unit().shift_by(CycleDuration(Rational::from_integer(cycle))),
                    )
                    .unwrap();
                    let mut pick = choice_roll(*seed, cycle) * total;
                    for option in options {
                        if pick < option.weight {
                            events.extend(query(&option.node, window).events);
                            break;
                        }
                        pick = pick - option.weight;
                    }
                }
            }
            PatternStream::new(events)
        }
        PatternNodeIr::MaskClip { source, mask } => {
            query(source, span).clip_by_mask(&query(mask, span))
        }
        PatternNodeIr::Concat { children } => {
            let mut offset = Rational::zero();
            let mut events = Vec::new();
            for child in children {
                let extent = child.duration().0;
                if let Some(window) = intersection(
                    span,
                    CycleSpan::new(CycleTime(offset), CycleDuration(extent)),
                ) {
                    events.extend(
                        query(
                            child,
                            window.shift_by(CycleDuration(Rational::zero() - offset)),
                        )
                        .shift_by(CycleDuration(offset))
                        .events,
                    );
                }
                offset = offset + extent;
            }
            PatternStream::new(events)
        }
    };
    let mut stream = stream;
    stream
        .events
        .sort_by_key(|event| (event.span.start, event.span.end()));
    stream
}

// Kept explicit and covered by host conformance tests: little-endian FNV-1a
// cycle key, SplitMix64 seed, then a million equally spaced probability steps.
// Neither process-local node IDs nor the queried window enter this decision.
fn choice_roll(seed: u64, cycle: i64) -> Rational {
    let mut key = 0xCBF2_9CE4_8422_2325u64;
    for byte in cycle.to_le_bytes() {
        key = (key ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3);
    }
    let mut value = (seed ^ key).wrapping_add(0x9E37_79B9_7F4A_7C15);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    Rational::new(((value ^ (value >> 31)) % 1_000_000) as i64, 1_000_000)
}
