use bevy::prelude::*;

use cadence::bevy::CadenceSet;

use crate::adapter::PersistencePlugin;
use crate::adapter::load_up::LoadUpMusaicPlugin;
use crate::application::{EditorPlugin, PlaybackPlugin};
use bevy_feathers::FeathersPlugins;

use crate::infrastructure::ui::MusaicUiPlugin;
use crate::infrastructure::ui::theme::insert_musaic_ui_theme;

mod window;

pub fn run() {
    build_app().run();
}

/// Construct the native editor, allowing examples to add diagnostics before run.
pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Musaic".into(),
                    resolution: (1280, 800).into(),
                    resize_constraints: bevy::window::WindowResizeConstraints {
                        min_width: 1024.0,
                        min_height: 680.0,
                        ..default()
                    },
                    ..default()
                }),
                ..default()
            }),
    )
    .add_plugins(MusaicPlugin);
    app
}

/// Shared by the native app and headless editor integration tests.
/// Commands, compilation, accepted playback and preview must describe one revision.
pub(crate) fn configure_pipeline_schedule(app: &mut App) {
    app.configure_sets(
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
    .configure_sets(
        Update,
        (
            CadenceSet::ReplaceScores.in_set(MusaicSet::Runtime),
            CadenceSet::Tick.in_set(MusaicSet::Runtime),
        ),
    );
}

pub struct MusaicPlugin;

impl Plugin for MusaicPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_state::<TransportMode>()
            .add_plugins(FeathersPlugins);
        insert_musaic_ui_theme(app);
        app.add_plugins(window::AppWindowPlugin).add_plugins((
            MeshPickingPlugin,  // external: 3D picking
            LoadUpMusaicPlugin, // assets: disk → handles
            EditorPlugin,       // editor: state, input, commands, mutation, projection
            crate::adapter::preferences::plugin,
            PlaybackPlugin,    // playback: compile → lower → audio
            MusaicUiPlugin,    // UI: editor shell + main menu rendering
            PersistencePlugin, // adapter: project file IO
        ));
        configure_pipeline_schedule(app);
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
    Paused,
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
