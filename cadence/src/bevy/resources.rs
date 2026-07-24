use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use bevy_ecs::reflect::ReflectResource;
use bevy_reflect::Reflect;

use crate::{
    domain::score::Score,
    infrastructure::{
        merge,
        playback::{PlaybackError, PlaybackRuntime, PlaybackSettings},
    },
};

/// Active lowered scores keyed by output id.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct ActiveScores {
    /// Monotonic revision bumped whenever scores change.
    pub revision: u64,
    /// Lowered output scores ready for playback.
    #[reflect(ignore)]
    pub scores: BTreeMap<String, Score>,
}

impl ActiveScores {
    /// Replaces all active scores and bumps the revision.
    pub fn replace(&mut self, scores: BTreeMap<String, Score>) {
        self.revision = self.revision.saturating_add(1);
        self.scores = scores;
    }
}

/// Tracks the last score revision applied to playback.
#[derive(Resource, Default, Reflect)]
#[reflect(Resource)]
pub struct PlaybackSync {
    /// Revision last merged into the active playback score.
    pub last_applied_revision: u64,
}

/// Host helper for installing a non-send playback runtime resource.
pub struct PlaybackHandle;

impl PlaybackHandle {
    /// Creates playback settings and runtime from host audio control.
    #[must_use]
    pub fn new(
        settings: PlaybackSettings,
        audio: crate::adapter::audio::AudioControl,
    ) -> PlaybackRuntime {
        PlaybackRuntime::new(settings, audio)
    }

    /// Merges lowered output scores into one playback score tree.
    pub fn replace_merged_scores(
        runtime: &mut PlaybackRuntime,
        scores: BTreeMap<String, Score>,
    ) -> Result<(), PlaybackError> {
        let merged = if scores.is_empty() {
            Score::empty()
        } else {
            merge(scores.into_values().collect())
        };
        runtime.replace_score(merged)
    }
}

#[derive(Resource, Reflect, Default)]
#[reflect(Resource)]
/// Runtime diagnostics surfaced by the Bevy playback integration.
pub struct CadenceDiagnostics {
    /// Latest playback tick error message, if any.
    pub last_tick_error: Option<String>,
}
