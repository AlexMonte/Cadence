//! Engine command values.

use crate::{application::renderer_core::RendererCore, domain::rational::Time};

#[derive(Debug)]
/// Commands supported by [`crate::application::engine::Engine`].
pub enum EngineCommand {
    /// Stop transport and reset to zero.
    Stop,
    /// Pause transport at its current time.
    Pause,
    /// Resume playback from the current paused/stopped state.
    Resume,
    /// Change the transport rate in cycles per second.
    SetCyclesPerSecond(Time),
    /// Replace the active renderer.
    ReplaceRenderer(RendererCore),
}

/// Backwards-compatible alias for [`EngineCommand`].
pub type Commands = EngineCommand;
