use crate::{
    application::{
        clock::Clock,
        performer::Performer,
        renderer_core::RendererCore,
        scheduler::events::{
            event_sink::EventSink,
            scheduled_intent::{ScheduledEntry, ScheduledIntent, ScheduledIntentKind},
        },
    },
    domain::{
        control::{ControlKey, ControlTile, ControlTrack, ControlValue, SignedUnitValue},
        intent::Intent,
        prelude::Time,
        score::{ControlScore, Score},
        span::Span,
        voice::{Tile, Voice},
    },
};
use std::time::Instant;

use super::*;

#[derive(Default)]
struct FakeSink {
    events: Vec<ScheduledIntent>,
}

impl EventSink for FakeSink {
    fn schedule(&mut self, event: ScheduledIntent) {
        self.events.push(event);
    }

    fn process_due<P: Performer>(
        &mut self,
        _now: crate::application::clock::ClockTime,
        _performer: &P,
    ) {
    }

    fn clear_pending(&mut self) {
        self.events.clear();
    }
}

fn sample_voice(sample: &str) -> Voice {
    Voice::new(
        Time::ONE,
        vec![Tile::spanning(Time::ZERO, Time::new(1, 2), Intent::sample(sample)).unwrap()],
    )
    .unwrap()
}

#[test]
fn scheduler_panics_on_zero_lookahead_or_step() {
    let renderer = RendererCore::voice(sample_voice("kick"));

    assert!(
        std::panic::catch_unwind(|| {
            let _ = Scheduler::new(renderer.clone(), Time::ZERO, Time::new(1, 2));
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            let _ = Scheduler::new(renderer.clone(), Time::new(1, 4), Time::ZERO);
        })
        .is_err()
    );
}

#[test]
fn poll_clips_visible_span_but_keeps_whole_span() {
    let renderer = RendererCore::voice(sample_voice("pad"));
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler = Scheduler::new(renderer, Time::new(1, 4), Time::new(1, 4));

    let events = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].whole(),
        Span::new(Time::ZERO, Time::new(1, 2)).unwrap()
    );
    assert_eq!(
        events[0].visible(),
        Span::new(Time::ZERO, Time::new(1, 4)).unwrap()
    );
    assert_eq!(events[0].trigger(), Time::ZERO);
}

#[test]
fn poll_keeps_deterministic_order_for_same_start_moments() {
    let renderer = RendererCore::voice(
        Voice::new(
            Time::ONE,
            vec![
                Tile::spanning(Time::ZERO, Time::new(1, 2), Intent::sample("kick"))
                    .unwrap()
                    .with_id(1_u64),
                Tile::spanning(Time::ZERO, Time::ONE, Intent::sample("hat"))
                    .unwrap()
                    .with_id(2_u64),
            ],
        )
        .unwrap(),
    );
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler = Scheduler::new(renderer, Time::new(1, 2), Time::new(1, 2));

    let events = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(events.len(), 2);
    match events[0].intent() {
        Intent::Sample(sample) => assert_eq!(sample.sample_id, "kick"),
        other => panic!("expected sample intent, got {other:?}"),
    }
    match events[1].intent() {
        Intent::Sample(sample) => assert_eq!(sample.sample_id, "hat"),
        other => panic!("expected sample intent, got {other:?}"),
    }
}

#[test]
fn schedule_due_windows_sends_new_visible_events_to_the_sink() {
    let renderer = RendererCore::voice(
        Voice::new(
            Time::ONE,
            vec![
                Tile::spanning(Time::ZERO, Time::new(1, 2), Intent::sample("kick")).unwrap(),
                Tile::spanning(Time::new(1, 2), Time::ONE, Intent::sample("snare")).unwrap(),
            ],
        )
        .unwrap(),
    );
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler = Scheduler::new(renderer, Time::ONE, Time::new(1, 2));
    let mut sink = FakeSink::default();

    scheduler.schedule_due_windows(&clock, &mut sink).unwrap();

    assert_eq!(sink.events.len(), 2);
    assert_eq!(sink.events[0].trigger(), Time::ZERO);
    assert_eq!(sink.events[1].trigger(), Time::new(1, 2));
}

#[test]
fn starting_at_non_zero_backfills_a_still_active_sample() {
    let renderer = RendererCore::voice(sample_voice("pad"));
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler =
        Scheduler::starting_at(renderer, Time::new(1, 4), Time::new(1, 4), Time::new(1, 4));

    let events = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].entry(),
        ScheduledEntry::InProgress {
            elapsed: Time::new(1, 4)
        }
    );
    assert_eq!(events[0].trigger(), Time::ZERO);
    assert_eq!(events[0].entry_time(), Time::new(1, 4));
    assert_eq!(events[0].remaining_duration(), Time::new(1, 4));
    assert_eq!(
        events[0].play_for,
        clock.cycles_to_duration(Time::new(1, 4))
    );
}

#[test]
fn sustained_event_is_emitted_once_while_it_remains_visible() {
    let renderer = RendererCore::voice(
        Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::new(3, 4), Intent::sample("pad")).unwrap()],
        )
        .unwrap(),
    );
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler = Scheduler::new(renderer, Time::new(1, 4), Time::new(1, 4));

    let first = scheduler.poll_next_window(&clock).unwrap();
    let second = scheduler.poll_next_window(&clock).unwrap();
    let third = scheduler.poll_next_window(&clock).unwrap();
    let fourth = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(first.len(), 1);
    assert!(matches!(first[0].entry(), ScheduledEntry::Onset));
    assert!(second.is_empty());
    assert!(third.is_empty());
    assert!(fourth.is_empty());
}

#[test]
fn event_is_emitted_again_after_leaving_the_visible_set() {
    let renderer = RendererCore::voice(sample_voice("pad"));
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler = Scheduler::new(renderer, Time::new(1, 4), Time::new(1, 4));

    let first = scheduler.poll_next_window(&clock).unwrap();
    let second = scheduler.poll_next_window(&clock).unwrap();
    let third = scheduler.poll_next_window(&clock).unwrap();
    let fourth = scheduler.poll_next_window(&clock).unwrap();
    let fifth = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(first.len(), 1);
    assert!(second.is_empty());
    assert!(third.is_empty());
    assert!(fourth.is_empty());
    assert_eq!(fifth.len(), 1);
    assert_eq!(fifth[0].trigger(), Time::ONE);
}

#[test]
fn replacing_a_renderer_mid_sustain_backfills_the_new_active_event() {
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler = Scheduler::starting_at(
        RendererCore::voice(sample_voice("kick")),
        Time::new(1, 4),
        Time::new(1, 4),
        Time::new(1, 4),
    );

    let first = scheduler.poll_next_window(&clock).unwrap();
    scheduler.replace_renderer(RendererCore::voice(sample_voice("snare")), Time::new(1, 4));
    let replaced = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(first.len(), 1);
    assert_eq!(
        first[0].entry(),
        ScheduledEntry::InProgress {
            elapsed: Time::new(1, 4)
        }
    );
    assert_eq!(replaced.len(), 1);
    assert_eq!(
        replaced[0].entry(),
        ScheduledEntry::InProgress {
            elapsed: Time::new(1, 4)
        }
    );
    match replaced[0].intent() {
        Intent::Sample(sample) => assert_eq!(sample.sample_id, "snare"),
        other => panic!("expected sample intent, got {other:?}"),
    }
}

#[test]
fn replacing_a_renderer_resets_scheduler_progress_to_the_given_start() {
    let renderer = RendererCore::voice(sample_voice("kick"));
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler = Scheduler::new(renderer, Time::new(1, 2), Time::new(1, 2));

    let _ = scheduler.poll_next_window(&clock).unwrap();
    scheduler.replace_renderer(RendererCore::voice(sample_voice("snare")), Time::ZERO);
    let events = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(events.len(), 1);
    match events[0].intent() {
        Intent::Sample(sample) => assert_eq!(sample.sample_id, "snare"),
        other => panic!("expected sample intent, got {other:?}"),
    }
}

fn pad_voice_with_pitch_bend_step() -> Score {
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
    Score::with_controls(Score::from(voice), ControlScore::from(controls))
}

#[test]
fn poll_schedules_pitch_bend_update_without_second_start() {
    let renderer = RendererCore::new(pad_voice_with_pitch_bend_step());
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler = Scheduler::new(renderer, Time::ONE, Time::new(1, 2));

    let first = scheduler.poll_next_window(&clock).unwrap();
    let second = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(first.len(), 1);
    assert!(matches!(
        first[0].kind(),
        ScheduledIntentKind::StartVoice { .. }
    ));
    assert_eq!(second.len(), 1);
    assert!(matches!(
        second[0].kind(),
        ScheduledIntentKind::UpdateVoiceControls { .. }
    ));
}

#[test]
fn starting_at_mid_lifecycle_with_gain_segments_backfills_start_voice() {
    let controls = ControlTrack::new(
        Time::ONE,
        vec![
            ControlTile::spanning(
                Time::ZERO,
                Time::new(1, 2),
                ControlKey::Gain,
                ControlValue::Scalar(1.0),
            )
            .unwrap(),
            ControlTile::spanning(
                Time::new(1, 2),
                Time::ONE,
                ControlKey::Gain,
                ControlValue::Scalar(0.5),
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
    let renderer = RendererCore::new(Score::with_controls(
        Score::from(voice),
        ControlScore::from(controls),
    ));
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler =
        Scheduler::starting_at(renderer, Time::new(1, 4), Time::new(1, 4), Time::new(1, 2));

    let events = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0].kind(),
        ScheduledIntentKind::StartVoice { .. }
    ));
    assert_eq!(events[0].entry_time(), Time::new(1, 2));
}

#[test]
fn schedule_due_windows_emits_gain_segment_update_without_restarting_voice() {
    let controls = ControlTrack::new(
        Time::ONE,
        vec![
            ControlTile::spanning(
                Time::ZERO,
                Time::new(1, 2),
                ControlKey::Gain,
                ControlValue::Scalar(1.0),
            )
            .unwrap(),
            ControlTile::spanning(
                Time::new(1, 2),
                Time::ONE,
                ControlKey::Gain,
                ControlValue::Scalar(0.5),
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
    let renderer = RendererCore::new(Score::with_controls(
        Score::from(voice),
        ControlScore::from(controls),
    ));
    let clock = Clock::new(Time::new(1, 1), Instant::now());
    let mut scheduler = Scheduler::new(renderer, Time::ONE, Time::new(1, 2));

    let first = scheduler.poll_next_window(&clock).unwrap();
    let second = scheduler.poll_next_window(&clock).unwrap();

    assert_eq!(first.len(), 1);
    assert!(matches!(
        first[0].kind(),
        ScheduledIntentKind::StartVoice { .. }
    ));
    assert_eq!(second.len(), 1);
    assert!(matches!(
        second[0].kind(),
        ScheduledIntentKind::UpdateVoiceControls { .. }
    ));
}
