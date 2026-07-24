use bevy_app::{App, Plugin, Update};
use bevy_ecs::schedule::IntoScheduleConfigs;

use super::events::{PlaybackStatusChanged, ScoreReplaced};
use super::resources::{ActiveScores, CadenceDiagnostics, PlaybackSync};
use super::systems::{replace_scores_system, tick_playback_system};

/// Registers Cadence playback resources and systems.
///
/// Hosts must insert [`super::resources::PlaybackHandle`] after creating audio
/// output (`AudioRenderer::split`) because device setup remains host-owned.
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
            .add_systems(
                Update,
                (replace_scores_system, tick_playback_system).chain(),
            );
    }
}
