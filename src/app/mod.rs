use bevy::prelude::*;

use crate::{
    board::BoardPlugin, diagnostics::DiagnosticsPlugin, document::DocumentPlugin,
    editor::EditorPlugin, lowering::LoweringPlugin, palette::PalettePlugin,
    persistence::PersistencePlugin, runtime::RuntimePlugin, scene_sync::SceneSyncPlugin,
    transport::TransportPlugin,
};

pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Cadence".into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(CadencePlugin)
        .run();
}

pub struct CadencePlugin;

impl Plugin for CadencePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<ProjectState>()
            .init_state::<TransportMode>()
            .configure_sets(
                Update,
                (
                    CadenceSet::Input,
                    CadenceSet::Commands,
                    CadenceSet::DocumentMutation,
                    CadenceSet::Compile,
                    CadenceSet::Lower,
                    CadenceSet::Runtime,
                    CadenceSet::SceneSync,
                    CadenceSet::RenderUi,
                )
                    .chain(),
            )
            .add_plugins((
                DocumentPlugin,
                EditorPlugin,
                BoardPlugin,
                PalettePlugin,
                DiagnosticsPlugin,
                LoweringPlugin,
                RuntimePlugin,
                TransportPlugin,
                SceneSyncPlugin,
                PersistencePlugin,
            ));
    }
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ProjectState {
    #[default]
    NoProject,
    LoadingProject,
    ProjectOpen,
}

#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TransportMode {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum CadenceSet {
    Input,
    Commands,
    DocumentMutation,
    Compile,
    Lower,
    Runtime,
    SceneSync,
    RenderUi,
}
