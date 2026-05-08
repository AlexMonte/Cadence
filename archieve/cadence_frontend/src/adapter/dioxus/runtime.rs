use dioxus::prelude::*;

use crate::adapter::take_menu_commands;
use crate::adapter::dioxus::editor_service::dispatch_editor_command;
use crate::application::editor::EditorShellState;

pub async fn process_menu_actions(state: Signal<EditorShellState>) {
    for command in take_menu_commands() {
        dispatch_editor_command(state, command).await;
    }
}
