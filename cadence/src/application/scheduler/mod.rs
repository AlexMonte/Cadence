//! Scheduling window logic over projected renderer output.

use std::collections::BTreeMap;

use crate::{
    application::{
        audio::VoiceInstanceId,
        clock::Clock,
        query::{EvaluatedEvent, EvaluatedEventKind, QueryKey},
        renderer_core::RendererCore,
        scheduler::events::{
            IDGenerator, event_sink::EventSink, scheduled_intent::ScheduledIntent,
        },
    },
    domain::span::Span,
    domain::{control::ControlModelError, prelude::Time},
};

/// Scheduled-event support types.
pub mod events;

#[cfg(test)]
mod tests;

static SCHEDULED_INTENT_ID_GENERATOR: IDGenerator = IDGenerator::new(10_000);

#[derive(Debug, Clone, Copy)]
struct ActiveStart;

#[derive(Debug, Clone, Copy)]
struct ActiveUpdate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct UpdateScheduleKey {
    voice_id: VoiceInstanceId,
    boundary_start: Time,
}

#[derive(Debug, Clone)]
/// Look-ahead scheduler that polls renderer windows and emits new intents.
pub struct Scheduler {
    renderer: RendererCore,
    active_starts: BTreeMap<QueryKey, ActiveStart>,
    active_updates: BTreeMap<UpdateScheduleKey, ActiveUpdate>,
    active_voices: std::collections::BTreeSet<VoiceInstanceId>,
    next_window_start: Time,
    step: Time,
    look_ahead: Time,
}

impl Scheduler {
    /// Creates a scheduler that starts at transport zero.
    #[must_use]
    pub fn new(renderer: RendererCore, look_ahead: Time, step: Time) -> Self {
        Self::starting_at(renderer, look_ahead, step, Time::ZERO)
    }

    /// Creates a scheduler starting at a specific transport time.
    ///
    /// # Panics
    ///
    /// Panics if `look_ahead <= 0` or `step <= 0`.
    #[must_use]
    pub fn starting_at(renderer: RendererCore, look_ahead: Time, step: Time, start: Time) -> Self {
        assert!(look_ahead > Time::ZERO, "look_ahead must be positive");
        assert!(step > Time::ZERO, "step must be positive");

        Self {
            renderer,
            active_starts: BTreeMap::new(),
            active_updates: BTreeMap::new(),
            active_voices: std::collections::BTreeSet::new(),
            next_window_start: start,
            step,
            look_ahead,
        }
    }

    /// Returns the active renderer.
    #[must_use]
    pub fn renderer(&self) -> &RendererCore {
        &self.renderer
    }

    /// Projects the next scheduling window and returns new intents that have
    /// just become visible.
    pub fn poll_next_window(
        &mut self,
        clock: &Clock,
    ) -> Result<Vec<ScheduledIntent>, ControlModelError> {
        let window_start = self.next_window_start;
        let window_end = self.next_window_start + self.step;
        let window = Span::new(window_start, window_end).unwrap();
        let evaluated = self.renderer.evaluate_window(&window)?;
        let mut current_starts = BTreeMap::new();
        let mut current_updates = BTreeMap::new();
        let mut current_voices = self.active_voices.clone();
        let mut intents = Vec::new();

        for event in evaluated {
            let kind = event.kind().clone();
            match kind {
                EvaluatedEventKind::StartVoice { voice_id, .. } => {
                    let key = event.key();
                    current_starts.insert(key, ActiveStart);
                    current_voices.insert(voice_id);

                    if self.active_starts.contains_key(&key) {
                        continue;
                    }

                    intents.push(ScheduledIntent::from_evaluated(
                        SCHEDULED_INTENT_ID_GENERATOR.next(),
                        event,
                        clock,
                    ));
                }
                EvaluatedEventKind::UpdateVoiceControls {
                    voice_whole,
                    voice_id,
                } => {
                    let update_key = UpdateScheduleKey {
                        voice_id,
                        boundary_start: event.key().whole().start(),
                    };
                    current_updates.insert(update_key, ActiveUpdate);
                    current_voices.insert(voice_id);

                    if self.active_updates.contains_key(&update_key) {
                        continue;
                    }

                    if !self.active_voices.contains(&voice_id) {
                        let start_kind = EvaluatedEventKind::StartVoice {
                            voice_whole,
                            voice_id,
                        };
                        let start_event = EvaluatedEvent::new_with_kind(
                            event.key(),
                            event.into_projected(),
                            start_kind,
                        );
                        let start_key = start_event.key();
                        current_starts.insert(start_key, ActiveStart);

                        if !self.active_starts.contains_key(&start_key) {
                            intents.push(ScheduledIntent::from_evaluated(
                                SCHEDULED_INTENT_ID_GENERATOR.next(),
                                start_event,
                                clock,
                            ));
                        }
                        continue;
                    }

                    intents.push(ScheduledIntent::from_evaluated(
                        SCHEDULED_INTENT_ID_GENERATOR.next(),
                        event,
                        clock,
                    ));
                }
            }
        }

        self.next_window_start = window_end;
        self.active_starts = current_starts;
        self.active_updates = current_updates;
        self.active_voices = current_voices;
        Ok(intents)
    }

    /// Clears active-window state and resumes scheduling from `start`.
    pub fn reset_to(&mut self, start: Time) {
        self.next_window_start = start;
        self.active_starts.clear();
        self.active_updates.clear();
        self.active_voices.clear();
    }

    /// Replaces the renderer and resets scheduling to `start`.
    pub fn replace_renderer(&mut self, renderer: RendererCore, start: Time) {
        self.renderer.replace_renderer(renderer);
        self.reset_to(start);
    }

    fn window_is_due(&self, now: Time) -> bool {
        self.next_window_start + self.step <= now + self.look_ahead
    }

    /// Schedules every window that has entered the look-ahead region.
    pub fn schedule_due_windows<S>(
        &mut self,
        clock: &Clock,
        sink: &mut S,
    ) -> Result<(), ControlModelError>
    where
        S: EventSink,
    {
        let now = clock.now_time();

        while self.window_is_due(now) {
            for intent in self.poll_next_window(clock)? {
                sink.schedule(intent);
            }
        }

        Ok(())
    }
}
