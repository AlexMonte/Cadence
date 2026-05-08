use bevy::prelude::*;

use crate::adapter::board_scene::projection::BoardSceneState;
use crate::application::board::{
    BoardFocus, BoardStore, InteractionStore, SelectionStore,
    runtime::{BoardCommandEvent, LastBoardError, apply_board_commands},
};
use crate::infrastructure::dto::{CadenceGraphTarget, EditorSyncSnapshotDto};
use crate::{SharedAppState, infrastructure::api};

#[derive(Resource)]
pub struct CadenceBackend {
    pub state: SharedAppState,
}

impl Default for CadenceBackend {
    fn default() -> Self {
        Self {
            state: SharedAppState::new(),
        }
    }
}

#[derive(Debug, Resource, Clone)]
pub struct EditorSnapshot {
    pub sync: EditorSyncSnapshotDto,
}

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CadenceBackend::default())
            .insert_resource(InteractionStore::default())
            .insert_resource(LastBoardError::default())
            .add_message::<BoardCommandEvent>()
            .add_systems(Startup, bootstrap_editor_snapshot)
            .add_systems(Update, (apply_board_commands, refresh_board_scene).chain());
    }
}

fn bootstrap_editor_snapshot(mut commands: Commands, backend: Res<CadenceBackend>) {
    api::project_bootstrap(&backend.state).expect("bootstrap default cadence project");
    let sync = api::editor_sync(&backend.state, CadenceGraphTarget::Runtime, true)
        .expect("build initial editor snapshot");
    let board_store = BoardStore::from_editor_sync(&sync);
    let board_focus = BoardFocus::new(board_store.root_board.clone());
    let selection_store = SelectionStore::new(board_store.root_board.clone());
    let interaction_store = InteractionStore::default();
    let board_scene =
        BoardSceneState::from_store(&board_store, &board_focus, &selection_store, &interaction_store)
            .expect("project initial board scene");

    commands.insert_resource(EditorSnapshot { sync });
    commands.insert_resource(board_store);
    commands.insert_resource(board_focus);
    commands.insert_resource(selection_store);
    commands.insert_resource(interaction_store);
    commands.insert_resource(board_scene);
}

fn refresh_board_scene(
    board_store: Res<BoardStore>,
    board_focus: Res<BoardFocus>,
    selection_store: Res<SelectionStore>,
    interactions: Res<InteractionStore>,
    mut commands: Commands,
) {
    if !board_store.is_changed()
        && !board_focus.is_changed()
        && !selection_store.is_changed()
        && !interactions.is_changed()
    {
        return;
    }

    if let Some(scene) =
        BoardSceneState::from_store(&board_store, &board_focus, &selection_store, &interactions)
    {
        commands.insert_resource(scene);
    }
}
