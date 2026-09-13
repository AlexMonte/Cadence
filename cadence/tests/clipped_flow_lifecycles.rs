//! Flow policies preserve held-note lifecycle decisions in interior queries.
use cadence::{
    application::{
        clock::Clock,
        scheduler::{
            Scheduler,
            events::scheduled_intent::{ScheduledEntry, ScheduledIntentKind},
        },
    },
    prelude::*,
};
use std::time::Instant;

fn span(start: Time, end: Time) -> TransportSpan {
    TransportSpan::new(start, end).unwrap()
}
fn held(id: u64, first_gain: f64, second_gain: f64) -> Score {
    let period = Time::whole_number(2);
    let voice = Voice::new(
        period,
        vec![
            Tile::spanning(Time::ZERO, period, Intent::sample("tone"))
                .unwrap()
                .with_id(id)
                .with_value_identity("c4"),
        ],
    )
    .unwrap();
    let controls = ControlTrack::new(
        period,
        vec![
            ControlTile::spanning(
                Time::ZERO,
                Time::ONE,
                ControlKey::Gain,
                ControlValue::Scalar(first_gain),
            )
            .unwrap(),
            ControlTile::spanning(
                Time::ONE,
                period,
                ControlKey::Gain,
                ControlValue::Scalar(second_gain),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    Score::with_controls(voice.into(), controls.into())
}
fn query(score: &Score, window: TransportSpan) -> PreviewReport {
    let prepared = PreparedScore::new(score.clone()).unwrap();
    CadenceCompiler::new().preview(&prepared, &window).unwrap()
}
fn lifecycle(event: &EvaluatedEvent) -> (i64, TransportSpan) {
    match event.kind() {
        EvaluatedEventKind::StartVoice {
            voice_whole,
            voice_id,
        }
        | EvaluatedEventKind::UpdateVoiceControls {
            voice_whole,
            voice_id,
        } => (voice_id.value(), *voice_whole),
    }
}
fn clipped_snapshot(
    report: &PreviewReport,
    window: TransportSpan,
) -> Vec<((i64, TransportSpan), TransportSpan, ControlMap)> {
    report
        .events
        .iter()
        .filter_map(|event| {
            Some((
                lifecycle(event),
                event.projected().visible().intersection(&window)?,
                event.projected().controls().clone(),
            ))
        })
        .collect()
}
fn assert_slices(score: &Score) -> PreviewReport {
    let full = query(score, span(Time::ZERO, Time::whole_number(2)));
    for (start, end) in [(1, 3), (3, 5), (5, 7), (1, 7)] {
        let window = span(Time::new(start, 4), Time::new(end, 4));
        let partial = query(score, window);
        assert_eq!(
            clipped_snapshot(&partial, window),
            clipped_snapshot(&full, window)
        );
        assert!(
            partial.events.iter().all(|event| matches!(
                event.kind(),
                EvaluatedEventKind::UpdateVoiceControls { .. }
            )),
            "clipped projection must not invent a start"
        );
    }
    full
}

#[test]
fn deduplicate_winners_keep_all_updates_in_mid_note_queries() {
    for key in [
        DeduplicateKey::WholeSpanAndIntent,
        DeduplicateKey::StartAndIntent,
        DeduplicateKey::WholeSpanAndValue,
        DeduplicateKey::StartAndValue,
    ] {
        for (winner, gains, id) in [
            (DeduplicateWinner::First, [0.25, 0.5], 11),
            (DeduplicateWinner::Last, [0.75, 1.0], 12),
        ] {
            let score = Score::deduplicate(
                Score::merge(vec![held(11, 0.25, 0.5), held(12, 0.75, 1.0)]),
                DeduplicatePolicy::new(key, winner),
            );
            let report = assert_slices(&score);
            assert_eq!(report.starts().count(), 1);
            assert_eq!(report.control_updates().count(), 1);
            assert_eq!(
                report
                    .events
                    .iter()
                    .map(|event| event.projected().controls()[&ControlKey::Gain].clone())
                    .collect::<Vec<_>>(),
                gains.map(ControlValue::Scalar)
            );
            assert!(
                report
                    .events
                    .iter()
                    .all(|event| event.projected().id() == Some(MomentId::new(id)))
            );
        }
    }
}

#[test]
fn priority_merge_retains_the_same_winning_lifecycle_after_mid_note_seek() {
    for conflict in [
        ConflictPolicy::SameWholeStartAndIntent,
        ConflictPolicy::SameWholeSpanAndIntent,
        ConflictPolicy::SameWholeStartAndValue,
        ConflictPolicy::SameWholeSpanAndValue,
        ConflictPolicy::WholeSpanOverlap,
    ] {
        let score = Score::priority_merge(
            vec![held(11, 0.25, 0.5), held(12, 0.75, 1.0)],
            PriorityMergePolicy::new(conflict),
        );
        let report = assert_slices(&score);
        assert_eq!(report.events.len(), 2);
        assert!(
            report
                .events
                .iter()
                .all(|event| event.projected().id() == Some(MomentId::new(11)))
        );
    }
}

#[test]
fn degrade_keeps_or_drops_a_whole_lifecycle_in_every_split_window() {
    let source = held(11, 0.25, 0.5);
    let mut kept = 0;
    for seed in 0..64 {
        let score = Score::degrade(source.clone(), DegradePolicy::new(Time::new(1, 2), seed));
        let full = assert_slices(&score);
        assert!(full.events.is_empty() || full.events.len() == 2);
        kept += usize::from(!full.events.is_empty());
    }
    assert!(
        kept > 0 && kept < 64,
        "exercise both kept and dropped lifecycles"
    );
}

#[test]
fn scheduler_backfills_once_and_updates_without_restarting_after_flow_filtering() {
    let left = held(11, 0.25, 0.5);
    let right = held(12, 0.75, 1.0);
    let dedup = Score::deduplicate(
        Score::merge(vec![left.clone(), right.clone()]),
        DeduplicatePolicy::new(DeduplicateKey::WholeSpanAndValue, DeduplicateWinner::Last),
    );
    let priority = Score::priority_merge(
        vec![left.clone(), right],
        PriorityMergePolicy::new(ConflictPolicy::WholeSpanOverlap),
    );
    let degraded = (0..64)
        .map(|seed| Score::degrade(left.clone(), DegradePolicy::new(Time::new(1, 2), seed)))
        .find(|score| {
            !query(score, span(Time::ZERO, Time::whole_number(2)))
                .events
                .is_empty()
        })
        .unwrap();
    for score in [dedup, priority, degraded] {
        let clock = Clock::new(Time::ONE, Instant::now());
        let mut scheduler = Scheduler::starting_at(
            RendererCore::new(score),
            Time::new(1, 4),
            Time::new(1, 4),
            Time::new(1, 4),
        );
        let mut scheduled = Vec::new();
        for _ in 0..7 {
            scheduled.extend(scheduler.poll_next_window(&clock).unwrap());
        }
        let starts: Vec<_> = scheduled
            .iter()
            .filter(|event| matches!(event.kind(), ScheduledIntentKind::StartVoice { .. }))
            .collect();
        assert_eq!(starts.len(), 1);
        assert_eq!(starts[0].entry_time(), Time::new(1, 4));
        assert_eq!(
            starts[0].entry(),
            ScheduledEntry::InProgress {
                elapsed: Time::new(1, 4)
            }
        );
        let updates: Vec<_> = scheduled
            .iter()
            .filter(|event| {
                matches!(
                    event.kind(),
                    ScheduledIntentKind::UpdateVoiceControls { .. }
                )
            })
            .collect();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].entry_time(), Time::ONE);
        assert_eq!(updates[0].voice_id(), starts[0].voice_id());
    }
}

#[test]
fn transformed_held_notes_keep_lifecycle_identity_when_only_updates_are_visible() {
    for (source, whole, interior) in [
        (
            Score::shift(held(11, 0.25, 0.5), Time::new(1, 4)),
            span(Time::new(1, 4), Time::new(9, 4)),
            span(Time::new(1, 2), Time::new(3, 4)),
        ),
        (
            Score::time_scale(held(11, 0.25, 0.5), Time::whole_number(2)),
            span(Time::ZERO, Time::ONE),
            span(Time::new(1, 4), Time::new(1, 2)),
        ),
    ] {
        let score = Score::deduplicate(
            source,
            DeduplicatePolicy::new(DeduplicateKey::Lifecycle, DeduplicateWinner::First),
        );
        let full = query(&score, whole);
        let partial = query(&score, interior);
        assert_eq!(full.starts().count(), 1);
        assert_eq!(partial.starts().count(), 0);
        assert_eq!(partial.events.len(), 1);
        assert_eq!(
            clipped_snapshot(&partial, interior),
            clipped_snapshot(&full, interior)
        );
    }
}

#[test]
fn transformed_updates_target_the_single_backfilled_voice_at_the_mapped_boundary() {
    for (score, start, step, update_at, whole) in [
        (
            Score::shift(held(11, 0.25, 0.5), Time::new(1, 4)),
            Time::new(1, 2),
            Time::new(1, 4),
            Time::new(5, 4),
            span(Time::new(1, 4), Time::new(9, 4)),
        ),
        (
            Score::time_scale(held(11, 0.25, 0.5), Time::whole_number(2)),
            Time::new(1, 8),
            Time::new(1, 8),
            Time::new(1, 2),
            span(Time::ZERO, Time::ONE),
        ),
    ] {
        let clock = Clock::new(Time::ONE, Instant::now());
        let mut scheduler = Scheduler::starting_at(RendererCore::new(score), step, step, start);
        let mut events = Vec::new();
        for _ in 0..7 {
            events.extend(scheduler.poll_next_window(&clock).unwrap());
        }
        assert_eq!(events.len(), 2);
        assert!(
            matches!(events[0].kind(), ScheduledIntentKind::StartVoice {voice_whole,..} if voice_whole == whole)
        );
        assert_eq!(events[0].entry_time(), start);
        assert_eq!(
            events[0].entry(),
            ScheduledEntry::InProgress {
                elapsed: start - whole.start()
            }
        );
        assert!(
            matches!(events[1].kind(), ScheduledIntentKind::UpdateVoiceControls {voice_whole,..} if voice_whole == whole)
        );
        assert_eq!(events[1].entry_time(), update_at);
        assert_eq!(events[1].voice_id(), events[0].voice_id());
    }
}
