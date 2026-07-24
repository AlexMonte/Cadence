//! Core musical domain model.
//!
//! This module holds the exact, backend-agnostic concepts that the rest of the
//! crate builds on: time, spans, repeating voices, projected moments, typed
//! control values, and score trees.

/// Backwards-compatible transport-time span alias.
pub mod arc;
/// Typed control keys, values, and repeating control tracks.
pub mod control;
/// Small shared error type used by older seams in the crate.
pub mod error;
/// Normalized live note and control input data.
pub mod input;
/// Typed musical intents carried by moments and tiles.
pub mod intent;
/// Thin transport-time musical events.
pub mod moment;
/// Ordered collections of transport-time moments.
pub mod mosaic;
/// Query-over-span pattern trait shared by signals and scores.
pub mod pattern;
/// Rich projected output that keeps clipped visibility and projected controls.
pub mod projection;
/// Exact rational time primitives.
pub mod rational;
/// Recursive source and control score trees.
pub mod score;
/// Continuous signal sources evaluated as pure functions of cycle time.
pub mod signal;
/// Exact spatial position and motion over the lattice.
pub mod space;
/// Typed time spans for phase-local and transport-time reasoning.
pub mod span;
/// Repeating voice source model and source-level transforms.
pub mod voice;

/// Convenient re-exports for the most commonly used domain types.
pub mod prelude {
    pub use crate::domain::arc::Arc;
    pub use crate::domain::control::{
        CompressorSettings, ControlKey, ControlMap, ControlMerge, ControlModelError, ControlSpec,
        ControlSupport, ControlTile, ControlTileId, ControlTiming, ControlTrack, ControlTrackId,
        ControlValue, ControlValueKind, DelaySettings, ReverbSettings, SignedUnitValue, Symbol,
        UnitValue,
    };
    pub use crate::domain::input::{
        ControlInput, HeldNotes, InputChannel, InputEvent, InputSourceId, MidiBinding,
        MidiBindings, MidiMessage, MidiValueKind, NoteInput, NoteNumber, SampleNoteBinding,
        SampleNoteMap, SampleNoteRule, Velocity,
    };
    pub use crate::domain::intent::{
        BuiltInSynthSource, GateIntent, Intent, LevelIntent, RateIntent, RegionIntent,
        SampleIntent, SelectIntent, SynthIntent, ToggleIntent,
    };
    pub use crate::domain::moment::{Moment, MomentId};
    pub use crate::domain::mosaic::Mosaic;
    pub use crate::domain::pattern::Pattern;
    pub use crate::domain::projection::{ProjectedMoment, ProjectedMosaic};
    pub use crate::domain::rational::{Coord, Time};
    pub use crate::domain::score::{
        ConflictPolicy, ControlScore, ControlScoreNodeId, DeduplicateKey, DeduplicatePolicy,
        DeduplicateWinner, DegradePolicy, PriorityMergePolicy, Score, ScoreNodeId,
        WeightedControlScore, WeightedScore,
    };
    pub use crate::domain::signal::{Signal, Waveform};
    pub use crate::domain::space::{Axis, Point3, SpatialMotion};
    pub use crate::domain::span::{Phase, PhaseSpan, Span, Transport, TransportSpan};
    pub use crate::domain::voice::{Repeat, Tile, TileId, Voice, VoiceId};
    pub use crate::domain::{gcd, lcm};
}

/// Returns the greatest common divisor using Euclid's algorithm.
///
/// This is used heavily by [`crate::domain::rational::Time`] normalization.
pub fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 {
        return a;
    } else {
        return gcd(b, a % b);
    }
}

/// Returns the least common multiple.
///
/// This is primarily useful when aligning rational denominators.
pub fn lcm(a: i64, b: i64) -> i64 {
    a / gcd(a, b) * b
}
