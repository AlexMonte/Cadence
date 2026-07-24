//! Compiled/projected musical output layer.
//!
//! Tessera and host crates should usually author repeating structure as
//! `Voice`s made of phase-local `Tile`s. `Mosaic` remains the thin projected
//! output layer made of transport-time `Moment`s and typed `Intent`s.
//! Hosts that need the full projected meaning of a render window should inspect
//! `infrastructure::projection`. Playback still accepts projected mosaics as a
//! lower-level compatibility path when the host already owns thin transport-time
//! output.

pub use crate::domain::{
    intent::{
        BuiltInSynthSource, GateIntent, Intent, LevelIntent, RateIntent, RegionIntent,
        SampleIntent, SelectIntent, SynthIntent, ToggleIntent,
    },
    moment::{Moment, MomentId},
    mosaic::{Mosaic, ops},
    rational::Time,
    span::Span,
};
