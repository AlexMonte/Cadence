#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use bevy_app::App;
    use bevy_ecs::system::RunSystemOnce;

    use crate::{
        adapter::audio::SampleBuffer,
        adapter::audio::{AudioRenderer, AudioRendererSettings, Frame},
        bevy::{
            events::ScoreReplaced,
            plugin::CadencePlugin,
            resources::{ActiveScores, PlaybackSync},
            systems::replace_scores_system,
        },
        domain::{
            intent::Intent,
            prelude::Time,
            score::Score,
            voice::{Tile, Voice},
        },
        infrastructure::playback::{PlaybackRuntime, PlaybackSettings},
    };

    fn test_playback_runtime() -> PlaybackRuntime {
        let (audio, _renderer) =
            AudioRenderer::split(AudioRendererSettings::new(8_000, 64)).unwrap();
        let mut runtime = PlaybackRuntime::new(PlaybackSettings::default(), audio);
        runtime.load_sample(
            "kick",
            SampleBuffer::new(8_000, vec![Frame::from_mono(0.0), Frame::from_mono(0.5)]),
        );
        runtime
    }

    #[test]
    fn replace_scores_system_applies_active_scores_revision() {
        let mut app = App::new();
        app.add_plugins(CadencePlugin);
        app.insert_non_send_resource(test_playback_runtime());
        app.insert_resource(ActiveScores::default());
        app.insert_resource(PlaybackSync::default());

        let voice = Voice::new(
            Time::ONE,
            vec![Tile::spanning(Time::ZERO, Time::ONE, Intent::sample("kick")).unwrap()],
        )
        .unwrap();
        let mut scores = BTreeMap::new();
        scores.insert("main".to_string(), Score::from(voice));
        app.world_mut().resource_mut::<ActiveScores>().scores = scores;
        app.world_mut().resource_mut::<ActiveScores>().revision = 1;

        app.world_mut()
            .run_system_once(replace_scores_system)
            .unwrap();

        let sync = app.world().resource::<PlaybackSync>();
        assert_eq!(sync.last_applied_revision, 1);
    }

    #[test]
    fn cadence_plugin_registers_active_scores_resource() {
        let mut app = App::new();
        app.add_plugins(CadencePlugin);
        assert!(app.world().get_resource::<ActiveScores>().is_some());
        assert!(app.world().get_resource::<PlaybackSync>().is_some());
    }
}
