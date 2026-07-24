use bevy_app::{App, Plugin, Update};
use bevy_ecs::schedule::{IntoScheduleConfigs, SystemSet};

use super::events::{PlaybackStatusChanged, ScoreReplaced};
use super::resources::{ActiveScores, CadenceDiagnostics, PlaybackSync};
use super::systems::{replace_scores_system, tick_playback_system};

/// Host-nestable Cadence Update phases.
///
/// Split so a host can run transport sync between score replace and tick
/// (e.g. Musaic: `ReplaceScores` → transport → `Tick` → clock readback).
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CadenceSet {
    /// Apply [`super::resources::ActiveScores`] revisions to the playback runtime.
    ReplaceScores,
    /// Advance playback when transport is playing.
    Tick,
}

/// Registers Cadence playback resources and systems.
///
/// Hosts must insert [`super::resources::PlaybackHandle`] after creating audio
/// output (`AudioRenderer::split`) because device setup remains host-owned.
/// Hosts that own frame order should nest [`CadenceSet`] into their schedule
/// (Musaic: both variants under `MusaicSet::Runtime`).
pub struct CadencePlugin;

impl Plugin for CadencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveScores>()
            .init_resource::<CadenceDiagnostics>()
            .init_resource::<PlaybackSync>()
            .register_type::<ActiveScores>()
            .register_type::<CadenceDiagnostics>()
            .register_type::<PlaybackSync>()
            .add_message::<ScoreReplaced>()
            .add_message::<PlaybackStatusChanged>()
            .configure_sets(
                Update,
                (CadenceSet::ReplaceScores, CadenceSet::Tick).chain(),
            )
            .add_systems(
                Update,
                (
                    replace_scores_system.in_set(CadenceSet::ReplaceScores),
                    tick_playback_system.in_set(CadenceSet::Tick),
                ),
            );
    }
}
