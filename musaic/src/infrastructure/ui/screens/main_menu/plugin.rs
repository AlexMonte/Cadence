use crate::application::editor::preferences::keymap::Action;
use bevy::prelude::*;
use bevy::state::condition::in_state;

use crate::application::command::{EditorCommand, EditorCommandBus};
use crate::application::session::MusaicProject;
use crate::infrastructure::app::AppState;

use super::launch::EditorLaunchIntent;
use super::ui::{EditorLaunchIntentHolder, MainMenuUiPlugin};
use super::unsaved_dialog::{UnsavedChangesPrompt, UnsavedDialogPlugin};

/// Boot and project launch lifecycle, with local prompt cancellation.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((MainMenuUiPlugin, UnsavedDialogPlugin))
            .add_systems(OnEnter(AppState::Boot), advance_from_boot)
            .add_systems(OnEnter(AppState::Editor), apply_editor_launch_intent)
            .add_systems(
                Update,
                (editor_save_shortcuts.run_if(crate::application::editor::interaction::keyboard_navigation::shortcuts_available), cancel_unsaved_prompt_on_escape)
                    .before(crate::infrastructure::app::MusaicSet::Commands)
                    .run_if(in_state(AppState::Editor)),
            )
            .add_systems(
                Update,
                apply_wasm_open_results.run_if(in_state(AppState::MainMenu)),
            );
    }
}

fn advance_from_boot(
    mut next: ResMut<NextState<AppState>>,
    #[cfg(not(target_arch = "wasm32"))] mut holder: ResMut<EditorLaunchIntentHolder>,
) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut args = std::env::args().skip(1);
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--first-loop" => {
                    holder.0 = Some(EditorLaunchIntent::FirstLoop);
                    next.set(AppState::Editor);
                    return;
                }
                "--example" => {
                    holder.0 = Some(EditorLaunchIntent::Example);
                    next.set(AppState::Editor);
                    return;
                }
                "--project" => {
                    if let Some(path) = args.next() {
                        holder.0 = Some(EditorLaunchIntent::OpenProject(path.into()));
                        next.set(AppState::Editor);
                        return;
                    }
                }
                _ => {}
            }
        }
    }
    next.set(AppState::MainMenu);
}

fn apply_editor_launch_intent(
    mut holder: ResMut<EditorLaunchIntentHolder>,
    mut bus: MessageWriter<EditorCommandBus>,
    mut guide: ResMut<crate::infrastructure::ui::first_loop::FirstLoopGuide>,
) {
    let Some(intent) = holder.take() else {
        return;
    };
    let command = match intent {
        EditorLaunchIntent::FirstLoop => {
            let starter = crate::application::session::first_loop();
            guide.request(&starter);
            bus.write(EditorCommandBus(EditorCommand::AdoptProject {
                project: starter.project,
                path: None,
            }));
            bus.write(EditorCommandBus(EditorCommand::EnterTimelineMode));
            bus.write(EditorCommandBus(EditorCommand::SelectNode {
                node: starter.pattern,
                mode: crate::application::editor::SelectionMode::Replace,
            }));
            return;
        }
        EditorLaunchIntent::Example => {
            let project = MusaicProject::reference_demo();
            let fast_value = project.document.graph.nodes_on_surface(project.document.root_surface).into_iter()
                .find(|(_, node)| matches!(&node.kind, crate::domain::document::DocumentNodeKind::Atom(atom) if atom.atom == crate::domain::document::AtomValue::Number(2)))
                .map(|(_, node)| node.id.clone());
            bus.write(EditorCommandBus(EditorCommand::AdoptProject {
                project,
                path: None,
            }));
            bus.write(EditorCommandBus(EditorCommand::EnterTimelineMode));
            if let Some(node) = fast_value {
                bus.write(EditorCommandBus(EditorCommand::SelectNode {
                    node,
                    mode: crate::application::editor::SelectionMode::Replace,
                }));
            }
            return;
        }
        EditorLaunchIntent::NewProject => EditorCommand::NewProject,
        EditorLaunchIntent::OpenProject(path) => EditorCommand::OpenProject { path },
        EditorLaunchIntent::LoadedProject(loaded) => {
            let path = loaded
                .metadata
                .file_path
                .as_ref()
                .map(std::path::PathBuf::from);
            EditorCommand::AdoptProject {
                project: *loaded,
                path,
            }
        }
    };
    bus.write(EditorCommandBus(command));
}

fn editor_save_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    preferences: Res<crate::application::editor::preferences::EditorPreferences>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if preferences.shortcut(Action::Save, &keyboard).is_some() {
        bus.write(EditorCommandBus(EditorCommand::SaveProject));
    }
}

fn cancel_unsaved_prompt_on_escape(
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut prompt: ResMut<UnsavedChangesPrompt>,
) {
    if keyboard.just_pressed(KeyCode::Escape) && prompt.active.is_some() {
        prompt.active = None;
        prompt.exit_after_save = false;
        keyboard.clear_just_pressed(KeyCode::Escape);
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
            holder.0 = Some(EditorLaunchIntent::LoadedProject(Box::new(project)));
            next.set(AppState::Editor);
        }
        Err(error) => bevy::log::error!("failed to open project: {error}"),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_wasm_open_results() {}
