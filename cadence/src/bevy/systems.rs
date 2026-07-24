use bevy_ecs::prelude::*;

use super::events::{PlaybackStatusChanged, ScoreReplaced};
use super::resources::{ActiveScores, CadenceDiagnostics, PlaybackHandle, PlaybackSync};
use crate::infrastructure::playback::{PlaybackRuntime, PlaybackState};

/// Advances playback scheduling when transport is playing.
pub fn tick_playback_system(
    mut runtime: NonSendMut<PlaybackRuntime>,
    mut diagnostics: ResMut<CadenceDiagnostics>,
    mut status_events: MessageWriter<PlaybackStatusChanged>,
    mut last_playing: Local<Option<bool>>,
) {
    let playing = matches!(runtime.status().state, PlaybackState::Playing);
    if last_playing.as_ref() != Some(&playing) {
        status_events.write(PlaybackStatusChanged { playing });
        *last_playing = Some(playing);
    }

    if !playing {
        return;
    }

    if let Err(error) = runtime.tick() {
        diagnostics.last_tick_error = Some(error.to_string());
    }
}

/// Applies newly lowered scores to the playback runtime.
pub fn replace_scores_system(
    scores: Res<ActiveScores>,
    mut runtime: NonSendMut<PlaybackRuntime>,
    mut sync: ResMut<PlaybackSync>,
    mut replaced_events: MessageWriter<ScoreReplaced>,
    mut diagnostics: ResMut<CadenceDiagnostics>,
) {
    if scores.revision == sync.last_applied_revision {
        return;
    }

    match PlaybackHandle::replace_merged_scores(&mut runtime, scores.scores.clone()) {
        Ok(()) => {
            sync.last_applied_revision = scores.revision;
            diagnostics.last_tick_error = None;
            replaced_events.write(ScoreReplaced {
                output_count: scores.scores.len(),
            });
        }
        Err(error) => {
            diagnostics.last_tick_error = Some(error.to_string());
        }
    }
}
