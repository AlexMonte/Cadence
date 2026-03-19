use super::*;
use dioxus::html::{Key, Modifiers};

#[derive(Clone)]
enum PaletteAction {
    OpenPicker,
    PlacePiece { piece_id: String },
    SwitchWorkspace { mode: WorkspaceMode },
    OpenCompile,
    OpenInspector,
    ProjectAction { action: PendingProjectAction },
    SaveProject { force_dialog: bool },
    ExportSong,
    PlayRuntime,
    StopRuntime,
    Undo,
    Redo,
    ToggleConsole,
}

#[derive(Clone)]
struct PaletteCommand {
    id: String,
    title: String,
    subtitle: String,
    shortcut: Option<&'static str>,
    action: PaletteAction,
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

pub(super) fn open_command_palette(state: &mut EditorShellState) {
    state.command_palette_open = true;
    state.command_palette_query.clear();
    state.command_palette_selected = 0;
    close_picker(state);
    clear_drag_state(state);
}

pub(super) fn close_command_palette(state: &mut EditorShellState) {
    state.command_palette_open = false;
    state.command_palette_query.clear();
    state.command_palette_selected = 0;
}

pub(super) fn toggle_command_palette(state: &mut EditorShellState) {
    if state.command_palette_open {
        close_command_palette(state);
    } else {
        open_command_palette(state);
    }
}

pub(super) fn command_palette_overlay(
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
                            apply_palette_action(state, action).await;
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

pub(super) fn handle_editor_keydown(
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
                        apply_palette_action(state, action).await;
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
            let _ = save_project(state, force_dialog).await;
        });
        return;
    }

    if is_primary_shortcut(&key, modifiers, "z") {
        event.prevent_default();
        let state = state;
        if modifiers.shift() {
            spawn(async move {
                redo_history(state).await;
            });
        } else {
            spawn(async move {
                undo_history(state).await;
            });
        }
        return;
    }

    if has_primary_modifier(modifiers) && matches!(key, Key::Enter) {
        event.prevent_default();
        let state = state;
        spawn(async move {
            if state.read().runtime_status.playing {
                stop_runtime(state).await;
            } else {
                play_runtime(state).await;
            }
        });
        return;
    }

    if matches!(key, Key::Escape) {
        let mut current = state.write();
        if current.compile_open || current.inspector_open || current.picker_open {
            event.prevent_default();
            current.compile_open = false;
            current.inspector_open = false;
            close_picker(&mut current);
            clear_drag_state(&mut current);
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
            title: "Switch to Runtime workspace".to_string(),
            subtitle: "Open the main runtime graph".to_string(),
            shortcut: None,
            action: PaletteAction::SwitchWorkspace {
                mode: WorkspaceMode::Runtime,
            },
        },
        PaletteCommand {
            id: "palette:init".to_string(),
            title: "Switch to Init workspace".to_string(),
            subtitle: "Open samples and tricks".to_string(),
            shortcut: None,
            action: PaletteAction::SwitchWorkspace {
                mode: WorkspaceMode::Init,
            },
        },
        PaletteCommand {
            id: "palette:code".to_string(),
            title: "Open code preview".to_string(),
            subtitle: "Inspect compile output and diagnostics".to_string(),
            shortcut: None,
            action: PaletteAction::OpenCompile,
        },
        PaletteCommand {
            id: "palette:console".to_string(),
            title: "Toggle console".to_string(),
            subtitle: "Show or hide runtime diagnostics".to_string(),
            shortcut: None,
            action: PaletteAction::ToggleConsole,
        },
    ];

    if state.tauri_available {
        commands.insert(
            0,
            PaletteCommand {
                id: "palette:picker".to_string(),
                title: "Open tile picker".to_string(),
                subtitle: "Browse pieces with the current picker UI".to_string(),
                shortcut: None,
                action: PaletteAction::OpenPicker,
            },
        );
        commands.extend([
            PaletteCommand {
                id: "palette:new".to_string(),
                title: "New project".to_string(),
                subtitle: "Create a new Cadence project".to_string(),
                shortcut: None,
                action: PaletteAction::ProjectAction {
                    action: PendingProjectAction::NewProject,
                },
            },
            PaletteCommand {
                id: "palette:open".to_string(),
                title: "Open project".to_string(),
                subtitle: "Choose an existing project".to_string(),
                shortcut: None,
                action: PaletteAction::ProjectAction {
                    action: PendingProjectAction::OpenProject,
                },
            },
            PaletteCommand {
                id: "palette:save".to_string(),
                title: "Save project".to_string(),
                subtitle: "Write the current project to disk".to_string(),
                shortcut: Some("Cmd/Ctrl+S"),
                action: PaletteAction::SaveProject {
                    force_dialog: false,
                },
            },
            PaletteCommand {
                id: "palette:save-as".to_string(),
                title: "Save project as".to_string(),
                subtitle: "Choose a new save path".to_string(),
                shortcut: Some("Cmd/Ctrl+Shift+S"),
                action: PaletteAction::SaveProject { force_dialog: true },
            },
            PaletteCommand {
                id: "palette:export".to_string(),
                title: "Export song".to_string(),
                subtitle: "Render the current program to a file".to_string(),
                shortcut: None,
                action: PaletteAction::ExportSong,
            },
        ]);
    }

    if state.selected_node.is_some() {
        commands.push(PaletteCommand {
            id: "palette:inspector".to_string(),
            title: "Open inspector".to_string(),
            subtitle: "Edit the selected tile".to_string(),
            shortcut: None,
            action: PaletteAction::OpenInspector,
        });
    }

    if state.tauri_available && state.runtime_status.playing {
        commands.push(PaletteCommand {
            id: "palette:stop".to_string(),
            title: "Stop runtime".to_string(),
            subtitle: "Stop playback and clear the current run".to_string(),
            shortcut: Some("Cmd/Ctrl+Enter"),
            action: PaletteAction::StopRuntime,
        });
    } else if state.tauri_available {
        commands.push(PaletteCommand {
            id: "palette:play".to_string(),
            title: "Play runtime".to_string(),
            subtitle: "Compile and run the current program".to_string(),
            shortcut: Some("Cmd/Ctrl+Enter"),
            action: PaletteAction::PlayRuntime,
        });
    }

    if state.history_status.can_undo {
        commands.push(PaletteCommand {
            id: "palette:undo".to_string(),
            title: "Undo".to_string(),
            subtitle: "Revert the last change".to_string(),
            shortcut: Some("Cmd/Ctrl+Z"),
            action: PaletteAction::Undo,
        });
    }

    if state.history_status.can_redo {
        commands.push(PaletteCommand {
            id: "palette:redo".to_string(),
            title: "Redo".to_string(),
            subtitle: "Reapply the last reverted change".to_string(),
            shortcut: Some("Cmd/Ctrl+Shift+Z"),
            action: PaletteAction::Redo,
        });
    }

    for trick in &state.init_stage.tricks {
        commands.push(PaletteCommand {
            id: format!("palette:trick:{}", trick.id),
            title: format!("Open trick {}", trick.name),
            subtitle: "Switch into this reusable subgraph".to_string(),
            shortcut: None,
            action: PaletteAction::SwitchWorkspace {
                mode: WorkspaceMode::Trick {
                    trick_id: trick.id.clone(),
                    name: trick.name.clone(),
                },
            },
        });
    }

    if state.tauri_available && state.workspace_mode.picker_allowed() {
        commands.extend(state.catalog.iter().map(|piece| {
            PaletteCommand {
                id: format!("palette:piece:{}", piece.id),
                title: format!("Insert {}", piece.label),
                subtitle: piece
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("{} | {}", piece.id, piece.category)),
                shortcut: None,
                action: PaletteAction::PlacePiece {
                    piece_id: piece.id.clone(),
                },
            }
        }));
    }

    commands
}

async fn apply_palette_action(mut state: Signal<EditorShellState>, action: PaletteAction) {
    close_command_palette(&mut state.write());
    match action {
        PaletteAction::OpenPicker => {
            toggle_picker_panel(&mut state.write());
        }
        PaletteAction::PlacePiece { piece_id } => {
            place_piece(state, piece_id, None).await;
        }
        PaletteAction::SwitchWorkspace { mode } => {
            switch_workspace_mode(state, mode).await;
        }
        PaletteAction::OpenCompile => {
            state.write().compile_open = true;
        }
        PaletteAction::OpenInspector => {
            if state.read().selected_node.is_some() {
                state.write().inspector_open = true;
            } else {
                state.write().status_message = Some("Select a tile to inspect.".to_string());
            }
        }
        PaletteAction::ProjectAction { action } => {
            request_project_action(state, action).await;
        }
        PaletteAction::SaveProject { force_dialog } => {
            let _ = save_project(state, force_dialog).await;
        }
        PaletteAction::ExportSong => {
            export_song(state).await;
        }
        PaletteAction::PlayRuntime => {
            play_runtime(state).await;
        }
        PaletteAction::StopRuntime => {
            stop_runtime(state).await;
        }
        PaletteAction::Undo => {
            undo_history(state).await;
        }
        PaletteAction::Redo => {
            redo_history(state).await;
        }
        PaletteAction::ToggleConsole => {
            let next = !state.read().diagnostics.mini_console_visible;
            set_mini_console_visible(state, next).await;
        }
    }
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
            tauri_available: true,
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
            tauri_available: true,
            catalog: vec![PieceDef {
                id: "strudel.note".to_string(),
                label: "note".to_string(),
                category: "generator".to_string(),
                semantic_kind: "generator".to_string(),
                namespace: "strudel".to_string(),
                params: Vec::new(),
                output_type: Some(PortType::Plain("pattern".to_string())),
                output_side: Some("right".to_string()),
                description: Some("note".to_string()),
                tags: Vec::new(),
            }],
            ..EditorShellState::default()
        };
        let commands = base_palette_commands(&state);

        assert!(
            commands
                .iter()
                .any(|command| matches!(command.action, PaletteAction::PlacePiece { .. }))
        );
    }
}
