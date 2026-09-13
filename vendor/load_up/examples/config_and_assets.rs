use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy_common_assets::ron::RonAssetPlugin;
use load_up::prelude::*;
use serde::Deserialize;

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

#[derive(Debug, Deserialize, Asset, TypePath, Clone)]
struct GameConfig {
    title: String,
}

#[derive(Resource, Asset, LoadAssetResource, TypePath, Clone)]
struct LevelAssets {
    #[dependency(path = "config/game.level.ron")]
    config: Handle<GameConfig>,
    #[dependency(init = placeholder_layout)]
    atlas: Handle<TextureAtlasLayout>,
    #[dependency(path = "textures/streaming.png", streaming)]
    optional_texture: Handle<Image>,
}

fn placeholder_layout() -> TextureAtlasLayout {
    TextureAtlasLayout::from_grid(UVec2 { x: 16, y: 16 }, 2, 2, None, None)
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RonAssetPlugin::<GameConfig>::new(&["level.ron"]))
        .add_plugins(LoadUpPlugin::<GameState>::default())
        .require_resource_for_state::<LevelAssets>(GameState::Game, RetentionPolicy::Keep)
        .init_state::<GameState>()
        .add_systems(Startup, |mut next: ResMut<NextState<GameState>>| {
            info!("Requesting Game state with mixed RON config and image dependencies...");
            next.set(GameState::Game);
        })
        .add_systems(
            Update,
            |state: Res<State<GameState>>,
             assets: Option<Res<LevelAssets>>,
             configs: Res<Assets<GameConfig>>,
             mut logged: Local<bool>| {
                if *logged {
                    return;
                }
                if *state.get() != GameState::Game {
                    return;
                }
                let Some(assets) = assets else {
                    return;
                };
                let Some(config) = configs.get(&assets.config) else {
                    return;
                };
                *logged = true;
                info!(
                    "LevelAssets ready — loaded RON config title: {:?}",
                    config.title
                );
            },
        )
        .run();
}
