use bevy::prelude::*;

use tessera::bevy::TesseraPlugin;

use crate::adapter::PersistencePlugin;
use crate::adapter::load_up::LoadUpMusaicPlugin;
use crate::application::{EditorPlugin, PlaybackPlugin};
use bevy_feathers::{FeathersPlugins, dark_theme::create_dark_theme, theme::UiTheme};

use crate::infrastructure::{loading::LoadingUiPlugin, ui::MusaicUiPlugin};

mod window;

pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Musaic".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(MusaicPlugin)
        .run();
}

pub struct MusaicPlugin;

impl Plugin for MusaicPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_state::<TransportMode>()
            .add_plugins(FeathersPlugins)
            .insert_resource(UiTheme(create_dark_theme()))
            .add_plugins((window::AppWindowPlugin, LoadingUiPlugin))
            .configure_sets(
                Update,
                (
                    MusaicSet::Input,
                    MusaicSet::Commands,
                    MusaicSet::DocumentMutation,
                    MusaicSet::Compile,
                    MusaicSet::Lower,
                    MusaicSet::Runtime,
                    MusaicSet::SceneSync,
                    MusaicSet::RenderUi,
                )
                    .chain(),
            )
            .add_plugins((
                MeshPickingPlugin,  // external: 3D picking
                TesseraPlugin,      // external: tile program authority
                LoadUpMusaicPlugin, // assets: disk → handles
                EditorPlugin,       // editor: state, input, commands, mutation, projection
                PlaybackPlugin,     // playback: compile → lower → audio
                MusaicUiPlugin,     // UI: editor shell + main menu rendering
                PersistencePlugin,  // adapter: project file IO
            ));
        #[cfg(target_arch = "wasm32")]
        app.add_plugins(crate::adapter::persistence::wasm_io::WasmIoPlugin);
    }
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Boot,
    MainMenu,
    Loading,
    Editor,
}

impl load_up::state::LoadingStateFor for AppState {
    fn loading_state() -> Self {
        AppState::Loading
    }
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TransportMode {
    #[default]
    Stopped,
    Playing,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MusaicSet {
    Input,
    Commands,
    DocumentMutation,
    Compile,
    Lower,
    Runtime,
    SceneSync,
    RenderUi,
}
