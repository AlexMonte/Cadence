//! Runtime/application layer for projection, scheduling, and performer plumbing.

/// Mixed sample/synth trigger lowering for audio delivery.
pub mod audio;
/// Exact transport clock and timing helpers.
pub mod clock;
/// Engine commands used by host runtimes.
pub mod commands;
/// High-level scheduler/clock engine coordination.
pub mod engine;
/// `Pattern` implementation for the structural `Score` source.
pub mod pattern;
/// Performer trait used to consume scheduled intents.
pub mod performer;
pub(crate) mod query;
pub use query::{EvaluatedEvent, EvaluatedEventKind};
/// Score-first renderer wrapper used by the scheduler.
pub mod renderer_core;
/// Sample-trigger lowering and queue performers.
pub mod sample;
/// Scheduling window logic and scheduled-event types.
pub mod scheduler;
/// Synth-trigger lowering.
pub mod synth;
