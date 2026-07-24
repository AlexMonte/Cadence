//! Source-only repeating voice seam.

pub use crate::domain::{
    intent::{
        BuiltInSynthSource, GateIntent, Intent, LevelIntent, RateIntent, RegionIntent,
        SampleIntent, SelectIntent, SynthIntent, ToggleIntent,
    },
    rational::Time,
    span::{Phase, PhaseSpan, Span, Transport, TransportSpan},
    voice::{Repeat, Tile, TileId, Voice, VoiceId, ops},
};
