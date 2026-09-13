use bevy::prelude::*;
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
        .add_plugins(LoadUpPlugin::<GameState>::default())
        .require_resource_for_state::<GameAssets>(GameState::Game, RetentionPolicy::Keep)
        .init_state::<GameState>()
        .add_systems(Startup, |mut next: ResMut<NextState<GameState>>| {
            info!("Requesting Game state...");
            next.set(GameState::Game);
        })
        .add_systems(
            Update,
            |state: Res<State<GameState>>, assets: Option<Res<GameAssets>>| {
                if *state.get() == GameState::Game && assets.is_some() {
                    info!("GameAssets ready in Game state.");
                }
            },
        )
        .run();
}
