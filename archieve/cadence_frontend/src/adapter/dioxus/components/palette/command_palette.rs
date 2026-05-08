use crate::application::editor::{
    EditorCommand, EditorShellState, PendingProjectAction, WorkspaceMode, picker_catalog,
};
use crate::adapter::dioxus::editor_service::dispatch_editor_command;
use crate::adapter::dioxus::components::palette::{clear_drag_state, close_picker};
use dioxus::html::{Key, Modifiers};
use dioxus::prelude::*;

#[derive(Clone)]
struct PaletteCommand {
    id: String,
    title: String,
    subtitle: String,
    shortcut: Option<&'static str>,
    action: EditorCommand,
}

impl PaletteCommand {
    fn matches_query(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let query = query.to_ascii_lowercase();
        self.title.to_ascii_lowercase().contains(&query)
            || self.subtitle.to_ascii_lowercase().contains(&query)
    }
}

pub(crate) fn open_command_palette(state: &mut EditorShellState) {
    state.command_palette_open = true;
    state.command_palette_query.clear();
    state.command_palette_selected = 0;
    close_picker(state);
    clear_drag_state(state);
}

pub(crate) fn close_command_palette(state: &mut EditorShellState) {
    state.command_palette_open = false;
    state.command_palette_query.clear();
    state.command_palette_selected = 0;
}

pub(crate) fn toggle_command_palette(state: &mut EditorShellState) {
    if state.command_palette_open {
        close_command_palette(state);
    } else {
        open_command_palette(state);
    }
}

pub(crate) fn command_palette_overlay(
    mut state: Signal<EditorShellState>,
    snapshot: &EditorShellState,
) -> Element {
    if !snapshot.command_palette_open {
        return rsx! {};
    }

    let commands = filtered_palette_commands(snapshot);
    let active_index = active_palette_index(snapshot, commands.len());
    let command_buttons = commands
        .iter()
        .enumerate()
        .map(|(index, command)| {
            let button_class = if Some(index) == active_index {
                "command-palette-item is-selected"
            } else {
                "command-palette-item"
            };
            let action = command.action.clone();
            let id = command.id.clone();
            let title = command.title.clone();
            let subtitle = command.subtitle.clone();
            let shortcut = command.shortcut;
            rsx! {
                button {
                    key: "{id}",
                    class: button_class,
                    r#type: "button",
                    onmouseenter: move |_| {
                        state.write().command_palette_selected = index;
                    },
                    onclick: move |_| {
                        let state = state;
                        let action = action.clone();
                        spawn(async move {
                            dispatch_editor_command(state, action).await;
                        });
                    },
                    span {
                        class: "command-palette-copy",
                        strong { class: "command-palette-title", "{title}" }
                        span { class: "command-palette-subtitle", "{subtitle}" }
                    }
                    if let Some(shortcut) = shortcut {
                        span { class: "command-palette-shortcut", "{shortcut}" }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    rsx! {
        div {
            class: "command-palette-backdrop",
            onclick: move |_| {
                close_command_palette(&mut state.write());
            },
        }
        section {
            id: "command-palette",
            class: "command-palette",
            "aria-label": "Command palette",

            header {
                class: "command-palette-head",
                span { "Command Palette" }
                span { class: "command-palette-hint", "Cmd/Ctrl+K" }
            }

            input {
                id: "command-palette-search",
                class: "command-palette-search",
                r#type: "search",
                autofocus: "true",
                value: snapshot.command_palette_query.clone(),
                placeholder: "Search tiles and actions...",
                "aria-label": "Search commands",
                oninput: move |event| {
                    let mut current = state.write();
                    current.command_palette_query = event.value();
                    current.command_palette_selected = 0;
                }
            }

            div {
                class: "command-palette-list",
                if command_buttons.is_empty() {
                    div {
                        class: "command-palette-empty",
                        "No commands match the current search."
                    }
                } else {
                    {command_buttons.into_iter()}
                }
            }
        }
    }
}

pub(crate) fn handle_editor_keydown(
    event: KeyboardEvent,
    mut state: Signal<EditorShellState>,
    snapshot: &EditorShellState,
) {
    let key = event.data().key();
    let modifiers = event.data().modifiers();

    if is_primary_shortcut(&key, modifiers, "k") {
        event.prevent_default();
        toggle_command_palette(&mut state.write());
        return;
    }

    if snapshot.command_palette_open {
        let commands = filtered_palette_commands(snapshot);
        match key {
            Key::Escape => {
                event.prevent_default();
                close_command_palette(&mut state.write());
            }
            Key::ArrowDown => {
                event.prevent_default();
                if !commands.is_empty() {
                    let len = commands.len();
                    let mut current = state.write();
                    current.command_palette_selected = (current.command_palette_selected + 1) % len;
                }
            }
            Key::ArrowUp => {
                event.prevent_default();
                if !commands.is_empty() {
                    let len = commands.len();
                    let mut current = state.write();
                    current.command_palette_selected =
                        (current.command_palette_selected + len - 1) % len;
                }
            }
            Key::Enter => {
                event.prevent_default();
                if let Some(index) = active_palette_index(snapshot, commands.len())
                    && let Some(command) = commands.get(index)
                {
                    let action = command.action.clone();
                    let state = state;
                    spawn(async move {
                        dispatch_editor_command(state, action).await;
                    });
                }
            }
            _ => {}
        }
        return;
    }

    if is_primary_shortcut(&key, modifiers, "s") {
        event.prevent_default();
        let force_dialog = modifiers.shift();
        let state = state;
        spawn(async move {
            dispatch_editor_command(state, EditorCommand::SaveProject { force_dialog }).await;
        });
        return;
    }

    if is_primary_shortcut(&key, modifiers, "z") {
        event.prevent_default();
        let state = state;
        if modifiers.shift() {
            spawn(async move {
                dispatch_editor_command(state, EditorCommand::Redo).await;
            });
        } else {
            spawn(async move {
                dispatch_editor_command(state, EditorCommand::Undo).await;
            });
        }
        return;
    }

    if has_primary_modifier(modifiers) && matches!(key, Key::Enter) {
        event.prevent_default();
        let state = state;
        spawn(async move {
            if state.read().runtime_status.playing {
                dispatch_editor_command(state, EditorCommand::StopRuntime).await;
            } else {
                dispatch_editor_command(state, EditorCommand::PlayRuntime).await;
            }
        });
        return;
    }

    if matches!(key, Key::Escape) {
        let current = state.write();
        if current.compile_open || current.inspector.open || current.picker_open {
            event.prevent_default();
            drop(current);
            let state = state;
            spawn(async move {
                dispatch_editor_command(state, EditorCommand::ClearTransientPanels).await;
            });
        }
    }
}

fn filtered_palette_commands(state: &EditorShellState) -> Vec<PaletteCommand> {
    let query = state.command_palette_query.trim();
    base_palette_commands(state)
        .into_iter()
        .filter(|command| command.matches_query(query))
        .collect()
}

fn base_palette_commands(state: &EditorShellState) -> Vec<PaletteCommand> {
    let mut commands = vec![
        PaletteCommand {
            id: "palette:runtime".to_string(),
            title: "Switch to Song workspace".to_string(),
            subtitle: "Open the main song canvas".to_string(),
            shortcut: None,
            action: EditorCommand::SwitchWorkspace {
                mode: WorkspaceMode::Runtime,
            },
        },
        PaletteCommand {
            id: "palette:init".to_string(),
            title: "Switch to Setup workspace".to_string(),
            subtitle: "Manage samples and patterns".to_string(),
            shortcut: None,
            action: EditorCommand::SwitchWorkspace {
                mode: WorkspaceMode::Init,
            },
        },
        PaletteCommand {
            id: "palette:code".to_string(),
            title: "Open activity preview".to_string(),
            subtitle: "Inspect issues, preview, and diagnostics".to_string(),
            shortcut: None,
            action: EditorCommand::OpenCompile,
        },
        PaletteCommand {
            id: "palette:console".to_string(),
            title: "Open diagnostics".to_string(),
            subtitle: "Jump to the diagnostics activity tab".to_string(),
            shortcut: None,
            action: EditorCommand::ToggleConsole,
        },
    ];

    if state.backend_available {
        commands.insert(
            0,
            PaletteCommand {
                id: "palette:picker".to_string(),
                title: "Open tile library".to_string(),
                subtitle: "Browse and add tiles from the library".to_string(),
                shortcut: None,
                action: EditorCommand::OpenPicker,
            },
        );
        commands.extend([
            PaletteCommand {
                id: "palette:new".to_string(),
                title: "New project".to_string(),
                subtitle: "Create a new Cadence project".to_string(),
                shortcut: None,
                action: EditorCommand::ProjectAction {
                    action: PendingProjectAction::NewProject,
                },
            },
            PaletteCommand {
                id: "palette:open".to_string(),
                title: "Open project".to_string(),
                subtitle: "Choose an existing project".to_string(),
                shortcut: None,
                action: EditorCommand::ProjectAction {
                    action: PendingProjectAction::OpenProject,
                },
            },
            PaletteCommand {
                id: "palette:save".to_string(),
                title: "Save project".to_string(),
                subtitle: "Write the current project to disk".to_string(),
                shortcut: Some("Cmd/Ctrl+S"),
                action: EditorCommand::SaveProject {
                    force_dialog: false,
                },
            },
            PaletteCommand {
                id: "palette:save-as".to_string(),
                title: "Save project as".to_string(),
                subtitle: "Choose a new save path".to_string(),
                shortcut: Some("Cmd/Ctrl+Shift+S"),
                action: EditorCommand::SaveProject { force_dialog: true },
            },
            PaletteCommand {
                id: "palette:export".to_string(),
                title: "Export song".to_string(),
                subtitle: "Render the current program to a file".to_string(),
                shortcut: None,
                action: EditorCommand::ExportSong,
            },
        ]);
    }

    if state.selected_node.is_some() {
        let inspector_subtitle = if state.selected_nodes.len() > 1 {
            "Review the selected group".to_string()
        } else {
            "Edit the selected tile".to_string()
        };
        commands.push(PaletteCommand {
            id: "palette:inspector".to_string(),
            title: "Open inspector".to_string(),
            subtitle: inspector_subtitle,
            shortcut: None,
            action: EditorCommand::OpenInspector,
        });
    }

    if state.backend_available && state.runtime_status.playing {
        commands.push(PaletteCommand {
            id: "palette:stop".to_string(),
            title: "Stop runtime".to_string(),
            subtitle: "Stop playback and clear the current run".to_string(),
            shortcut: Some("Cmd/Ctrl+Enter"),
            action: EditorCommand::StopRuntime,
        });
    } else if state.backend_available {
        commands.push(PaletteCommand {
            id: "palette:play".to_string(),
            title: "Play runtime".to_string(),
            subtitle: "Compile and run the current program".to_string(),
            shortcut: Some("Cmd/Ctrl+Enter"),
            action: EditorCommand::PlayRuntime,
        });
    }

    if state.history_status.can_undo {
        commands.push(PaletteCommand {
            id: "palette:undo".to_string(),
            title: "Undo".to_string(),
            subtitle: "Revert the last change".to_string(),
            shortcut: Some("Cmd/Ctrl+Z"),
            action: EditorCommand::Undo,
        });
    }

    if state.history_status.can_redo {
        commands.push(PaletteCommand {
            id: "palette:redo".to_string(),
            title: "Redo".to_string(),
            subtitle: "Reapply the last reverted change".to_string(),
            shortcut: Some("Cmd/Ctrl+Shift+Z"),
            action: EditorCommand::Redo,
        });
    }

    for trick in &state.init_stage.tricks {
        commands.push(PaletteCommand {
            id: format!("palette:trick:{}", trick.id),
            title: format!("Open pattern {}", trick.name),
            subtitle: "Switch into this reusable pattern graph".to_string(),
            shortcut: None,
            action: EditorCommand::SwitchWorkspace {
                mode: WorkspaceMode::Trick {
                    trick_id: trick.id.clone(),
                    name: trick.name.clone(),
                },
            },
        });
    }

    if state.backend_available && state.workspace_mode.picker_allowed() {
        commands.extend(picker_catalog(state).into_iter().map(|piece| {
            let subtitle = {
                let mut base = piece
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("{} | {}", piece.id, piece.category));
                if !piece.tags.is_empty() {
                    base.push_str(" | tags: ");
                    base.push_str(&piece.tags.join(", "));
                }
                base
            };
            PaletteCommand {
                id: format!("palette:piece:{}", piece.id),
                title: format!("Add {}", piece.label),
                subtitle,
                shortcut: None,
                action: EditorCommand::PlacePiece {
                    piece_id: piece.id.clone(),
                },
            }
        }));
    }

    commands
}

fn active_palette_index(state: &EditorShellState, len: usize) -> Option<usize> {
    if len == 0 {
        None
    } else {
        Some(state.command_palette_selected.min(len - 1))
    }
}

fn has_primary_modifier(modifiers: Modifiers) -> bool {
    modifiers.meta() || modifiers.ctrl()
}

fn is_primary_shortcut(key: &Key, modifiers: Modifiers, expected: &str) -> bool {
    has_primary_modifier(modifiers)
        && matches!(key, Key::Character(value) if value.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_query_matches_project_and_piece_actions() {
        let state = EditorShellState {
            backend_available: true,
            ..EditorShellState::default()
        };
        let commands = filtered_palette_commands(&EditorShellState {
            command_palette_query: "save".to_string(),
            ..state.clone()
        });

        assert!(
            commands
                .iter()
                .any(|command| command.title == "Save project")
        );
        assert!(!commands.is_empty());
    }

    #[test]
    fn palette_includes_piece_insert_commands_when_picker_is_available() {
        let state = EditorShellState {
            backend_available: true,
            catalog: vec![crate::adapter::PieceDef {
                id: "cadence.note".to_string(),
                label: "note".to_string(),
                category: "generator".to_string(),
                semantic_kind: "generator".to_string(),
                namespace: "cadence".to_string(),
                params: Vec::new(),
                output_side: Some("right".to_string()),
                description: Some("note".to_string()),
                tags: Vec::new(),
                bundle_input: None,
                temporal_kind: String::new(),
                fan_in: String::new(),
                fan_out: String::new(),
            }],
            ..EditorShellState::default()
        };
        let commands = base_palette_commands(&state);

        assert!(
            commands
                .iter()
                .any(|command| matches!(command.action, EditorCommand::PlacePiece { .. }))
        );
    }
}
