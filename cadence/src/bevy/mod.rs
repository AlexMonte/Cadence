//! Bevy ECS integration for Cadence and other hosts.

mod events;
mod plugin;
mod resources;
mod systems;

#[cfg(test)]
mod plugin_tests;

pub use events::{PlaybackStatusChanged, ScoreReplaced};
pub use plugin::{CadencePlugin, CadenceSet};
pub use resources::{ActiveScores, CadenceDiagnostics, PlaybackHandle, PlaybackSync};
pub use systems::{replace_scores_system, tick_playback_system};
