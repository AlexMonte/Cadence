use bevy_ecs::prelude::*;
use bevy_ecs::reflect::ReflectResource;
use bevy_reflect::Reflect;

use crate::infrastructure::PreparedScore;

/// Prepared score currently proposed by the host.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct ActiveScores {
    /// Monotonic revision bumped whenever scores change.
    pub revision: u64,
    /// Number of host output lanes represented by the prepared score.
    pub output_count: usize,
    /// Merged score ready for playback without another structural scan.
    #[reflect(ignore)]
    score: PreparedScore,
}

impl ActiveScores {
    /// Replaces the active proposal and bumps the revision.
    pub fn replace(&mut self, output_count: usize, score: PreparedScore) {
        self.revision = self.revision.saturating_add(1);
        self.output_count = output_count;
        self.score = score;
    }

    /// Borrows the prepared merged score.
    #[must_use]
    pub fn score(&self) -> &PreparedScore {
        &self.score
    }
}

/// Tracks the last score revision applied to playback.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct PlaybackSync {
    /// Revision last merged into the active playback score.
    pub last_applied_revision: u64,
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
/// Runtime diagnostics surfaced by the Bevy playback integration.
pub struct CadenceDiagnostics {
    /// Latest playback tick error message, if any.
    pub last_tick_error: Option<String>,
}
