//! Engine orchestration for clock, scheduler, and performer.

use crate::{
    application::{
        clock::{Clock, ClockTime, Instant},
        commands::EngineCommand,
        performer::Performer,
        renderer_core::RendererCore,
        scheduler::{Scheduler, events::event_sink::EventSink},
    },
    domain::{control::ControlModelError, prelude::Time},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Observable engine transport state.
pub enum EngineStatus {
    /// Transport is actively advancing and scheduling.
    Playing,
    /// Transport is paused at the given clock time.
    Paused(ClockTime),
    /// Transport is stopped and considered to be at zero.
    Stopped,
}

/// Coordinates clock progression, window scheduling, and event performance.
pub struct Engine<S, P>
where
    S: EventSink,
    P: Performer,
{
    /// Window scheduler over the active renderer.
    pub scheduler: Scheduler,
    /// Pending-event storage used between scheduling and performance.
    pub event_sink: S,
    /// Consumer that performs due scheduled intents.
    pub performer: P,
    /// Exact transport clock.
    pub clock: Clock,
    /// Current engine transport status.
    pub status: EngineStatus,
}

impl<S, P> Engine<S, P>
where
    S: EventSink,
    P: Performer,
{
    /// Creates a running engine.
    pub fn new(
        renderer: RendererCore,
        event_sink: S,
        look_ahead: Time,
        cycles_per_second: Time,
        performer: P,
        step: Time,
    ) -> Self {
        let clock = Clock::new(cycles_per_second, Instant::now());
        let scheduler = Scheduler::new(renderer, look_ahead, step);

        Self {
            scheduler,
            clock,
            event_sink,
            performer,
            status: EngineStatus::Playing,
        }
    }

    /// Schedules any due windows and performs all events that are ready now.
    pub fn tick(&mut self) -> Result<(), ControlModelError> {
        if self.status != EngineStatus::Playing {
            return Ok(());
        }

        // Scheduling and performance are split so the engine can preserve exact
        // timing windows while still delegating side effects to the performer.
        self.scheduler
            .schedule_due_windows(&self.clock, &mut self.event_sink)?;
        let now = self.clock.snapshot();
        self.event_sink.process_due(now.time, &self.performer);
        Ok(())
    }

    /// Applies a high-level engine command.
    pub fn command(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::SetCyclesPerSecond(cps) => self.set_cycles_per_second(cps),
            EngineCommand::ReplaceRenderer(renderer) => self.replace_renderer(renderer),
            EngineCommand::Stop => self.stop(),
            EngineCommand::Pause => self.pause(),
            EngineCommand::Resume => self.resume(),
        }
    }

    /// Changes the transport rate while preserving the current musical time.
    pub fn set_cycles_per_second(&mut self, cps: Time) {
        match self.status {
            EngineStatus::Playing => self.clock.set_cycles_per_second(cps),
            EngineStatus::Paused(paused_at) => {
                self.clock.set_cycles_per_second_from_time(cps, paused_at)
            }
            EngineStatus::Stopped => self
                .clock
                .set_cycles_per_second_from_time(cps, ClockTime::ZERO),
        }
    }

    /// Replaces the active renderer and clears pending scheduled events.
    pub fn replace_renderer(&mut self, renderer: RendererCore) {
        let start = match self.status {
            EngineStatus::Playing => self.clock.now_time(),
            EngineStatus::Paused(paused_at) => paused_at.as_time(),
            EngineStatus::Stopped => Time::ZERO,
        };

        // Replacing the renderer invalidates any pending scheduled events,
        // so we rebuild scheduling from the correct transport position.
        self.scheduler.replace_renderer(renderer, start);
        self.event_sink.clear_pending();
    }

    /// Stops playback and resets the scheduler and clock to zero.
    pub fn stop(&mut self) {
        self.status = EngineStatus::Stopped;
        self.clock.reset();
        self.scheduler.reset_to(Time::ZERO);
        self.event_sink.clear_pending();
    }

    /// Pauses playback at the current transport position.
    pub fn pause(&mut self) {
        if let EngineStatus::Playing = self.status {
            let paused_at = self.clock.now();
            self.status = EngineStatus::Paused(paused_at);
            // Re-anchor the clock so resuming continues from the paused musical
            // time instead of reusing stale elapsed wall time.
            self.clock.reset_at(paused_at);
            self.scheduler.reset_to(paused_at.as_time());
            self.event_sink.clear_pending();
        }
    }

    /// Resumes playback from the current paused or stopped state.
    pub fn resume(&mut self) {
        match self.status {
            EngineStatus::Playing => {}
            EngineStatus::Paused(paused_at) => {
                // The scheduler is rewound to the paused time so the next tick
                // can rebuild the look-ahead window from that exact point.
                self.clock.reset_at(paused_at);
                self.scheduler.reset_to(paused_at.as_time());
                self.status = EngineStatus::Playing;
            }
            EngineStatus::Stopped => {
                self.clock.reset();
                self.scheduler.reset_to(Time::ZERO);
                self.status = EngineStatus::Playing;
            }
        }
    }

    /// Returns the current engine status.
    #[must_use]
    pub fn status(&self) -> EngineStatus {
        self.status
    }

    /// Returns the current musical transport time.
    #[must_use]
    pub fn current_time(&self) -> Time {
        match self.status {
            EngineStatus::Playing => self.clock.now_time(),
            EngineStatus::Paused(paused_at) => paused_at.as_time(),
            EngineStatus::Stopped => Time::ZERO,
        }
    }

    /// Returns the current transport rate in cycles per second.
    #[must_use]
    pub fn cycles_per_second(&self) -> Time {
        self.clock.cycles_per_second()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{
            commands::EngineCommand,
            performer::Performer,
            scheduler::events::{event_sink::Sink, scheduled_intent::ScheduledIntent},
        },
        domain::{
            intent::Intent,
            prelude::Time,
            score::Score,
            voice::{Tile, Voice},
        },
    };
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone)]
    struct RecordingPerformer {
        intents: Rc<RefCell<Vec<Intent>>>,
    }

    impl RecordingPerformer {
        fn new() -> Self {
            Self {
                intents: Rc::new(RefCell::new(Vec::new())),
            }
        }
    }

    impl Performer for RecordingPerformer {
        fn perform(&self, event: ScheduledIntent) {
            self.intents.borrow_mut().push(event.intent().clone());
        }
    }

    fn sample_voice(sample: &str) -> Voice {
        Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::new(1, 8), Intent::sample(sample)).unwrap()],
        )
        .unwrap()
    }

    fn test_engine() -> Engine<Sink, RecordingPerformer> {
        let renderer = RendererCore::voice(sample_voice("kick"));
        let sink = Sink::new();
        let performer = RecordingPerformer::new();

        Engine::new(
            renderer,
            sink,
            Time::ONE,
            Time::new(4, 1),
            performer,
            Time::new(1, 8),
        )
    }

    #[test]
    fn pause_stops_ticks_without_scheduling_new_events() {
        let mut engine = test_engine();
        let performed = Rc::clone(&engine.performer.intents);

        engine.command(EngineCommand::Pause);
        engine.tick().unwrap();

        assert!(matches!(engine.status, EngineStatus::Paused(_)));
        assert!(performed.borrow().is_empty());
    }

    #[test]
    fn resume_restores_playing_state_after_pause() {
        let mut engine = test_engine();

        engine.command(EngineCommand::Pause);
        engine.command(EngineCommand::Resume);
        engine.tick().unwrap();

        assert_eq!(engine.status, EngineStatus::Playing);
    }

    #[test]
    fn stop_resets_clock_and_scheduler_to_zero() {
        let mut engine = test_engine();
        let performed = Rc::clone(&engine.performer.intents);

        engine.tick().unwrap();
        engine.command(EngineCommand::Stop);
        engine.command(EngineCommand::Resume);
        engine.tick().unwrap();

        assert_eq!(engine.clock.started_at_time(), ClockTime::ZERO);
        assert_eq!(performed.borrow().len(), 2);
    }

    #[test]
    fn command_replaces_renderer_immediately() {
        let mut engine = test_engine();
        let snare = sample_voice("snare");

        engine.command(EngineCommand::ReplaceRenderer(RendererCore::voice(
            snare.clone(),
        )));

        assert_eq!(engine.scheduler.renderer().score(), &Score::from(snare));
    }
}
