use crate::adapter::backend::runtime;
use crate::application::editor::{EditorCommand, PendingProjectAction};

fn map_menu_action(action: &str) -> Option<EditorCommand> {
    match action {
        "file.new" => Some(EditorCommand::ProjectAction {
            action: PendingProjectAction::NewProject,
        }),
        "file.open" => Some(EditorCommand::ProjectAction {
            action: PendingProjectAction::OpenProject,
        }),
        "file.quit" => Some(EditorCommand::ProjectAction {
            action: PendingProjectAction::QuitApp,
        }),
        "file.save" => Some(EditorCommand::SaveProject {
            force_dialog: false,
        }),
        "file.save_as" => Some(EditorCommand::SaveProject { force_dialog: true }),
        "file.export_song" => Some(EditorCommand::ExportSong),
        "edit.undo" => Some(EditorCommand::Undo),
        "edit.redo" => Some(EditorCommand::Redo),
        "window.close_requested" => Some(EditorCommand::ProjectAction {
            action: PendingProjectAction::CloseWindow,
        }),
        "view.toggle_mini_console" => Some(EditorCommand::ToggleConsole),
        "view.toggle_dev_inspector" => Some(EditorCommand::ToggleDevInspector),
        _ => None,
    }
}

pub fn take_menu_commands() -> Vec<EditorCommand> {
    runtime::take_menu_actions()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|action| map_menu_action(action.as_str()))
        .collect()
}
