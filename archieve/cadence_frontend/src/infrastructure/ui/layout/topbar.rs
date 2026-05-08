use dioxus::prelude::*;

use crate::adapter::backend;
use crate::adapter::dioxus::components::palette::toggle_command_palette;
use crate::adapter::dioxus::components::palette::toggle_picker_panel;
use crate::adapter::dioxus::editor_service::{
    play_runtime, redo_history, rename_project, request_project_action, save_project,
    stop_runtime, switch_workspace_mode, undo_history,
};
use crate::adapter::dioxus::theme as shell_theme;
use crate::application::editor::*;

#[component]
pub(crate) fn TopbarRegion(state: Signal<EditorShellState>, snapshot: EditorShellState) -> Element {
    let top_mode_label = snapshot.workspace_mode.label();
    let workspace_runtime_active = matches!(snapshot.workspace_mode, WorkspaceMode::Runtime);
    let workspace_init_active = matches!(snapshot.workspace_mode, WorkspaceMode::Init);
    let show_back = matches!(snapshot.workspace_mode, WorkspaceMode::Trick { .. });
    let mode_chip_title = match &snapshot.workspace_mode {
        WorkspaceMode::Runtime => "Song workspace",
        WorkspaceMode::Init => "Setup workspace",
        WorkspaceMode::Trick { name, .. } => name.as_str(),
    };
    let picker_available = snapshot.backend_available && snapshot.workspace_mode.picker_allowed();
    let activity_open = snapshot.compile_open;
    let can_play = snapshot.backend_available && snapshot.project_preview.can_play;
    let can_stop = snapshot.backend_available
        && (snapshot.runtime_status.playing
            || !matches!(
                snapshot.runtime_status.program_state,
                backend::RuntimeProgramStateDto::None
            ));
    let can_save = snapshot.backend_available;
    let can_undo = snapshot.backend_available && snapshot.history_status.can_undo;
    let can_redo = snapshot.backend_available && snapshot.history_status.can_redo;

    rsx! {
        header {
            class: shell_theme::TOPBAR,

            div {
                class: shell_theme::topbar_section("topbar-section-transport"),
                span { class: shell_theme::TOPBAR_SECTION_LABEL, "Transport" }
                div { class: shell_theme::TOPBAR_GROUP,
                    button {
                        id: "play-toggle",
                        r#type: "button",
                        class: shell_theme::toolbar_button(snapshot.runtime_status.playing),
                        disabled: !can_play || snapshot.loading,
                        title: if !snapshot.backend_available { "Playback requires the live backend." } else if can_play { "Compile and play the current song." } else { "Finish the song graph until playback is ready." },
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                play_runtime(state).await;
                            });
                        },
                        if snapshot.runtime_status.playing { "Pause Preview" } else { "Play Preview" }
                    }
                    button {
                        id: "stop-toggle",
                        r#type: "button",
                        class: shell_theme::toolbar_button(false),
                        disabled: !can_stop || snapshot.loading,
                        title: if !snapshot.backend_available { "Stop requires the live backend." } else { "Stop song playback." },
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                stop_runtime(state).await;
                            });
                        },
                        "Stop"
                    }
                    label {
                        class: shell_theme::TOPBAR_FIELD,
                        span { "Tempo" }
                        input {
                            id: "cpm-value",
                            r#type: "number",
                            min: "20",
                            max: "240",
                            step: "1",
                            value: snapshot.cpm_input.clone(),
                            oninput: move |event| {
                                state.write().cpm_input = event.value();
                            }
                        }
                        small { "cycles/min" }
                    }
                }
            }

            div {
                class: shell_theme::topbar_section("topbar-section-project"),
                span { class: shell_theme::TOPBAR_SECTION_LABEL, "Project" }
                div { class: shell_theme::TOPBAR_GROUP,
                    label {
                        class: shell_theme::TOPBAR_PROJECT_FIELD,
                        span { "Name" }
                        input {
                            id: "project-name-input",
                            r#type: "text",
                            value: snapshot.project_name_input.clone(),
                            placeholder: "Project Name",
                            oninput: move |event| {
                                state.write().project_name_input = event.value();
                            },
                            onblur: move |_| {
                                let next_name = state.read().project_name_input.clone();
                                let state = state;
                                spawn(async move {
                                    rename_project(state, next_name).await;
                                });
                            }
                        }
                    }
                    button {
                        id: "new-project",
                        r#type: "button",
                        class: shell_theme::toolbar_button(false),
                        disabled: !snapshot.backend_available || snapshot.loading,
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                request_project_action(state, PendingProjectAction::NewProject).await;
                            });
                        },
                        "New"
                    }
                    button {
                        id: "open-project",
                        r#type: "button",
                        class: shell_theme::toolbar_button(false),
                        disabled: !snapshot.backend_available || snapshot.loading,
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                request_project_action(state, PendingProjectAction::OpenProject).await;
                            });
                        },
                        "Open"
                    }
                    button {
                        id: "save-project",
                        r#type: "button",
                        class: shell_theme::toolbar_button(false),
                        disabled: !can_save || snapshot.loading,
                        title: if !snapshot.backend_available { "Save requires the live backend." } else { "Save the current project." },
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                let _ = save_project(state, false).await;
                            });
                        },
                        "Save"
                    }
                    button {
                        id: "save-as-project",
                        r#type: "button",
                        class: shell_theme::toolbar_button(false),
                        disabled: !can_save || snapshot.loading,
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                let _ = save_project(state, true).await;
                            });
                        },
                        "Save As"
                    }
                    button {
                        id: "undo-project",
                        r#type: "button",
                        class: shell_theme::toolbar_button(false),
                        disabled: !can_undo || snapshot.loading,
                        title: if !snapshot.backend_available { "Undo requires the live backend." } else { "Undo the last action." },
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                undo_history(state).await;
                            });
                        },
                        "Undo"
                    }
                    button {
                        id: "redo-project",
                        r#type: "button",
                        class: shell_theme::toolbar_button(false),
                        disabled: !can_redo || snapshot.loading,
                        title: if !snapshot.backend_available { "Redo requires the live backend." } else { "Redo the last undone action." },
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                redo_history(state).await;
                            });
                        },
                        "Redo"
                    }
                }
            }

            div {
                class: shell_theme::topbar_section("topbar-section-workspace"),
                span { class: shell_theme::TOPBAR_SECTION_LABEL, "Workspace" }
                div { class: shell_theme::TOPBAR_GROUP,
                    button {
                        id: "workspace-runtime",
                        r#type: "button",
                        class: shell_theme::toolbar_button(workspace_runtime_active),
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                switch_workspace_mode(state, WorkspaceMode::Runtime).await;
                            });
                        },
                        "Song"
                    }
                    button {
                        id: "workspace-init",
                        r#type: "button",
                        class: shell_theme::toolbar_button(workspace_init_active),
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                switch_workspace_mode(state, WorkspaceMode::Init).await;
                            });
                        },
                        "Setup"
                    }
                    button {
                        id: "workspace-back",
                        r#type: "button",
                        class: if show_back {
                            shell_theme::toolbar_button(false)
                        } else {
                            "is-hidden"
                        },
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                switch_workspace_mode(state, WorkspaceMode::Init).await;
                            });
                        },
                        "Back to Setup"
                    }
                    span {
                        id: "mode-chip",
                        class: shell_theme::mode_chip(show_back),
                        title: mode_chip_title,
                        "{top_mode_label}"
                    }
                }
            }

            div {
                class: shell_theme::topbar_section("topbar-section-view"),
                span { class: shell_theme::TOPBAR_SECTION_LABEL, "View" }
                div { class: shell_theme::TOPBAR_GROUP,
                    button {
                        id: "add-node",
                        r#type: "button",
                        class: shell_theme::toolbar_button(snapshot.contextual_library_open),
                        disabled: !picker_available || snapshot.loading,
                        title: if !snapshot.backend_available { "The tile palette needs the live backend." } else { "Open the tile palette." },
                        onclick: move |_| toggle_picker_panel(&mut state.write()),
                        "Tile Palette"
                    }
                    button {
                        id: "open-compile",
                        r#type: "button",
                        class: shell_theme::toolbar_button(activity_open),
                        onclick: move |_| {
                            let mut current = state.write();
                            current.compile_open = !current.compile_open;
                            if current.compile_open {
                                current.activity_tab = ActivityTab::Preview;
                            }
                        },
                        if activity_open { "Hide Activity" } else { "Show Activity" }
                    }
                    button {
                        id: "open-command-palette",
                        r#type: "button",
                        class: shell_theme::toolbar_button(snapshot.command_palette_open),
                        onclick: move |_| toggle_command_palette(&mut state.write()),
                        "Command Palette"
                    }
                }
            }
        }
    }
}
