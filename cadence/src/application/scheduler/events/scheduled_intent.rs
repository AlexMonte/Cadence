//! Scheduled intent payloads with clock metadata.

use std::time::Duration;

use crate::{
    application::{
        audio::VoiceInstanceId,
        clock::{Clock, ClockTime, Instant},
        query::{EvaluatedEvent, EvaluatedEventKind},
        scheduler::events::EventId,
    },
    domain::{
        control::ControlMap, intent::Intent, moment::MomentId, projection::ProjectedMoment,
        rational::Time, span::TransportSpan,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// How a projected moment enters the current scheduling window.
pub enum ScheduledEntry {
    /// The whole event begins at the visible start.
    Onset,
    /// The event began earlier and the window sees it mid-flight.
    InProgress {
        /// Exact musical time already elapsed before the visible start.
        elapsed: Time,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// What kind of audio-side action this scheduled event represents.
pub enum ScheduledIntentKind {
    /// Start a new voice instance.
    StartVoice {
        /// Full lifecycle span of the voice instance.
        voice_whole: TransportSpan,
        /// Concrete voice instance to start.
        voice_id: VoiceInstanceId,
    },
    /// Update runtime controls on an already-started voice.
    UpdateVoiceControls {
        /// Full lifecycle span of the target voice instance.
        voice_whole: TransportSpan,
        /// Concrete voice instance to update.
        voice_id: VoiceInstanceId,
    },
}

#[derive(Debug, Clone, PartialEq)]
/// Projected moment plus timing metadata for performance.
pub struct ScheduledIntent {
    /// Unique scheduled-event identifier.
    pub id: EventId,
    projected: ProjectedMoment,
    kind: ScheduledIntentKind,
    entry: ScheduledEntry,
    elapsed_for: Duration,
    /// Exact clock time when the event should fire.
    pub fire_at: ClockTime,
    /// Wall-clock deadline derived from `fire_at`.
    pub deadline: Instant,
    /// Remaining wall-clock duration the event should be played for.
    pub play_for: Duration,
}

impl ScheduledIntent {
    /// Creates a scheduled intent from explicit timing metadata.
    #[must_use]
    pub fn new(
        id: EventId,
        projected: ProjectedMoment,
        fire_at: ClockTime,
        deadline: Instant,
        play_for: Duration,
    ) -> Self {
        let kind = ScheduledIntentKind::StartVoice {
            voice_whole: projected.whole(),
            voice_id: VoiceInstanceId::new(id.0),
        };
        Self::new_with_kind(id, projected, kind, fire_at, deadline, play_for)
    }

    /// Creates a scheduled intent with an explicit semantic kind.
    #[must_use]
    pub fn new_with_kind(
        id: EventId,
        projected: ProjectedMoment,
        kind: ScheduledIntentKind,
        fire_at: ClockTime,
        deadline: Instant,
        play_for: Duration,
    ) -> Self {
        let entry = scheduled_entry_for(&projected);

        Self {
            id,
            projected,
            kind,
            entry,
            elapsed_for: Duration::ZERO,
            fire_at,
            deadline,
            play_for,
        }
    }

    /// Creates a scheduled intent from a projected moment and a clock.
    #[must_use]
    pub fn from_projected(id: EventId, projected: ProjectedMoment, clock: &Clock) -> Self {
        let kind = ScheduledIntentKind::StartVoice {
            voice_whole: projected.whole(),
            voice_id: VoiceInstanceId::new(id.0),
        };
        Self::from_projected_with_kind(id, projected, kind, clock)
    }

    /// Creates a scheduled intent with an explicit semantic kind from a
    /// projected moment and a clock.
    #[must_use]
    pub fn from_projected_with_kind(
        id: EventId,
        projected: ProjectedMoment,
        kind: ScheduledIntentKind,
        clock: &Clock,
    ) -> Self {
        let entry_time = projected.visible().start();
        let fire_at = clock.time_for_cycle(entry_time);
        let deadline = clock.instant_for(fire_at);
        let play_for = clock.cycles_to_duration(projected.whole().end() - entry_time);
        let entry = scheduled_entry_for(&projected);
        let elapsed_for = match entry {
            ScheduledEntry::Onset => Duration::ZERO,
            ScheduledEntry::InProgress { elapsed } => clock.cycles_to_duration(elapsed),
        };

        Self {
            id,
            projected,
            kind,
            entry,
            elapsed_for,
            fire_at,
            deadline,
            play_for,
        }
    }

    #[must_use]
    pub(crate) fn from_evaluated(id: EventId, event: EvaluatedEvent, clock: &Clock) -> Self {
        let kind = match event.kind() {
            EvaluatedEventKind::StartVoice {
                voice_whole,
                voice_id,
            } => ScheduledIntentKind::StartVoice {
                voice_whole: *voice_whole,
                voice_id: *voice_id,
            },
            EvaluatedEventKind::UpdateVoiceControls {
                voice_whole,
                voice_id,
            } => ScheduledIntentKind::UpdateVoiceControls {
                voice_whole: *voice_whole,
                voice_id: *voice_id,
            },
        };

        Self::from_projected_with_kind(id, event.into_projected(), kind, clock)
    }

    /// Returns the projected moment payload.
    #[must_use]
    pub fn projected(&self) -> &ProjectedMoment {
        &self.projected
    }

    /// Returns how the event enters the current window.
    #[must_use]
    pub fn entry(&self) -> ScheduledEntry {
        self.entry
    }

    /// Returns the scheduled semantic kind.
    #[must_use]
    pub fn kind(&self) -> ScheduledIntentKind {
        self.kind
    }

    /// Returns the concrete voice instance this scheduled event targets.
    #[must_use]
    pub fn voice_id(&self) -> VoiceInstanceId {
        match self.kind {
            ScheduledIntentKind::StartVoice { voice_id, .. }
            | ScheduledIntentKind::UpdateVoiceControls { voice_id, .. } => voice_id,
        }
    }

    /// Returns the lifecycle span for the targeted voice instance.
    #[must_use]
    pub fn voice_whole(&self) -> TransportSpan {
        match self.kind {
            ScheduledIntentKind::StartVoice { voice_whole, .. }
            | ScheduledIntentKind::UpdateVoiceControls { voice_whole, .. } => voice_whole,
        }
    }

    /// Returns the underlying moment identifier, if any.
    #[must_use]
    pub fn moment_id(&self) -> Option<MomentId> {
        self.projected.id()
    }

    /// Returns the full transport span of the underlying moment.
    #[must_use]
    pub fn whole(&self) -> TransportSpan {
        self.projected.whole()
    }

    /// Returns the visible span used for this scheduling pass.
    #[must_use]
    pub fn visible(&self) -> TransportSpan {
        self.projected.visible()
    }

    /// Returns the musical intent to perform.
    #[must_use]
    pub fn intent(&self) -> &Intent {
        self.projected.intent()
    }

    /// Returns the projected controls active for this event.
    #[must_use]
    pub fn controls(&self) -> &ControlMap {
        self.projected.controls()
    }

    /// Returns the whole-span start time.
    #[must_use]
    pub fn trigger(&self) -> Time {
        self.whole().start()
    }

    /// Returns the visible-span start time.
    #[must_use]
    pub fn entry_time(&self) -> Time {
        self.visible().start()
    }

    /// Returns the full duration of the underlying moment.
    #[must_use]
    pub fn duration(&self) -> Time {
        self.whole().end() - self.whole().start()
    }

    /// Returns the visible duration inside the current window.
    #[must_use]
    pub fn visible_duration(&self) -> Time {
        self.visible().end() - self.visible().start()
    }

    /// Returns the remaining duration from the visible start to the whole end.
    #[must_use]
    pub fn remaining_duration(&self) -> Time {
        self.whole().end() - self.entry_time()
    }

    /// Returns how much wall-clock time had already elapsed before the visible
    /// start.
    #[must_use]
    pub fn elapsed_duration(&self) -> Duration {
        self.elapsed_for
    }

    /// Returns the clock time when this event should fire.
    #[must_use]
    pub fn fired_at(&self) -> ClockTime {
        self.fire_at
    }

    /// Returns the wall-clock deadline associated with `fire_at`.
    #[must_use]
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    /// Returns `true` when the event should be performed at `now`.
    #[must_use]
    pub fn is_due(&self, now: &ClockTime) -> bool {
        self.fire_at <= *now
    }
}

fn scheduled_entry_for(projected: &ProjectedMoment) -> ScheduledEntry {
    if projected.visible().start() == projected.whole().start() {
        ScheduledEntry::Onset
    } else {
        ScheduledEntry::InProgress {
            elapsed: projected.visible().start() - projected.whole().start(),
        }
    }
}
