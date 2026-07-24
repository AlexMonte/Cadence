#![warn(missing_docs)]

//! Library-first Tessera-oriented voice/runtime kernel.
//!
//! `domain` owns exact musical time, repeating voices, thin moments, canonical
//! projected output, and typed intent.
//! `application` owns projection logic, runtime scheduling, and engine
//! behavior.
//! `adapter` owns concrete external integrations such as sample playback.
//! `infrastructure` is the supported host-facing API surface.
//!
//! Authoring surfaces, ASTs, and notation parsing live above this crate. They
//! should lower typed meaning into `Score`, `ControlScore`, and normalized
//! `InputEvent` values instead of extending the runtime kernel with
//! string-first concerns.
//!
//! Cross-crate integration expectations live in
//! `HOST_API.md` at the crate root. Tessera and Cadence should adapt to
//! that contract instead of depending on implementation details from
//! `application` or `adapter`.

pub mod adapter;
pub mod application;
pub mod domain;
pub mod infrastructure;

#[cfg(feature = "bevy")]
pub mod bevy;

/// Convenient imports for the most commonly used domain and host types.
pub mod prelude {
    pub use crate::application::{EvaluatedEvent, EvaluatedEventKind};
    #[cfg(feature = "bevy")]
    pub use crate::bevy::PlaybackHandle;
    pub use crate::domain::prelude::*;
    pub use crate::infrastructure::{
        CadenceCompiler, PatternExt, PlaybackError, PlaybackRuntime, PlaybackSettings,
        PlaybackState, PlaybackStatus, PreviewReport, RendererCore, ScoreExt, concat, cycle, fast,
        merge, reflect, sample, shift, slow, stack, synth, tile, voice, with_controls, with_signal,
    };
}

/// Convenient imports for host crates using the Bevy integration feature.
#[cfg(feature = "bevy")]
pub mod bevy_prelude {
    pub use crate::bevy::*;
}

pub use infrastructure::{
    CadenceCompiler, PlaybackError, PlaybackRuntime, PlaybackSettings, PlaybackState,
    PlaybackStatus, PreviewReport, RendererCore,
};
