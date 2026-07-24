//! Event sink trait and default in-memory implementation.

use crate::application::{
    clock::ClockTime, performer::Performer, scheduler::events::scheduled_intent::ScheduledIntent,
};

/// Storage for scheduled events waiting to be performed.
pub trait EventSink {
    /// Adds one scheduled event.
    fn schedule(&mut self, event: ScheduledIntent);
    /// Performs and removes every event that is due at `now`.
    fn process_due<P: Performer>(&mut self, now: ClockTime, performer: &P);
    /// Removes all pending events without performing them.
    fn clear_pending(&mut self);
}

#[derive(Default)]
/// Simple in-memory pending-event list.
pub struct Sink {
    events: Vec<ScheduledIntent>,
}

impl Sink {
    /// Creates an empty sink.
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }
}

impl EventSink for Sink {
    fn schedule(&mut self, event: ScheduledIntent) {
        self.events.push(event);
    }

    fn process_due<P>(&mut self, now: ClockTime, performer: &P)
    where
        P: Performer,
    {
        let (due_events, future_events): (Vec<_>, Vec<_>) = std::mem::take(&mut self.events)
            .into_iter()
            .partition(|event| event.is_due(&now));

        self.events = future_events;

        for event in due_events {
            performer.perform(event);
        }
    }

    fn clear_pending(&mut self) {
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{
            clock::{Clock, ClockTime},
            performer::Performer,
        },
        domain::{intent::Intent, rational::Time, span::Span},
    };
    use std::{
        cell::RefCell,
        rc::Rc,
        time::{Duration, Instant},
    };

    #[derive(Clone)]
    struct RecordingPerformer {
        performed_values: Rc<RefCell<Vec<Intent>>>,
    }

    impl RecordingPerformer {
        fn new() -> Self {
            Self {
                performed_values: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl Performer for RecordingPerformer {
        fn perform(&self, event: ScheduledIntent) {
            self.performed_values
                .borrow_mut()
                .push(event.intent().clone());
        }
    }

    fn scheduled_intent(value: Intent, fire_at: ClockTime, deadline: Instant) -> ScheduledIntent {
        let id = super::super::IDGenerator::new(1).next();
        let projected = crate::domain::projection::ProjectedMoment::new(
            crate::domain::moment::Moment::new(Span::new(Time::ZERO, Time::ONE).unwrap(), value),
            Span::new(Time::ZERO, Time::ONE).unwrap(),
            crate::domain::control::ControlMap::new(),
        );

        ScheduledIntent::new(id, projected, fire_at, deadline, Duration::from_millis(250))
    }

    #[test]
    fn not_fired_before_fire_time_even_if_deadline_has_passed() {
        let clock = Clock::new(Time::new(1, 8), Instant::now());
        let mut sink = Sink::new();
        let fire_at = clock.time_for_cycle(Time::new(60, 1));
        let deadline = Instant::now() - Duration::from_secs(1);
        let performer = RecordingPerformer::new();

        sink.schedule(scheduled_intent(Intent::sample("kick"), fire_at, deadline));
        sink.process_due(clock.time_for_cycle(Time::ZERO), &performer);

        assert_eq!(sink.events.len(), 1);
        assert_eq!(sink.events[0].deadline, deadline);
        assert!(sink.events[0].deadline < Instant::now());
        assert!(performer.performed_values.borrow().is_empty());
    }

    #[test]
    fn fired_once_at_or_after_fire_time() {
        let clock = Clock::new(Time::new(1, 1), Instant::now());
        let mut sink = Sink::new();
        let fire_at = clock.time_for_cycle(Time::ZERO);
        let deadline = Instant::now();
        let performer = RecordingPerformer::new();

        sink.schedule(scheduled_intent(Intent::sample("snare"), fire_at, deadline));
        sink.process_due(fire_at, &performer);
        sink.process_due(clock.time_for_cycle(Time::new(1, 16)), &performer);

        assert_eq!(
            performer.performed_values.borrow().as_slice(),
            [Intent::sample("snare")]
        );
    }

    #[test]
    fn removed_after_firing() {
        let clock = Clock::new(Time::new(1, 1), Instant::now());
        let mut sink = Sink::new();
        let fire_at = clock.time_for_cycle(Time::ZERO);
        let deadline = Instant::now();
        let performer = RecordingPerformer::new();

        sink.schedule(scheduled_intent(Intent::sample("hat"), fire_at, deadline));
        assert_eq!(sink.events.len(), 1);

        sink.process_due(fire_at, &performer);

        assert!(sink.events.is_empty());
        assert_eq!(
            performer.performed_values.borrow().as_slice(),
            [Intent::sample("hat")]
        );
    }
}
