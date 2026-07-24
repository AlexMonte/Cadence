use bevy::prelude::*;
use bevy::state::condition::in_state;

use crate::application::command::{EditorCommand, EditorCommandBus};
use crate::application::session::MusaicProject;
use crate::infrastructure::app::AppState;

use super::launch::EditorLaunchIntent;
use super::ui::{EditorLaunchIntentHolder, MainMenuUiPlugin};
use super::unsaved_dialog::{ExitTarget, UnsavedChangesPrompt, UnsavedDialogPlugin, request_exit};

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((MainMenuUiPlugin, UnsavedDialogPlugin))
            .add_systems(OnEnter(AppState::Boot), advance_from_boot)
            .add_systems(OnEnter(AppState::Editor), apply_editor_launch_intent)
            .add_systems(
                Update,
                (editor_save_shortcuts, return_to_main_menu_on_escape)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                apply_wasm_open_results.run_if(in_state(AppState::MainMenu)),
            );
    }
}

fn advance_from_boot(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::MainMenu);
}

fn apply_editor_launch_intent(
    mut holder: ResMut<EditorLaunchIntentHolder>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Some(intent) = holder.take() else {
        return;
    };
    let command = match intent {
        EditorLaunchIntent::NewProject => EditorCommand::NewProject,
        EditorLaunchIntent::OpenProject(path) => EditorCommand::OpenProject { path },
        EditorLaunchIntent::LoadedProject(loaded) => {
            let path = loaded
                .metadata
                .file_path
                .as_ref()
                .map(std::path::PathBuf::from);
            EditorCommand::AdoptProject {
                project: loaded,
                path,
            }
        }
    };
    bus.write(EditorCommandBus(command));
}

fn editor_save_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let save = keyboard.just_pressed(KeyCode::KeyS)
        && (keyboard.pressed(KeyCode::SuperLeft)
            || keyboard.pressed(KeyCode::SuperRight)
            || keyboard.pressed(KeyCode::ControlLeft)
            || keyboard.pressed(KeyCode::ControlRight));
    if !save {
        return;
    }
    bus.write(EditorCommandBus(EditorCommand::SaveProject));
}

fn return_to_main_menu_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    project: Res<MusaicProject>,
    session: Res<crate::application::editor::EditorSession>,
    mut prompt: ResMut<UnsavedChangesPrompt>,
    mut next: ResMut<NextState<AppState>>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    // Placement / connection cancel owns Esc first (command bus).
    if session.is_placing_from_drawer() || session.is_armed() || session.is_connecting() {
        return;
    }
    if request_exit(&project, &mut prompt, ExitTarget::MainMenu) {
        next.set(AppState::MainMenu);
    }
}

#[cfg(target_arch = "wasm32")]
fn apply_wasm_open_results(
    mut io: ResMut<crate::adapter::persistence::wasm_io::WasmProjectIo>,
    mut holder: ResMut<EditorLaunchIntentHolder>,
    mut next: ResMut<NextState<AppState>>,
) {
    let Some(result) = io.pending_opens.pop() else {
        return;
    };
    match result {
        Ok(project) => {
            holder.0 = Some(EditorLaunchIntent::LoadedProject(project));
            next.set(AppState::Editor);
        }
        Err(error) => bevy::log::error!("failed to open project: {error}"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_wasm_open_results() {}
