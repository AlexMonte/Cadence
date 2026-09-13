use bevy::prelude::*;
use iyes_progress::prelude::*;
use load_up::prelude::*;

#[derive(States, Clone, Eq, PartialEq, Hash, Debug, Default)]
enum GameState {
    #[default]
    Loading,
    Game,
}

impl LoadingStateFor for GameState {
    fn loading_state() -> Self {
        GameState::Loading
    }
}

#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
struct GameAssets {
    #[dependency(init = placeholder_layout)]
    atlas: Handle<TextureAtlasLayout>,
}

fn placeholder_layout() -> TextureAtlasLayout {
    TextureAtlasLayout::from_grid(UVec2 { x: 32, y: 32 }, 4, 4, None, None)
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(ProgressPlugin::<GameState>::new())
        .add_plugins(LoadUpPlugin::<GameState>::default())
        .add_plugins(LoadUpProgressPlugin::<GameState>::default())
        .require_resource_for_state::<GameAssets>(GameState::Game, RetentionPolicy::Keep)
        .init_state::<GameState>()
        .add_systems(Startup, |mut next: ResMut<NextState<GameState>>| {
            info!("Requesting Game state...");
            next.set(GameState::Game);
        })
        .add_systems(
            Update,
            (
                log_progress,
                log_game_ready.run_if(in_state(GameState::Game)),
            ),
        )
        .run();
}

fn log_progress(state: Res<State<GameState>>, tracker: Res<ProgressTracker<GameState>>) {
    if *state.get() != GameState::Loading {
        return;
    }

    let progress = tracker.get_global_progress();
    if progress.total > 0 {
        info!(
            "Loading progress: {}/{} ({:.0}%)",
            progress.done,
            progress.total,
            f32::from(progress) * 100.0
        );
    }
}

fn log_game_ready(assets: Option<Res<GameAssets>>) {
    if assets.is_some() {
        info!("GameAssets ready in Game state.");
    }
}
