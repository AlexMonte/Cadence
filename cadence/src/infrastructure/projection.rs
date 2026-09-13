//! Canonical projected transport-time output.
//!
//! Hosts that need the full projected meaning of a render window inspect the
//! ordered `EvaluatedEvent` values returned by `CadenceCompiler` or
//! `RendererCore`. Each event carries one `ProjectedMoment`.

pub use crate::domain::{
    control::{ControlKey, ControlMap, ControlValue, Symbol, UnitValue},
    intent::{
        BuiltInSynthSource, GateIntent, Intent, LevelIntent, RateIntent, RegionIntent,
        SampleIntent, SelectIntent, SynthIntent, ToggleIntent,
    },
    moment::{Moment, MomentId},
    projection::ProjectedMoment,
    rational::Time,
    span::{Span, TransportSpan},
};
