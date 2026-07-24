//! Global editor keyboard shortcuts (delete, undo/redo, transport, cancel).

use bevy::prelude::*;

use crate::application::command::{EditorCommand, EditorCommandBus};
use crate::application::editor::EditorSession;

pub fn dispatch_delete_selection(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if keyboard.just_pressed(KeyCode::Delete) || keyboard.just_pressed(KeyCode::Backspace) {
        bus.write(EditorCommandBus(EditorCommand::DeleteSelection));
    }
}

pub fn dispatch_undo_redo(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let ctrl = keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight)
        || keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight);
    if !ctrl {
        return;
    }
    if keyboard.just_pressed(KeyCode::KeyZ) {
        bus.write(EditorCommandBus(EditorCommand::Undo));
    } else if keyboard.just_pressed(KeyCode::KeyY) {
        bus.write(EditorCommandBus(EditorCommand::Redo));
    }
}

pub fn dispatch_transport_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        bus.write(EditorCommandBus(EditorCommand::TransportToggle));
    }
    if keyboard.just_pressed(KeyCode::Home) {
        bus.write(EditorCommandBus(EditorCommand::TransportSeek {
            position_cycles: 0.0,
        }));
    }
}

/// Esc cancels placement or connection first; main-menu exit is handled separately
/// only when neither modal is active.
pub fn dispatch_cancel_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    session: Res<EditorSession>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if !keyboard.just_pressed(KeyCode::Escape) {
        return;
    }
    if session.is_placing_from_drawer() || session.is_armed() {
        bus.write(EditorCommandBus(EditorCommand::CancelPlacement));
        return;
    }
    if session.is_connecting() {
        bus.write(EditorCommandBus(EditorCommand::AbortConnection));
    }
}
