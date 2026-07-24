//! Canonical projected transport-time output.
//!
//! Hosts that need the full projected meaning of a render window should inspect
//! `ProjectedMoment` / `ProjectedMosaic`. Thin `Moment` / `Mosaic` values
//! remain available as lossy transport-time compatibility views.

pub use crate::domain::{
    control::{ControlKey, ControlMap, ControlValue, Symbol, UnitValue},
    intent::{
        BuiltInSynthSource, GateIntent, Intent, LevelIntent, RateIntent, RegionIntent,
        SampleIntent, SelectIntent, SynthIntent, ToggleIntent,
    },
    moment::{Moment, MomentId},
    projection::{ProjectedMoment, ProjectedMosaic},
    rational::Time,
    span::{Span, TransportSpan},
};
