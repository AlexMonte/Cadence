//! Primary recursive authoring/query seam.

pub use crate::domain::{
    control::{
        CompressorSettings, ControlKey, ControlMap, ControlTile, ControlTileId, ControlTrack,
        ControlTrackId, ControlValue, DelaySettings, ReverbSettings, SignedUnitValue, Symbol,
        UnitValue,
    },
    mosaic::Mosaic,
    rational::Time,
    score::{
        ConflictPolicy, ControlScore, DeduplicateKey, DeduplicatePolicy, DeduplicateWinner,
        DegradePolicy, PriorityMergePolicy, Score, WeightedControlScore, WeightedScore,
    },
    span::{Phase, PhaseSpan, Span, Transport, TransportSpan},
    voice::{Repeat, Tile, TileId, Voice, VoiceId, ops},
};
