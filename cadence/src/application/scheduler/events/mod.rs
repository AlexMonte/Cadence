//! Scheduled-event support types.

use std::sync::atomic::{AtomicI64, Ordering};

/// Event sink trait and default in-memory sink.
pub mod event_sink;
/// Scheduled intent payload and timing metadata.
pub mod scheduled_intent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Opaque identifier for a scheduled event.
pub struct EventId(i64);

/// Thread-safe monotonic event identifier generator.
pub struct IDGenerator {
    next: AtomicI64,
}

impl IDGenerator {
    /// Creates a generator whose next produced value starts at `start`.
    pub const fn new(start: i64) -> Self {
        Self {
            next: AtomicI64::new(start),
        }
    }

    /// Produces the next event identifier.
    pub fn next(&self) -> EventId {
        EventId(self.next.fetch_add(1, Ordering::Relaxed))
    }
}
