//! Global editor keyboard shortcuts (delete, undo/redo, transport, cancel).

use crate::application::editor::preferences::{EditorPreferences, keymap::Action};
use bevy::prelude::*;

use crate::application::command::{EditorCommand, EditorCommandBus};
use crate::application::editor::{
    EditorAttention, EditorSession, FocusTarget, SelectionMode, SelectionState,
};

pub fn dispatch_delete_selection(
    keyboard: Res<ButtonInput<KeyCode>>,
    preferences: Res<EditorPreferences>,
    attention: Res<EditorAttention>,
    selection: Res<SelectionState>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if preferences.shortcut(Action::Delete, &keyboard).is_some() {
        select_cursor_if_needed(&attention, &selection, &mut bus);
        bus.write(EditorCommandBus(EditorCommand::DeleteSelection));
    }
}

pub fn dispatch_undo_redo(
    keyboard: Res<ButtonInput<KeyCode>>,
    preferences: Res<EditorPreferences>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if preferences.shortcut(Action::Undo, &keyboard).is_some() {
        bus.write(EditorCommandBus(EditorCommand::Undo));
    } else if preferences.shortcut(Action::Redo, &keyboard).is_some() {
        bus.write(EditorCommandBus(EditorCommand::Redo));
    }
}

pub fn dispatch_transport_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    preferences: Res<EditorPreferences>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if preferences.shortcut(Action::PlayPause, &keyboard).is_some() {
        bus.write(EditorCommandBus(EditorCommand::TransportToggle));
    }
    if preferences.shortcut(Action::Stop, &keyboard).is_some() {
        bus.write(EditorCommandBus(EditorCommand::TransportStop));
    }
    if preferences.shortcut(Action::Restart, &keyboard).is_some() {
        bus.write(EditorCommandBus(EditorCommand::TransportSeek {
            position_cycles: 0.0,
        }));
    }
}

/// Escape cancels the current tool; closing a project is an explicit menu action.
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

/// Standard editing shortcuts use the same transactions as the visible toolbar.
pub fn dispatch_edit_keys(
    keyboard: Res<ButtonInput<KeyCode>>,
    preferences: Res<EditorPreferences>,
    attention: Res<EditorAttention>,
    selection: Res<SelectionState>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    use crate::application::command::editing::TileEdit;
    for (key, action) in [
        (Action::Copy, TileEdit::Copy),
        (Action::Paste, TileEdit::Paste),
        (Action::Duplicate, TileEdit::Duplicate),
        (
            Action::Group,
            TileEdit::Group(crate::domain::document::ContainerKind::Sequence),
        ),
    ] {
        if preferences.shortcut(key, &keyboard).is_some() {
            if !matches!(action, TileEdit::Paste) {
                select_cursor_if_needed(&attention, &selection, &mut bus);
            }
            bus.write(EditorCommandBus(EditorCommand::EditTiles(action)));
        }
    }
}

fn select_cursor_if_needed(
    attention: &EditorAttention,
    selection: &SelectionState,
    bus: &mut MessageWriter<EditorCommandBus>,
) {
    if !selection.nodes.is_empty() {
        return;
    }
    if let FocusTarget::Tile { node }
    | FocusTarget::Atom { node }
    | FocusTarget::Port { node, .. } = &attention.focus
    {
        bus.write(EditorCommandBus(EditorCommand::SelectNode {
            node: node.clone(),
            mode: SelectionMode::Replace,
        }));
    }
}
