use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
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
        .add_plugins(WorldInspectorPlugin::new())
        .add_plugins(LoadUpPlugin::<GameState>::default())
        .require_resource_for_state::<GameAssets>(GameState::Game, RetentionPolicy::Keep)
        .init_state::<GameState>()
        .add_systems(Startup, |mut next: ResMut<NextState<GameState>>| {
            info!("Requesting Game state — inspect DependencyRegistry and PreparedResources in the world inspector.");
            next.set(GameState::Game);
        })
        .run();
}
