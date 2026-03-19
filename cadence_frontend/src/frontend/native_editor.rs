use std::collections::BTreeMap;

use dioxus::prelude::*;
use serde_json::Value;

mod command_palette;
mod defaults;
mod graph_interaction;
mod inspector_panel;
mod presentation;
mod project_actions;
mod runtime_telemetry;
mod session;

use self::command_palette::*;
use self::defaults::*;
use self::graph_interaction::*;
use self::inspector_panel::side_config_content;
use self::presentation::*;
use self::project_actions::*;
use self::runtime_telemetry::*;
use self::session::*;
use crate::bridge::{
    runtime,
    tauri::{
        self, CadenceGraphTarget, DiagnosticDto, DiagnosticKind, DiagnosticSeverity,
        DiagnosticsSnapshotDto, DomainBridgeDto, EdgeConnectProbeReason, GraphCompilePreviewDto,
        GraphOp, GraphPickTargetParamDto, GridPos, HistoryStatusDto, InitStageOp,
        InitStageSnapshotDto, ParamDef, ParamSchema, PieceDef, PortType, ProjectCompilePreviewDto,
        ProjectViewDto, RepairSuggestion, RuntimeCommitArgs, RuntimeStatusDto, TileSide,
    },
};
use crate::frontend::modal::compiled_modal::{
    CompileDiagnosticRow, CompileModal, CompileModalViewData,
};
use crate::frontend::modal::tile_inspector::{InspectorModal, InspectorModalViewData};
use crate::frontend::modal::unsaved::UnsavedModal;

const CELL_W: i32 = 64;
const CELL_H: i32 = 64;
const GRID_ORIGIN_X: i32 = 32;
const GRID_ORIGIN_Y: i32 = 32;
const NODE_W: i32 = 60;
const NODE_H: i32 = 60;
const DEFAULT_GRID_COLS: u32 = 12;
const DEFAULT_GRID_ROWS: u32 = 8;
const SAMPLE_ALIAS_PLACEHOLDER: &str = r#"{"gtr":"gtr/0001_cleanC.wav"}"#;
const BACKEND_REQUIRED_MESSAGE: &str =
    "Desktop backend unavailable. Launch the Cadence desktop shell to load and edit a project.";

#[derive(Clone, Debug, PartialEq, Eq)]
enum WorkspaceMode {
    Runtime,
    Init,
    Trick { trick_id: String, name: String },
}

impl WorkspaceMode {
    fn label(&self) -> &'static str {
        match self {
            Self::Runtime => "RUN",
            Self::Init => "INIT",
            Self::Trick { .. } => "TRK",
        }
    }

    fn picker_allowed(&self) -> bool {
        !matches!(self, Self::Init)
    }

    fn graph_target(&self) -> CadenceGraphTarget {
        match self {
            Self::Runtime | Self::Init => CadenceGraphTarget::Runtime,
            Self::Trick { trick_id, .. } => CadenceGraphTarget::Trick {
                trick_id: trick_id.clone(),
            },
        }
    }

    fn workspace_mode_attr(&self) -> &'static str {
        match self {
            Self::Init => "init",
            _ => "runtime",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DragSession {
    PickerPiece { piece_id: String },
    NodeMove { from: GridPos },
    EdgeFrom { from: GridPos },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum DragHoverStatus {
    Valid,
    Invalid,
    Swap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DragHover {
    position: GridPos,
    status: DragHoverStatus,
}

#[derive(Clone, Debug, PartialEq)]
struct GraphNodeView {
    position: GridPos,
    piece_id: String,
    inline_params: BTreeMap<String, Value>,
    input_sides: BTreeMap<String, String>,
    output_side: Option<String>,
    label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct GraphEdgeView {
    id: String,
    from: GridPos,
    to_node: GridPos,
    to_param: String,
}

#[derive(Clone, Debug, PartialEq)]
struct GraphView {
    name: String,
    cols: u32,
    rows: u32,
    nodes: Vec<GraphNodeView>,
    edges: Vec<GraphEdgeView>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PieceInfoPanelData {
    detail_lines: Vec<String>,
    param_lines: Vec<String>,
}

impl PieceInfoPanelData {
    fn is_empty(&self) -> bool {
        self.detail_lines.is_empty() && self.param_lines.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PendingProjectAction {
    NewProject,
    OpenProject,
    QuitApp,
    CloseWindow,
}

#[derive(Clone, Debug, PartialEq)]
struct EditorShellState {
    loading: bool,
    tauri_available: bool,
    project: ProjectViewDto,
    project_name_input: String,
    cpm_input: String,
    workspace_mode: WorkspaceMode,
    graph: GraphView,
    catalog: Vec<PieceDef>,
    init_stage: InitStageSnapshotDto,
    project_preview: ProjectCompilePreviewDto,
    graph_preview: GraphCompilePreviewDto,
    history_status: HistoryStatusDto,
    runtime_status: RuntimeStatusDto,
    diagnostics: DiagnosticsSnapshotDto,
    runtime_boot: runtime::RuntimeBootStatus,
    sample_readiness: runtime::RuntimeSampleReadiness,
    sample_cache: runtime::RuntimeSampleCacheStatus,
    init_sample_status: Vec<runtime::RuntimeInitSampleStatus>,
    selected_cell: Option<GridPos>,
    selected_node: Option<GridPos>,
    picker_open: bool,
    picker_target: Option<GridPos>,
    picker_query: String,
    command_palette_open: bool,
    command_palette_query: String,
    command_palette_selected: usize,
    inspector_open: bool,
    compile_open: bool,
    drag_session: Option<DragSession>,
    drag_hover: Option<DragHover>,
    init_cps_input: String,
    sample_id_input: String,
    sample_source_input: String,
    sample_aliases_input: String,
    trick_name_input: String,
    label_input: String,
    param_inputs: BTreeMap<String, String>,
    pending_probe: Option<GraphPickTargetParamDto>,
    pending_project_action: Option<PendingProjectAction>,
    recovery_path: Option<String>,
    recovery_open: bool,
    recovery_checked: bool,
    recovery_generation: u64,
    status_message: Option<String>,
}

impl Default for EditorShellState {
    fn default() -> Self {
        let project = empty_project_view();
        Self {
            loading: true,
            tauri_available: false,
            project_name_input: project.name.clone(),
            cpm_input: "120".to_string(),
            workspace_mode: WorkspaceMode::Runtime,
            graph: empty_graph(),
            catalog: empty_catalog(),
            init_stage: empty_init_stage(),
            project_preview: empty_project_preview(),
            graph_preview: empty_graph_preview(),
            history_status: default_history_status(),
            runtime_status: default_runtime_status(),
            diagnostics: default_diagnostics_snapshot(),
            runtime_boot: default_runtime_boot_status(),
            sample_readiness: default_sample_readiness(),
            sample_cache: default_sample_cache_status(),
            init_sample_status: Vec::new(),
            selected_cell: None,
            selected_node: None,
            picker_open: false,
            picker_target: None,
            picker_query: String::new(),
            command_palette_open: false,
            command_palette_query: String::new(),
            command_palette_selected: 0,
            inspector_open: false,
            compile_open: false,
            drag_session: None,
            drag_hover: None,
            init_cps_input: String::new(),
            sample_id_input: String::new(),
            sample_source_input: String::new(),
            sample_aliases_input: String::new(),
            trick_name_input: String::new(),
            label_input: String::new(),
            param_inputs: BTreeMap::new(),
            pending_probe: None,
            pending_project_action: None,
            recovery_path: None,
            recovery_open: false,
            recovery_checked: false,
            recovery_generation: 0,
            status_message: Some("Connecting to desktop backend…".to_string()),
            project,
        }
    }
}

#[derive(Clone)]
struct LoadedSnapshot {
    tauri_available: bool,
    project: ProjectViewDto,
    graph: GraphView,
    catalog: Vec<PieceDef>,
    init_stage: InitStageSnapshotDto,
    project_preview: ProjectCompilePreviewDto,
    graph_preview: GraphCompilePreviewDto,
    history_status: HistoryStatusDto,
    runtime_status: RuntimeStatusDto,
    diagnostics: DiagnosticsSnapshotDto,
    recovery_path: Option<String>,
}

#[component]
pub fn NativeEditorApp() -> Element {
    let mut state = use_signal(EditorShellState::default);
    let mut booted = use_signal(|| false);

    use_effect(move || {
        if *booted.read() {
            return;
        }
        booted.set(true);
        spawn(async move {
            load_snapshot(state, true).await;
        });
        spawn(async move {
            let _ = runtime::install_menu_bridge();
            loop {
                process_menu_actions(state).await;
                refresh_runtime_telemetry(state).await;
                if runtime::sleep_ms(500).await.is_err() {
                    break;
                }
            }
        });
    });

    let snapshot = state.read().clone();
    let keydown_snapshot = snapshot.clone();
    let compile_view = compile_modal_view(&snapshot);
    let inspector_view = inspector_modal_view(&snapshot);
    let board_width = GRID_ORIGIN_X * 2 + snapshot.graph.cols as i32 * CELL_W;
    let board_height = GRID_ORIGIN_Y * 2 + snapshot.graph.rows as i32 * CELL_H;
    let filtered_catalog = filtered_catalog(&snapshot);
    let selected_piece = selected_piece_def(&snapshot);
    let selected_node = selected_node_view(&snapshot);
    let piece_panel = selected_piece_panel(&snapshot);
    let picker_available = snapshot.tauri_available && snapshot.workspace_mode.picker_allowed();
    let selection_label = selected_piece
        .map(|piece| {
            format!(
                "Operator: {}",
                compact_piece_label(Some(piece), selected_node)
            )
        })
        .unwrap_or_else(|| "Operator: None".to_string());
    let board_info = format!(
        "Selected: {} | Cursor: [-, -]",
        format_selected_position(
            snapshot
                .selected_node
                .as_ref()
                .or(snapshot.selected_cell.as_ref())
        )
    );
    let side_config_content = side_config_content(state, &snapshot);
    let top_mode_label = snapshot.workspace_mode.label();
    let workspace_runtime_active = matches!(snapshot.workspace_mode, WorkspaceMode::Runtime);
    let workspace_init_active = matches!(snapshot.workspace_mode, WorkspaceMode::Init);
    let show_back = matches!(snapshot.workspace_mode, WorkspaceMode::Trick { .. });
    let can_play = snapshot.tauri_available && snapshot.project_preview.can_play;
    let can_stop = snapshot.tauri_available
        && (snapshot.runtime_status.playing || snapshot.runtime_status.has_program);
    let can_save = snapshot.tauri_available;
    let can_undo = snapshot.tauri_available && snapshot.history_status.can_undo;
    let can_redo = snapshot.tauri_available && snapshot.history_status.can_redo;
    let runtime_boot_label = format_runtime_boot_label(&snapshot.runtime_boot);
    let runtime_playback_label = format_runtime_playback_label(&snapshot);
    let console_output = render_console_output(&snapshot);
    let console_visible = snapshot.status_message.is_some()
        || snapshot.diagnostics.mini_console_visible
        || !snapshot.diagnostics.entries.is_empty();
    let mini_status = selected_piece
        .map(|piece| format!("Selected {}", piece.id))
        .unwrap_or_else(|| "No source selected.".to_string());
    let mini_info_lines = piece_panel
        .detail_lines
        .iter()
        .map(|line| {
            rsx! {
                p {
                    key: "mini-detail-{line}",
                    class: "tile-param-hint",
                    "{line}"
                }
            }
        })
        .collect::<Vec<_>>();
    let mini_param_cards = piece_panel
        .param_lines
        .iter()
        .map(|line| {
            rsx! {
                div {
                    key: "mini-param-{line}",
                    class: "tile-param-card",
                    p { class: "tile-param-title", "{line}" }
                }
            }
        })
        .collect::<Vec<_>>();
    let mode_chip_title = match &snapshot.workspace_mode {
        WorkspaceMode::Runtime => "Runtime graph",
        WorkspaceMode::Init => "Init stage",
        WorkspaceMode::Trick { name, .. } => name.as_str(),
    };
    let data_mode_label = backend_mode_label(snapshot.tauri_available);
    let data_mode_detail = backend_mode_detail(snapshot.tauri_available);
    let backend_mode_class = backend_mode_state_class(snapshot.tauri_available);
    let data_mode_class = format!("backend-mode-chip {backend_mode_class}");
    let status_source_class = format!("status-source {backend_mode_class}");
    let mini_editor_meta = if snapshot.tauri_available {
        "Live project data is rendered directly from the desktop backend."
    } else {
        BACKEND_REQUIRED_MESSAGE
    };
    let status_project = format!(
        "{} N:{} E:{}",
        compact_project_name(&snapshot.project.name, snapshot.project.dirty),
        snapshot.project.node_count,
        snapshot.project.edge_count
    );
    let mini_empty = "Pick a tile to inspect it here.".to_string();
    let has_selected_node = snapshot.selected_node.is_some();
    let sample_cards = snapshot
        .init_stage
        .sample_loads
        .iter()
        .map(|sample| {
            let sample_id = sample.id.clone();
            let state = state;
            rsx! {
                div {
                    key: "{sample.id}",
                    class: "init-card",
                    div {
                        class: "init-card-head",
                        strong { "{sample.id}" }
                        span { class: "init-status is-ready", "{sample.aliases.len()} aliases" }
                    }
                    div {
                        class: "init-card-meta",
                        div {
                            strong { "Source" }
                            span { "{sample.source}" }
                        }
                    }
                    div {
                        class: "init-card-actions",
                        button {
                            r#type: "button",
                            onclick: move |_| {
                                let state = state;
                                let sample_id = sample_id.clone();
                                spawn(async move {
                                    apply_init_ops(state, vec![InitStageOp::SampleLoadRemove { id: sample_id }]).await;
                                });
                            },
                            "Remove"
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let trick_cards = snapshot
        .init_stage
        .tricks
        .iter()
        .map(|trick| {
            let open_trick_id = trick.id.clone();
            let remove_trick_id = trick.id.clone();
            let open_trick_name = trick.name.clone();
            let state = state;
            rsx! {
                div {
                    key: "{trick.id}",
                    class: "init-card",
                    div {
                        class: "init-card-head",
                        strong { "{trick.name}" }
                        span { class: "init-status is-ready", "N:{trick.node_count} E:{trick.edge_count}" }
                    }
                    div {
                        class: "init-card-actions",
                        button {
                            r#type: "button",
                            onclick: move |_| {
                                let state = state;
                                let trick_id = open_trick_id.clone();
                                let trick_name = open_trick_name.clone();
                                spawn(async move {
                                    switch_workspace_mode(
                                        state,
                                        WorkspaceMode::Trick {
                                            trick_id,
                                            name: trick_name,
                                        },
                                    ).await;
                                });
                            },
                            "Open"
                        }
                        button {
                            r#type: "button",
                            onclick: move |_| {
                                let state = state;
                                let trick_id = remove_trick_id.clone();
                                spawn(async move {
                                    apply_init_ops(state, vec![InitStageOp::TrickDelete { id: trick_id }]).await;
                                });
                            },
                            "Remove"
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let rendered_nodes = snapshot.graph.nodes.clone();
    let rendered_catalog = snapshot.catalog.clone();
    let rendered_selected_cell = snapshot.selected_cell;
    let rendered_drag_hover = snapshot.drag_hover.clone();
    let rendered_drag_session = snapshot.drag_session.clone();
    let cell_buttons = (0..snapshot.graph.rows)
        .flat_map(|row| {
            let nodes = rendered_nodes.clone();
            let catalog = rendered_catalog.clone();
            let selected_cell = rendered_selected_cell;
            let drag_hover = rendered_drag_hover.clone();
            let drag_session = rendered_drag_session.clone();
            (0..snapshot.graph.cols).map(move |col| {
                let position = GridPos {
                    col: col as i32,
                    row: row as i32,
                };
                let occupied = nodes.iter().any(|node| node.position == position);
                let mut class_name = String::from("grid-cell");
                if selected_cell.as_ref() == Some(&position) {
                    class_name.push_str(" is-selected");
                }
                if let Some(drag_class) =
                    cell_drag_class(drag_session.as_ref(), drag_hover.as_ref(), &position)
                {
                    class_name.push(' ');
                    class_name.push_str(drag_class);
                }
                if occupied {
                    class_name.push_str(" is-occupied");
                    if let Some(node) = nodes.iter().find(|node| node.position == position) {
                        class_name.push(' ');
                        class_name.push_str(piece_category_class(
                            piece_def_for_id(&catalog, &node.piece_id)
                                .map(|piece| piece.category.as_str())
                                .unwrap_or("generator"),
                        ));
                    }
                }
                let left = GRID_ORIGIN_X + position.col * CELL_W;
                let top = GRID_ORIGIN_Y + position.row * CELL_H;
                let click_position = position;
                let context_position = position;
                let hover_position = position;
                let drop_position = position;
                let mut state = state;
                rsx! {
                    button {
                        key: "cell-{col}-{row}",
                        r#type: "button",
                        class: class_name,
                        "data-grid-pos": "{col}:{row}",
                        title: "Cell {col},{row}",
                        style: "left: {left}px; top: {top}px; width: {CELL_W}px; height: {CELL_H}px;",
                        onclick: move |_| {
                            let mut shell = state.write();
                            shell.selected_cell = Some(click_position);
                            shell.picker_target = Some(click_position);
                            if !occupied {
                                shell.selected_node = None;
                            }
                        },
                        oncontextmenu: move |event| {
                            event.prevent_default();
                            let mut shell = state.write();
                            shell.selected_cell = Some(context_position);
                            shell.selected_node = None;
                            shell.picker_target = Some(context_position);
                            shell.picker_open = true;
                        },
                        ondragover: move |event| {
                            let should_allow_drop = {
                                let mut shell = state.write();
                                update_drag_hover(&mut shell, &hover_position)
                            };
                            if should_allow_drop {
                                event.prevent_default();
                            }
                        },
                        ondrop: move |event| {
                            event.prevent_default();
                            let drag_session = state.read().drag_session.clone();
                            {
                                let mut shell = state.write();
                                clear_drag_state(&mut shell);
                            }
                            if let Some(DragSession::PickerPiece { piece_id }) = drag_session {
                                let state = state;
                                spawn(async move {
                                    place_piece(state, piece_id, Some(drop_position)).await;
                                });
                            }
                        },
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    let edge_lines = snapshot
        .graph
        .edges
        .iter()
        .map(|edge| {
            let (x1, y1) = node_center(&edge.from);
            let (x2, y2) = node_center(&edge.to_node);
            rsx! {
                line {
                    key: "{edge.id}",
                    class: "graph-edge-line",
                    x1: "{x1}",
                    y1: "{y1}",
                    x2: "{x2}",
                    y2: "{y2}",
                }
            }
        })
        .collect::<Vec<_>>();
    let rendered_selected_node = snapshot.selected_node;
    let node_buttons = rendered_nodes
        .iter()
        .map(|node| {
            let position = node.position;
            let node_left = GRID_ORIGIN_X + position.col * CELL_W + (CELL_W - NODE_W) / 2;
            let node_top = GRID_ORIGIN_Y + position.row * CELL_H + (CELL_H - NODE_H) / 2;
            let piece = piece_def_for_id(&rendered_catalog, &node.piece_id);
            let label = compact_piece_label(piece, Some(node));
            let mut class_name = format!(
                "voice-node {}",
                piece_category_class(
                    piece
                        .map(|piece| piece.category.as_str())
                        .unwrap_or("generator"),
                )
            );
            if rendered_selected_node.as_ref() == Some(&position) {
                class_name.push_str(" is-selected");
            }
            let output_handle_class = node_output_handle_class(node, piece);
            let pos_col = position.col;
            let pos_row = position.row;
            let click_position = position;
            let drag_position = position;
            let handle_position = position;
            let mut state = state;
            rsx! {
                button {
                    key: "node-{pos_col}-{pos_row}",
                    id: "node-{pos_col}-{pos_row}",
                    r#type: "button",
                    class: class_name,
                    "data-grid-pos": "{pos_col}:{pos_row}",
                    "data-piece-id": "{node.piece_id}",
                    style: "left: {node_left}px; top: {node_top}px; width: {NODE_W}px; height: {NODE_H}px;",
                    onclick: move |_| {
                        let mut shell = state.write();
                        shell.selected_cell = Some(click_position);
                        shell.selected_node = Some(click_position);
                        shell.picker_target = Some(click_position);
                    },
                    onpointerdown: move |event| {
                        event.prevent_default();
                        let mut shell = state.write();
                        shell.drag_session = Some(DragSession::NodeMove {
                            from: drag_position,
                        });
                        shell.drag_hover = None;
                        shell.selected_cell = Some(drag_position);
                        shell.selected_node = Some(drag_position);
                        shell.picker_target = Some(drag_position);
                    },
                    span {
                        class: "grid-tile-label",
                        "{label}"
                    }
                    if let Some(output_handle_class) = output_handle_class {
                        div {
                            class: "node-output-handle {output_handle_class}",
                            onpointerdown: move |event| {
                                event.prevent_default();
                                event.stop_propagation();
                                let mut shell = state.write();
                                shell.drag_session = Some(DragSession::EdgeFrom {
                                    from: handle_position,
                                });
                                shell.drag_hover = None;
                                shell.selected_cell = Some(handle_position);
                                shell.selected_node = Some(handle_position);
                            },
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let picker_buttons = filtered_catalog
        .iter()
        .map(|piece| {
            let piece_id = piece.id.clone();
            let button_class = format!(
                "grid-piece-picker-item {}",
                piece_category_class(piece.category.as_str())
            );
            let click_piece_id = piece_id.clone();
            let pointer_drag_piece_id = piece_id.clone();
            let html_drag_piece_id = piece_id.clone();
            let mut state = state;
            rsx! {
                button {
                    key: "{piece.id}",
                    class: button_class,
                    "data-piece-id": "{piece.id}",
                    r#type: "button",
                    draggable: "true",
                    onclick: move |_| {
                        clear_drag_state(&mut state.write());
                        let state = state;
                        let piece_id = click_piece_id.clone();
                        spawn(async move {
                            let target = state.read().picker_target;
                            place_piece(state, piece_id, target).await;
                        });
                    },
                    onpointerdown: move |event| {
                        event.prevent_default();
                        let mut shell = state.write();
                        shell.drag_session = Some(DragSession::PickerPiece {
                            piece_id: pointer_drag_piece_id.clone(),
                        });
                        shell.drag_hover = None;
                    },
                    ondragstart: move |_| {
                        let mut shell = state.write();
                        shell.drag_session = Some(DragSession::PickerPiece {
                            piece_id: html_drag_piece_id.clone(),
                        });
                        shell.drag_hover = None;
                    },
                    ondragend: move |_| {
                        let mut shell = state.write();
                        clear_drag_state(&mut shell);
                    },
                    "{piece.label} ({piece.id})"
                }
            }
        })
        .collect::<Vec<_>>();

    rsx! {
        main {
            class: "app-shell",
            tabindex: "0",
            onkeydown: move |event| {
                handle_editor_keydown(event, state, &keydown_snapshot);
            },

            header {
                class: "topbar",

                button {
                    id: "play-toggle",
                    r#type: "button",
                    disabled: !can_play,
                    title: if !snapshot.tauri_available { "Playback requires the desktop backend." } else if can_play { "Compile and play the current runtime." } else { "Compile the runtime graph until playback is available." },
                    onclick: move |_| {
                        let state = state;
                        spawn(async move {
                            play_runtime(state).await;
                        });
                    },
                    if snapshot.runtime_status.playing { "||" } else { ">" }
                }
                button {
                    id: "stop-toggle",
                    r#type: "button",
                    disabled: !can_stop,
                    title: if !snapshot.tauri_available { "Stop requires the desktop backend." } else { "Stop runtime playback." },
                    onclick: move |_| {
                        let state = state;
                        spawn(async move {
                            stop_runtime(state).await;
                        });
                    },
                    "[]"
                }

                button {
                    id: "new-project",
                    r#type: "button",
                    disabled: !snapshot.tauri_available,
                    onclick: move |_| {
                        let state = state;
                        spawn(async move {
                            request_project_action(state, PendingProjectAction::NewProject).await;
                        });
                    },
                    "New"
                }
                button {
                    id: "save-project",
                    r#type: "button",
                    disabled: !can_save,
                    title: if !snapshot.tauri_available { "Save requires the desktop backend." } else { "Save the current project." },
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
                    disabled: !can_save,
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
                    disabled: !can_undo,
                    title: if !snapshot.tauri_available { "Undo requires the desktop backend." } else { "Undo the last action." },
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
                    disabled: !can_redo,
                    title: if !snapshot.tauri_available { "Redo requires the desktop backend." } else { "Redo the last undone action." },
                    onclick: move |_| {
                        let state = state;
                        spawn(async move {
                            redo_history(state).await;
                        });
                    },
                    "Redo"
                }

                label { r#for: "cpm-value", "CPM" }
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

                label { r#for: "project-name-input", "Name" }
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

                button {
                    id: "workspace-runtime",
                    r#type: "button",
                    class: if workspace_runtime_active { "is-active" } else { "" },
                    onclick: move |_| {
                        let state = state;
                        spawn(async move {
                            switch_workspace_mode(state, WorkspaceMode::Runtime).await;
                        });
                    },
                    "Runtime"
                }
                button {
                    id: "workspace-init",
                    r#type: "button",
                    class: if workspace_init_active { "is-active" } else { "" },
                    onclick: move |_| {
                        let state = state;
                        spawn(async move {
                            switch_workspace_mode(state, WorkspaceMode::Init).await;
                        });
                    },
                    "Init"
                }
                button {
                    id: "workspace-back",
                    r#type: "button",
                    class: if show_back { "" } else { "is-hidden" },
                    onclick: move |_| {
                        let state = state;
                        spawn(async move {
                            switch_workspace_mode(state, WorkspaceMode::Init).await;
                        });
                    },
                    "Back"
                }
                button {
                    id: "command-palette-toggle",
                    r#type: "button",
                    class: if snapshot.command_palette_open { "is-active" } else { "" },
                    title: "Open the command palette (Cmd/Ctrl+K).",
                    onclick: move |_| {
                        toggle_command_palette(&mut state.write());
                    },
                    "Palette"
                }
                button {
                    id: "compile-toggle",
                    r#type: "button",
                    onclick: move |_| {
                        let mut current = state.write();
                        current.compile_open = !current.compile_open;
                    },
                    "Code"
                }
                span {
                    id: "mode-chip",
                    class: "mode-chip",
                    title: mode_chip_title,
                    "{top_mode_label}"
                }
                span {
                    id: "backend-mode-chip",
                    class: data_mode_class,
                    title: data_mode_detail,
                    "{data_mode_label}"
                }
                span {
                    id: "project-chip",
                    class: "project-chip",
                    "{compact_project_name(&snapshot.project.name, snapshot.project.dirty)}"
                }
                button {
                    id: "open-project",
                    r#type: "button",
                    disabled: !snapshot.tauri_available,
                    onclick: move |_| {
                        let state = state;
                        spawn(async move {
                            request_project_action(state, PendingProjectAction::OpenProject).await;
                        });
                    },
                    "Open"
                }
                button {
                    id: "add-node",
                    r#type: "button",
                    disabled: !picker_available,
                    onclick: move |_| {
                        toggle_picker_panel(&mut state.write());
                    },
                    "+"
                }
            }

            if !snapshot.tauri_available {
                div {
                    id: "backend-banner",
                    class: "backend-banner",
                    "BACKEND REQUIRED - Launch the desktop shell to load, edit, and play a project."
                }
            }

            section {
                class: "workspace-main",
                "aria-label": "Grid workspace",
                "data-workspace-mode": snapshot.workspace_mode.workspace_mode_attr(),

                if !snapshot.tauri_available {
                    section {
                        id: "backend-unavailable",
                        class: "backend-unavailable",

                        strong { "Desktop backend unavailable" }
                        p {
                            "Open Cadence through the Tauri desktop shell to load a project, edit tiles, and control playback."
                        }
                        button {
                            id: "retry-backend-connect",
                            r#type: "button",
                            onclick: move |_| {
                                let state = state;
                                spawn(async move {
                                    load_snapshot(state, true).await;
                                });
                            },
                            "Retry connection"
                        }
                    }
                }

                if matches!(snapshot.workspace_mode, WorkspaceMode::Init) {
                    section {
                        id: "init-workspace",
                        class: "init-workspace",
                        "aria-label": "Init workspace",

                        section {
                            class: "init-section",
                            header {
                                class: "init-section-head",
                                strong { "Setup" }
                                span { class: "role-chip", "Init Stage" }
                            }

                            label { class: "field", r#for: "init-cps-input", "CPS Expression" }
                            div {
                                class: "init-inline",
                                input {
                                    id: "init-cps-input",
                                    r#type: "text",
                                    value: snapshot.init_cps_input.clone(),
                                    placeholder: "113/60/4",
                                    oninput: move |event| {
                                        state.write().init_cps_input = event.value();
                                    }
                                }
                                button {
                                    id: "init-cps-save",
                                    r#type: "button",
                                    onclick: move |_| {
                                        let state = state;
                                        spawn(async move {
                                            let expr = {
                                                let value = state.read().init_cps_input.trim().to_string();
                                                if value.is_empty() {
                                                    None
                                                } else {
                                                    Some(value)
                                                }
                                            };
                                            apply_init_ops(state, vec![InitStageOp::SetCps { expr }]).await;
                                        });
                                    },
                                    "Refresh"
                                }
                            }
                        }

                        div {
                            class: "init-columns",

                            section {
                                class: "init-section",
                                header {
                                    class: "init-section-head",
                                    strong { "Sample Loads" }
                                    span { id: "init-sample-summary", class: "node-chip", "{snapshot.init_stage.sample_loads.len()}" }
                                }

                                div {
                                    class: "init-form-grid",
                                    label { class: "field", r#for: "init-sample-id", "Load ID" }
                                    input {
                                        id: "init-sample-id",
                                        r#type: "text",
                                        value: snapshot.sample_id_input.clone(),
                                        placeholder: "gtr",
                                        oninput: move |event| state.write().sample_id_input = event.value()
                                    }

                                    label { class: "field", r#for: "init-sample-source", "Source" }
                                    input {
                                        id: "init-sample-source",
                                        r#type: "text",
                                        value: snapshot.sample_source_input.clone(),
                                        placeholder: "github:tidalcycles/Dirt-Samples/master/",
                                        oninput: move |event| state.write().sample_source_input = event.value()
                                    }

                                    label { class: "field", r#for: "init-sample-aliases", "Aliases (JSON)" }
                                    textarea {
                                        id: "init-sample-aliases",
                                        class: "init-textarea",
                                        spellcheck: "false",
                                        value: snapshot.sample_aliases_input.clone(),
                                        placeholder: SAMPLE_ALIAS_PLACEHOLDER,
                                        oninput: move |event| state.write().sample_aliases_input = event.value()
                                    }
                                }

                                div {
                                    class: "init-actions",
                                    button {
                                        id: "init-sample-save",
                                        r#type: "button",
                                        onclick: move |_| {
                                            let state = state;
                                            spawn(async move {
                                                upsert_sample_load(state).await;
                                            });
                                        },
                                        "Upsert Sample Load"
                                    }
                                }

                                div {
                                    id: "init-sample-list",
                                    class: "init-list",
                                    {sample_cards.into_iter()}
                                }
                            }

                            section {
                                class: "init-section",
                                header {
                                    class: "init-section-head",
                                    strong { "Tricks" }
                                    span { id: "init-trick-summary", class: "node-chip", "{snapshot.init_stage.tricks.len()}" }
                                }

                                div {
                                    class: "init-inline",
                                    input {
                                        id: "init-trick-name",
                                        r#type: "text",
                                        value: snapshot.trick_name_input.clone(),
                                        placeholder: "melodia",
                                        oninput: move |event| state.write().trick_name_input = event.value()
                                    }
                                    button {
                                        id: "init-trick-create",
                                        r#type: "button",
                                        onclick: move |_| {
                                            let state = state;
                                            spawn(async move {
                                                create_trick(state).await;
                                            });
                                        },
                                        "Create Trick"
                                    }
                                }

                                div {
                                    id: "init-trick-list",
                                    class: "init-list",
                                    {trick_cards.into_iter()}
                                }
                            }
                        }
                    }
                } else {
                    section {
                        id: "grid-window",
                        class: "grid-window",

                        section {
                            class: "editor-stage",

                            button {
                                id: "selected-piece-name",
                                class: "selection-banner",
                                r#type: "button",
                                onclick: move |_| {
                                    if has_selected_node {
                                        state.write().inspector_open = true;
                                    }
                                },
                                "{selection_label}"
                            }

                            section {
                                class: "editor-board",

                                section {
                                    class: "canvas-shell",

                                    section {
                                        id: "canvas",
                                        class: "canvas",
                                        "aria-label": "Node canvas",

                                        div {
                                            id: "canvas-grid",
                                            class: "canvas-grid",
                                            style: "width: {board_width}px; height: {board_height}px; --grid-cell-w: {CELL_W}px; --grid-cell-h: {CELL_H}px; --grid-origin-x: {GRID_ORIGIN_X}px; --grid-origin-y: {GRID_ORIGIN_Y}px;",
                                            {cell_buttons.into_iter()}
                                        }

                                        svg {
                                            id: "edge-layer",
                                            class: "edge-layer",
                                            xmlns: "http://www.w3.org/2000/svg",
                                            width: "{board_width}",
                                            height: "{board_height}",
                                            view_box: "0 0 {board_width} {board_height}",
                                            style: "width: {board_width}px; height: {board_height}px;",
                                            {edge_lines.into_iter()}
                                        }

                                        div {
                                            id: "canvas-layer",
                                            class: "canvas-layer",
                                            style: "width: {board_width}px; height: {board_height}px;",
                                            {node_buttons.into_iter()}
                                        }

                                        if snapshot.drag_session.is_some() {
                                            div {
                                                id: "canvas-drag-overlay",
                                                class: "canvas-drag-overlay {canvas_drag_overlay_class(snapshot.drag_session.as_ref())}",
                                                style: "width: {board_width}px; height: {board_height}px;",
                                                onpointermove: move |event| {
                                                    let mut shell = state.write();
                                                    if let Some(position) =
                                                        grid_position_from_canvas_event(&event, &shell.graph)
                                                    {
                                                        update_drag_hover(&mut shell, &position);
                                                    } else {
                                                        shell.drag_hover = None;
                                                    }
                                                },
                                                onpointerleave: move |_| {
                                                    state.write().drag_hover = None;
                                                },
                                                onpointercancel: move |_| {
                                                    let mut shell = state.write();
                                                    clear_drag_state(&mut shell);
                                                },
                                                onpointerup: move |event| {
                                                    let (target, target_has_node, drag_session) = {
                                                        let shell = state.read();
                                                        let target =
                                                            grid_position_from_canvas_event(&event, &shell.graph);
                                                        let target_has_node = target
                                                            .as_ref()
                                                            .is_some_and(|position| {
                                                                is_cell_occupied(&shell.graph, position)
                                                            });
                                                        (target, target_has_node, shell.drag_session.clone())
                                                    };

                                                    {
                                                        let mut shell = state.write();
                                                        clear_drag_state(&mut shell);
                                                    }

                                                    match (drag_session, target, target_has_node) {
                                                        (
                                                            Some(DragSession::NodeMove { from }),
                                                            Some(position),
                                                            _,
                                                        ) => {
                                                            let state = state;
                                                            spawn(async move {
                                                                move_node(state, from, position).await;
                                                            });
                                                        }
                                                        (
                                                            Some(DragSession::PickerPiece { piece_id }),
                                                            Some(position),
                                                            _,
                                                        ) => {
                                                            let state = state;
                                                            spawn(async move {
                                                                place_piece(state, piece_id, Some(position)).await;
                                                            });
                                                        }
                                                        (
                                                            Some(DragSession::EdgeFrom { from }),
                                                            Some(position),
                                                            true,
                                                        ) => {
                                                            let state = state;
                                                            spawn(async move {
                                                                connect_nodes(state, from, position).await;
                                                            });
                                                        }
                                                        _ => {}
                                                    }
                                                },
                                            }
                                        }

                                        div {
                                            id: "canvas-marquee",
                                            class: "canvas-marquee is-hidden",
                                            "aria-hidden": "true",
                                        }
                                    }

                                    section {
                                        class: "board-footer",
                                        "aria-label": "Board tools",

                                        section {
                                            class: "board-info",
                                            "aria-label": "Board info",

                                            div {
                                                id: "board-selection-info",
                                                class: "board-selection-info",
                                                "{board_info}"
                                            }
                                        }

                                        section {
                                            id: "side-config",
                                            class: "side-config",
                                            "aria-label": "Node configuration",

                                            header {
                                                class: "side-config-head",
                                                span { id: "side-config-title", "Config" }
                                            }

                                            div {
                                                id: "side-config-body",
                                                class: "side-config-body",
                                                {side_config_content}
                                            }
                                        }
                                    }
                                }

                                aside {
                                    id: "mini-editor-pane",
                                    class: "mini-editor-pane",
                                    "aria-label": "Mininotation editor",

                                    header {
                                        class: "mini-editor-head",

                                        div {
                                            class: "mini-editor-head-copy",
                                            strong { id: "mini-editor-title", "Native Dioxus Editor" }
                                            p {
                                                id: "mini-editor-meta",
                                                class: "mini-editor-meta",
                                                "{mini_editor_meta}"
                                            }
                                        }
                                    }

                                    div {
                                        id: "mini-editor-status",
                                        class: "mini-editor-status",
                                        "{mini_status}"
                                    }

                                    button {
                                        id: "mini-editor-jump",
                                        r#type: "button",
                                        class: "mini-editor-jump is-hidden",
                                        "Jump to source"
                                    }

                                    div {
                                        id: "mini-editor-host",
                                        class: if piece_panel.is_empty() {
                                            "mini-editor-host"
                                        } else {
                                            "mini-editor-host is-active"
                                        },

                                        div {
                                            class: "tile-data-scroll",

                                            if !piece_panel.detail_lines.is_empty() {
                                                div {
                                                    class: "tile-info-card",
                                                    p { class: "tile-section-title", "Tile Info" }
                                                    {mini_info_lines.into_iter()}
                                                }
                                            }

                                            if !piece_panel.param_lines.is_empty() {
                                                div {
                                                    class: "tile-controls-stack",
                                                    p { class: "tile-section-title", "Parameters" }
                                                    {mini_param_cards.into_iter()}
                                                }
                                            }
                                        }
                                    }

                                    if piece_panel.is_empty() {
                                        div {
                                            id: "mini-editor-empty",
                                            class: "mini-editor-empty",
                                            "{mini_empty}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                section {
                    id: "grid-piece-picker",
                    class: if picker_available && snapshot.picker_open {
                        "grid-piece-picker"
                    } else {
                        "grid-piece-picker is-hidden"
                    },
                    "aria-label": "Place tile",

                    header { class: "grid-piece-picker-head", "Place Tile" }
                    input {
                        id: "grid-piece-picker-search",
                        class: "grid-piece-picker-search",
                        r#type: "search",
                        value: snapshot.picker_query.clone(),
                        placeholder: "Search...",
                        "aria-label": "Search tiles",
                        oninput: move |event| {
                            state.write().picker_query = event.value();
                        }
                    }
                    div {
                        id: "grid-piece-picker-list",
                        class: "grid-piece-picker-list",
                        {picker_buttons.into_iter()}
                    }
                }
            }

            {command_palette_overlay(state, &snapshot)}

            section {
                id: "mini-console",
                class: if console_visible { "mini-console" } else { "mini-console is-hidden" },
                "aria-label": "Mini console",

                div {
                    class: "mini-console-head",
                    span { "Console" }
                    button {
                        id: "mini-console-toggle",
                        r#type: "button",
                        onclick: move |_| {
                            let state = state;
                            spawn(async move {
                                set_mini_console_visible(state, false).await;
                            });
                        },
                        "X"
                    }
                }

                pre {
                    id: "mini-console-output",
                    class: "mini-console-output",
                    "{console_output}"
                }
            }

            InspectorModal {
                view_data: inspector_view,
                on_close: move |_| {
                    state.write().inspector_open = false;
                },
                on_delete: move |_| {
                    let state = state;
                    spawn(async move {
                        delete_selected_node(state).await;
                    });
                },
            }

            CompileModal {
                view_data: compile_view,
                on_close: move |_| {
                    state.write().compile_open = false;
                },
                on_refresh: move |_| {
                    let state = state;
                    spawn(async move {
                        load_snapshot(state, false).await;
                    });
                },
            }

            UnsavedModal {
                is_visible: snapshot.pending_project_action.is_some() || snapshot.recovery_open,
                title: if snapshot.recovery_open {
                    "Recovery Snapshot Found".to_string()
                } else {
                    "Unsaved Changes".to_string()
                },
                message: if snapshot.recovery_open {
                    snapshot
                        .recovery_path
                        .clone()
                        .map(|path| format!("Cadence found a recovery snapshot at {path}. Restore it before continuing?"))
                        .unwrap_or_else(|| "Cadence found a recovery snapshot. Restore it before continuing?".to_string())
                } else {
                    "You have unsaved changes. Save before continuing?".to_string()
                },
                save_label: if snapshot.recovery_open {
                    "Restore".to_string()
                } else {
                    "Save".to_string()
                },
                discard_label: if snapshot.recovery_open {
                    "Discard Recovery".to_string()
                } else {
                    "Discard".to_string()
                },
                cancel_label: "Cancel".to_string(),
                on_save: move |_| {
                    let state = state;
                    spawn(async move {
                        if state.read().recovery_open {
                            restore_recovery_snapshot(state).await;
                        } else {
                            confirm_unsaved_save(state).await;
                        }
                    });
                },
                on_discard: move |_| {
                    let state = state;
                    spawn(async move {
                        if state.read().recovery_open {
                            discard_recovery_snapshot(state).await;
                        } else {
                            confirm_unsaved_discard(state).await;
                        }
                    });
                },
                on_cancel: move |_| {
                    let mut shell = state.write();
                    shell.pending_project_action = None;
                    shell.recovery_open = false;
                },
            }

            footer {
                class: "statusbar",
                span {
                    id: "status-source",
                    class: status_source_class,
                    title: data_mode_detail,
                    "Data: {data_mode_label}"
                }
                span { id: "status-host", "{runtime_boot_label}" }
                span { id: "status-playback", "{runtime_playback_label}" }
                span { id: "status-selection", "{format_selected_position(snapshot.selected_node.as_ref().or(snapshot.selected_cell.as_ref()))}" }
                span { id: "status-project", "{status_project}" }
            }
        }
    }
}

fn grid_position_from_canvas_event(event: &PointerEvent, graph: &GraphView) -> Option<GridPos> {
    let point = event.data().element_coordinates();
    grid_position_from_canvas_coords(point.x, point.y, graph)
}

fn grid_position_from_canvas_coords(x: f64, y: f64, graph: &GraphView) -> Option<GridPos> {
    let x = x - f64::from(GRID_ORIGIN_X);
    let y = y - f64::from(GRID_ORIGIN_Y);
    if x < 0.0 || y < 0.0 {
        return None;
    }

    let col = (x / f64::from(CELL_W)).floor() as i32;
    let row = (y / f64::from(CELL_H)).floor() as i32;
    if col < 0 || row < 0 || col >= graph.cols as i32 || row >= graph.rows as i32 {
        return None;
    }

    Some(GridPos { col, row })
}

fn update_drag_hover(state: &mut EditorShellState, position: &GridPos) -> bool {
    let Some(status) = drag_hover_status_for_cell(state, position) else {
        if state.drag_hover.as_ref().map(|hover| &hover.position) == Some(position) {
            state.drag_hover = None;
        }
        return false;
    };

    state.drag_hover = Some(DragHover {
        position: *position,
        status,
    });
    true
}

fn canvas_drag_overlay_class(drag_session: Option<&DragSession>) -> &'static str {
    match drag_session {
        Some(DragSession::EdgeFrom { .. }) => "is-connecting",
        Some(DragSession::NodeMove { .. }) | Some(DragSession::PickerPiece { .. }) => "is-dragging",
        None => "",
    }
}

fn preferred_picker_target(state: &EditorShellState) -> Option<GridPos> {
    state
        .selected_cell
        .filter(|pos| !is_cell_occupied(&state.graph, pos))
        .or_else(|| first_free_position(&state.graph))
}

fn drag_hover_status_for_cell(
    state: &EditorShellState,
    position: &GridPos,
) -> Option<DragHoverStatus> {
    match state.drag_session.as_ref()? {
        DragSession::PickerPiece { .. } => Some(if is_cell_occupied(&state.graph, position) {
            DragHoverStatus::Invalid
        } else {
            DragHoverStatus::Valid
        }),
        DragSession::NodeMove { from } => {
            if from == position {
                None
            } else if is_cell_occupied(&state.graph, position) {
                Some(DragHoverStatus::Swap)
            } else {
                Some(DragHoverStatus::Valid)
            }
        }
        DragSession::EdgeFrom { .. } => None,
    }
}

fn cell_drag_class(
    drag_session: Option<&DragSession>,
    drag_hover: Option<&DragHover>,
    position: &GridPos,
) -> Option<&'static str> {
    let hover = drag_hover.filter(|hover| hover.position == *position)?;
    match (drag_session?, &hover.status) {
        (DragSession::PickerPiece { .. }, DragHoverStatus::Valid) => Some("is-drop-valid"),
        (DragSession::PickerPiece { .. }, DragHoverStatus::Invalid) => Some("is-drop-invalid"),
        (DragSession::NodeMove { .. }, DragHoverStatus::Valid) => Some("is-move-valid"),
        (DragSession::NodeMove { .. }, DragHoverStatus::Swap) => Some("is-move-swap"),
        (DragSession::NodeMove { .. }, DragHoverStatus::Invalid) => Some("is-move-invalid"),
        _ => None,
    }
}

fn default_input_sides(piece: Option<&PieceDef>) -> BTreeMap<String, String> {
    piece
        .map(|piece| {
            piece
                .params
                .iter()
                .map(|param| {
                    (
                        param.id.clone(),
                        canonical_side(param.side.as_str()).to_string(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn remap_edges_for_position(edges: &mut [GraphEdgeView], from: &GridPos, to: &GridPos) {
    for edge in edges {
        if edge.from == *from {
            edge.from = *to;
        }
        if edge.to_node == *from {
            edge.to_node = *to;
        }
    }
}

fn swap_edges_for_positions(edges: &mut [GraphEdgeView], a: &GridPos, b: &GridPos) {
    for edge in edges {
        if edge.from == *a {
            edge.from = *b;
        } else if edge.from == *b {
            edge.from = *a;
        }
        if edge.to_node == *a {
            edge.to_node = *b;
        } else if edge.to_node == *b {
            edge.to_node = *a;
        }
    }
}

fn node_output_handle_class(
    node: &GraphNodeView,
    piece: Option<&PieceDef>,
) -> Option<&'static str> {
    let side = node
        .output_side
        .as_deref()
        .or(piece.and_then(|piece| piece.output_side.as_deref()))?;
    Some(match canonical_side(side) {
        "left" => "side-left",
        "top" => "side-top",
        "bottom" => "side-bottom",
        _ => "side-right",
    })
}

fn canonical_side(side: &str) -> &'static str {
    match side {
        "east" | "right" => "right",
        "west" | "left" => "left",
        "north" | "top" => "top",
        "south" | "bottom" => "bottom",
        _ => "right",
    }
}

fn first_free_position(graph: &GraphView) -> Option<GridPos> {
    for row in 0..graph.rows as i32 {
        for col in 0..graph.cols as i32 {
            let position = GridPos { col, row };
            if graph.nodes.iter().all(|node| node.position != position) {
                return Some(position);
            }
        }
    }
    None
}

fn is_cell_occupied(graph: &GraphView, position: &GridPos) -> bool {
    graph.nodes.iter().any(|node| node.position == *position)
}

fn node_center(position: &GridPos) -> (i32, i32) {
    (
        GRID_ORIGIN_X + position.col * CELL_W + CELL_W / 2,
        GRID_ORIGIN_Y + position.row * CELL_H + CELL_H / 2,
    )
}

fn slugify_identifier(value: &str) -> String {
    let normalized = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = normalized.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "trick".to_string()
    } else {
        trimmed
    }
}

fn tile_side_to_string(side: TileSide) -> &'static str {
    match side {
        TileSide::Left => "left",
        TileSide::Top => "top",
        TileSide::Right => "right",
        TileSide::Bottom => "bottom",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_mode_disables_picker_in_init() {
        assert!(WorkspaceMode::Runtime.picker_allowed());
        assert!(!WorkspaceMode::Init.picker_allowed());
        assert!(
            WorkspaceMode::Trick {
                trick_id: "melodia".to_string(),
                name: "Melodia".to_string(),
            }
            .picker_allowed()
        );
    }

    #[test]
    fn merge_loaded_snapshot_closes_picker_and_drag_state() {
        let current = EditorShellState {
            workspace_mode: WorkspaceMode::Init,
            loading: true,
            picker_open: true,
            picker_target: Some(GridPos { col: 4, row: 4 }),
            drag_session: Some(DragSession::PickerPiece {
                piece_id: "core.and".to_string(),
            }),
            drag_hover: Some(DragHover {
                position: GridPos { col: 4, row: 4 },
                status: DragHoverStatus::Valid,
            }),
            selected_node: Some(GridPos { col: 0, row: 0 }),
            selected_cell: Some(GridPos { col: 0, row: 0 }),
            ..EditorShellState::default()
        };

        let next = merge_loaded_snapshot(
            &current,
            LoadedSnapshot {
                tauri_available: true,
                project: empty_project_view(),
                graph: empty_graph(),
                catalog: empty_catalog(),
                init_stage: empty_init_stage(),
                project_preview: empty_project_preview(),
                graph_preview: empty_graph_preview(),
                history_status: default_history_status(),
                runtime_status: default_runtime_status(),
                diagnostics: default_diagnostics_snapshot(),
                recovery_path: None,
            },
            false,
        );

        assert!(!next.loading);
        assert!(!next.picker_open);
        assert_eq!(next.picker_target, None);
        assert_eq!(next.drag_session, None);
        assert_eq!(next.drag_hover, None);
        assert_eq!(next.selected_node, None);
    }

    #[test]
    fn backend_mode_copy_is_explicit() {
        assert_eq!(backend_mode_label(true), "LIVE BACKEND");
        assert_eq!(backend_mode_label(false), "BACKEND REQUIRED");
        assert!(backend_mode_detail(true).contains("live project data"));
        assert!(backend_mode_detail(false).contains("Desktop backend unavailable"));
    }
}
