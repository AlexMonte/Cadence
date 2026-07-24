//! Supported host-facing API surface for `cadence`.
//!
//! Host crates should treat this module as the stable integration boundary:
//! - `score` is the primary recursive authoring/query seam
//! - `voice` is the source-only repeating seam
//! - `mosaic` is the thin/lossy transport-time compatibility layer
//! - `projection` is the canonical projected truth layer
//! - `input` is the normalized live note/control seam
//! - `audio` is the portable realtime render seam plus mixed trigger plumbing
//! - `render` is the score-first backend-agnostic projection/query seam
//! - `playback` is the active score-first runtime API, with projected-mosaic
//!   compatibility helpers
//! - `stack` and `score_ext` provide Tessera-style authoring helpers
//! - `compiler` exposes score preview/projection for hosts

mod compiler;
mod score_ext;
mod stack;

pub use compiler::{CadenceCompiler, PreviewReport};
pub use score_ext::{
    PatternExt, ScoreExt, concat, fast, merge, reflect, shift, slow, stack, with_controls,
    with_signal,
};
pub use stack::{cycle, sample, synth, tile, voice};

pub use playback::{
    PlaybackError, PlaybackRuntime, PlaybackSettings, PlaybackState, PlaybackStatus,
};
pub use render::RendererCore;

/// Backend-agnostic audio runtime seam.
pub mod audio;
/// Normalized live note/control input seam.
pub mod input;
/// Thin/lossy projected transport-time output seam.
pub mod mosaic;
/// Host-facing score-first playback runtime.
pub mod playback;
/// Canonical rich projected output seam.
pub mod projection;
/// Score-first projection/query contract.
pub mod render;
/// Primary recursive authoring/query seam.
pub mod score;
/// Source-only repeating voice seam.
pub mod voice;
