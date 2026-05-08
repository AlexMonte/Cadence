use std::collections::BTreeMap;

use serde_json::Value;

use crate::adapter::{EdgeConnectProbeReason, GraphOp, GridPos, RepairSuggestion};
use crate::adapter::dioxus::editor_service::{
    apply_graph_ops, disconnect_edge, move_node, set_output_side, set_param_side,
};
use crate::application::editor::{
    EditorShellState, EDITING_SURFACE_INVARIANT, format_param_schema, piece_def_for_id,
    selected_node_view, selected_piece_def, side_config_text, tile_side_to_string,
};
use crate::domain::canonical_side;
use crate::application::editor::{
    format_runtime_boot_label, format_runtime_playback_label,
};
use dioxus::prelude::*;
pub(crate) fn tile_detail_panel_content(
    mut state: Signal<EditorShellState>,
    snapshot: &EditorShellState,
) -> Element {
    let runtime_error = snapshot
        .runtime_status
        .last_error
        .clone()
        .or_else(|| snapshot.status_message.clone());
    let probe = snapshot.inspector.pending_probe.clone();
    let selected_node = selected_node_view(snapshot).cloned();
    let selected_piece = selected_piece_def(snapshot).cloned();
    let selected_count = snapshot.selected_nodes.len();
    let multi_selected = selected_count > 1;
    let tile_help = side_config_text(snapshot);
    let tile_panel_advanced_open = snapshot.inspector.show_advanced;
    let mut piece_counts = BTreeMap::new();
    if multi_selected {
        for node in snapshot
            .graph
            .nodes
            .iter()
            .filter(|node| snapshot.selected_nodes.contains(&node.position))
        {
            let label = piece_def_for_id(&snapshot.catalog, &node.piece_id)
                .map(|piece| piece.label.clone())
                .unwrap_or_else(|| node.piece_id.clone());
            *piece_counts.entry(label).or_insert(0usize) += 1;
        }
    }
    let mut positions = snapshot.selected_nodes.clone();
    positions.sort_by_key(|pos| (pos.row, pos.col));
    let position_preview = positions
        .iter()
        .take(6)
        .map(|pos| format!("[{}, {}]", pos.col, pos.row))
        .collect::<Vec<_>>()
        .join(", ");
    let hidden_positions = positions.len().saturating_sub(6);

    rsx! {
        if let Some(probe) = probe {
            div {
                class: "side-config-group side-config-group-emphasis",
                p { class: "side-config-section-title", "Wiring suggestion" }
                p {
                    class: "side-config-help",
                    "{format_probe_reason(probe.reason.as_ref())}"
                }
                if let Some(detail) = probe.detail {
                    p { class: "side-config-port-note", "{detail}" }
                }
                if !probe.suggestions.is_empty() {
                    div {
                        class: "init-actions",
                        for suggestion in probe.suggestions {
                            button {
                                key: "{format_repair_suggestion(&suggestion)}",
                                class: "side-config-button",
                                r#type: "button",
                                title: "{format_repair_suggestion(&suggestion)}",
                                onclick: move |_| {
                                    let state = state;
                                    let suggestion = suggestion.clone();
                                    spawn(async move {
                                        apply_repair_suggestion(state, suggestion).await;
                                    });
                                },
                                "{format_repair_suggestion(&suggestion)}"
                            }
                        }
                    }
                }
            }
        }

        if multi_selected {
            div {
                class: "side-config-group",
                p { class: "side-config-section-title", "Selection" }
                div {
                    class: "side-config-summary-grid",
                    p { class: "side-config-help", "{selected_count} tiles are selected." }
                    p {
                        class: "side-config-port-note",
                        "Group move and delete are active. Port wiring, labels, and per-tile editing return when the selection collapses back to one tile."
                    }
                }
            }

            div {
                class: "side-config-group side-config-group-secondary",
                p { class: "side-config-section-title", "Tiles in group" }
                div {
                    class: "side-config-value",
                    for (label, count) in piece_counts {
                        p { class: "side-config-help", "{label}: {count}" }
                    }
                }
            }

            div {
                class: "side-config-group side-config-group-secondary",
                p { class: "side-config-section-title", "Positions" }
                p { class: "side-config-help", "{position_preview}" }
                if hidden_positions > 0 {
                    p {
                        class: "side-config-port-note",
                        "{hidden_positions} more tile(s) are also selected."
                    }
                }
            }

            if let Some(error) = runtime_error {
                div {
                    class: "side-config-group side-config-group-secondary",
                    p { class: "side-config-section-title", "Runtime" }
                    p {
                        id: "runtime-error-detail",
                        class: "side-config-port-note",
                        "Latest runtime message: {error}"
                    }
                }
            }
        } else if let (Some(node), Some(piece)) = (selected_node, selected_piece) {
            div {
                class: "side-config-group",
                p { class: "side-config-section-title", "Tile" }
                div {
                    class: "side-config-summary-grid",
                    input {
                        r#type: "text",
                        value: snapshot.inspector.label_input.clone(),
                        placeholder: piece.label.clone(),
                        "aria-label": "Tile name",
                        oninput: move |event| {
                            state.write().inspector.label_input = event.value();
                        },
                        onblur: move |_| {
                            let state = state;
                            let position = node.position;
                            spawn(async move {
                                commit_node_label(state, position).await;
                            });
                        }
                    }
                    if let Some(description) = piece.description.clone() {
                        p { class: "side-config-help", "{description}" }
                    }
                    p { class: "side-config-port-note", "{tile_help}" }
                }
            }

            div {
                class: "side-config-group side-config-group-secondary",
                if piece.semantic_kind == "input" {
                    p {
                        class: "side-config-help",
                        "{EDITING_SURFACE_INVARIANT}"
                    }
                } else {
                    p {
                        class: "side-config-help",
                        "This tile now expects connected pattern/control inputs instead of inspector-edited inline values."
                    }
                    if !node.inline_params.is_empty() {
                        p {
                            class: "side-config-port-note",
                            "Legacy inline values still load, but they are read-only here. Move them into input tiles if you want to keep editing them."
                        }
                    }
                }
            }

            if piece.output_side.is_some() {
                div {
                    class: "side-config-group",
                    p { class: "side-config-section-title", "Output side" }
                    div {
                        class: "compact-segmented-control",
                        for side in ["left", "top", "right", "bottom"] {
                            button {
                                key: "output-side-{side}",
                                class: "side-config-button",
                                r#type: "button",
                                title: "{side}",
                                onclick: move |_| {
                                    let state = state;
                                    let position = node.position;
                                    let side = side.to_string();
                                    spawn(async move {
                                        set_output_side(state, position, side).await;
                                    });
                                },
                                "{short_side_label(side)}"
                            }
                        }
                    }
                }
            }

            div {
                class: "side-config-group",
                p { class: "side-config-section-title", "Inputs" }
                div {
                    class: "side-config-port-list",
                    for param in piece.params.iter().cloned() {
                        {
                            let param_id = param.id.clone();
                            let param_label = param.label.clone();
                            let param_schema = format_param_schema(&param);
                            let connected_edge = snapshot
                                .graph
                                .edges
                                .iter()
                                .find(|edge| edge.to_node == node.position && edge.to_param == param_id);
                            let has_inline_value = node.inline_params.contains_key(&param_id);
                            let active_side = node
                                .input_sides
                                .get(&param_id)
                                .cloned()
                                .unwrap_or_else(|| canonical_side(param.side.as_str()).to_string());
                            let status = if connected_edge.is_some() {
                                "connected"
                            } else if has_inline_value {
                                "inline"
                            } else if param.required {
                                "required"
                            } else {
                                "open"
                            };
                            rsx! {
                                div {
                                    key: "port-summary-{node.position.col}-{node.position.row}-{param_id}",
                                    class: "side-config-port-row",
                                    strong { "{param_label}" }
                                    span { "{param_schema}" }
                                    div {
                                        class: "compact-segmented-control",
                                        for side in ["left", "top", "right", "bottom"] {
                                            {
                                                let button_param_id = param_id.clone();
                                                rsx! {
                                                    button {
                                                        key: "input-side-{node.position.col}-{node.position.row}-{param_id}-{side}",
                                                        class: if active_side == side {
                                                            "side-config-button is-connected"
                                                        } else {
                                                            "side-config-button"
                                                        },
                                                        r#type: "button",
                                                        title: "{side}",
                                                        onclick: {
                                                            let param_id = button_param_id.clone();
                                                            move |_| {
                                                                let state = state;
                                                                let position = node.position;
                                                                let param_id = param_id.clone();
                                                                let side = side.to_string();
                                                                spawn(async move {
                                                                    set_param_side(state, position, param_id, side).await;
                                                                });
                                                            }
                                                        },
                                                        "{short_side_label(side)}"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    span { "{status}" }
                                }
                            }
                        }
                    }
                }
            }

            if tile_panel_advanced_open {
                for param in piece.params {
                    {
                        let position = node.position;
                        let left_param_id = param.id.clone();
                        let top_param_id = param.id.clone();
                        let right_param_id = param.id.clone();
                        let bottom_param_id = param.id.clone();
                        let connected_edge = snapshot
                            .graph
                            .edges
                            .iter()
                            .find(|edge| edge.to_node == node.position && edge.to_param == param.id)
                            .cloned();
                        let has_inline_value = node.inline_params.contains_key(&param.id);
                        let active_side = node
                            .input_sides
                            .get(&param.id)
                            .cloned()
                            .unwrap_or_else(|| canonical_side(param.side.as_str()).to_string());
                        let param_status = if connected_edge.is_some() {
                            "Connected".to_string()
                        } else if has_inline_value {
                            "Inline value".to_string()
                        } else if param.required {
                            "Required".to_string()
                        } else {
                            "Optional".to_string()
                        };
                        let param_note = if let Some(edge) = connected_edge.clone() {
                            Some(format!(
                                "Connected from tile [{}, {}]",
                                edge.from.col, edge.from.row
                            ))
                        } else if param.required && !has_inline_value {
                            Some("Required before playback".to_string())
                        } else {
                            None
                        };
                        rsx! {
                            div {
                                key: "param-{position.col}-{position.row}-{param.id}",
                                class: "side-config-group side-config-param-group",
                                div { class: "side-config-param-head",
                                    div {
                                        class: "side-config-param-title",
                                        p {
                                            class: "side-config-section-title",
                                            "{param.label}"
                                        }
                                        p {
                                            class: "side-config-help",
                                            "{format_param_schema(&param)}"
                                        }
                                    }
                                    div {
                                        class: "side-config-chip-row",
                                        span { class: "tile-param-badge", "{param_status}" }
                                        span { class: "tile-param-badge is-muted", "{active_side}" }
                                    }
                                }
                                if let Some(param_note) = param_note {
                                    p {
                                        class: "side-config-port-note",
                                        "{param_note}"
                                    }
                                }
                                if let Some(inline_value) = node.inline_params.get(&param.id) {
                                    p {
                                        class: "side-config-help",
                                        "Current value: {format_inline_param_value(inline_value)}"
                                    }
                                }
                                div {
                                    class: "side-config-value",
                                    div {
                                        class: "init-actions",
                                        if let Some(edge) = connected_edge.clone() {
                                            button {
                                                class: "side-config-button is-connected",
                                                r#type: "button",
                                                title: "Disconnect edge",
                                                onclick: move |_| {
                                                    let state = state;
                                                    let edge_id = edge.id.clone();
                                                    spawn(async move {
                                                        disconnect_edge(state, edge_id).await;
                                                    });
                                                },
                                            "Disconnect"
                                            }
                                        }
                                    }
                                    div {
                                        class: "compact-segmented-control",
                                        button {
                                            key: "param-side-{param.id}-left",
                                            class: "side-config-button",
                                            r#type: "button",
                                            title: "left",
                                            onclick: move |_| {
                                                let state = state;
                                                let param_id = left_param_id.clone();
                                                spawn(async move {
                                                    set_param_side(state, position, param_id, "left".to_string()).await;
                                                });
                                            },
                                            "Left"
                                        }
                                        button {
                                            key: "param-side-{param.id}-top",
                                            class: "side-config-button",
                                            r#type: "button",
                                            title: "top",
                                            onclick: move |_| {
                                                let state = state;
                                                let param_id = top_param_id.clone();
                                                spawn(async move {
                                                    set_param_side(state, position, param_id, "top".to_string()).await;
                                                });
                                            },
                                            "Top"
                                        }
                                        button {
                                            key: "param-side-{param.id}-right",
                                            class: "side-config-button",
                                            r#type: "button",
                                            title: "right",
                                            onclick: move |_| {
                                                let state = state;
                                                let param_id = right_param_id.clone();
                                                spawn(async move {
                                                    set_param_side(state, position, param_id, "right".to_string()).await;
                                                });
                                            },
                                            "Right"
                                        }
                                        button {
                                            key: "param-side-{param.id}-bottom",
                                            class: "side-config-button",
                                            r#type: "button",
                                            title: "bottom",
                                            onclick: move |_| {
                                                let state = state;
                                                let param_id = bottom_param_id.clone();
                                                spawn(async move {
                                                    set_param_side(state, position, param_id, "bottom".to_string()).await;
                                                });
                                            },
                                            "Bottom"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div {
                class: "side-config-group side-config-group-secondary",
                button {
                    id: "inspector-advanced-toggle",
                    class: "panel-toggle-button",
                    r#type: "button",
                    onclick: move |_| {
                        let next = !state.read().inspector.show_advanced;
                        state.write().inspector.show_advanced = next;
                    },
                    if tile_panel_advanced_open {
                        "Hide advanced details"
                    } else {
                        "Show advanced details"
                    }
                }
                if tile_panel_advanced_open {
                    div {
                        class: "side-config-value",
                        p { class: "side-config-help", "Tile position: [{node.position.col}, {node.position.row}]" }
                        p { class: "side-config-help", "Audio: {format_runtime_boot_label(&snapshot.runtime_boot)}" }
                        p { class: "side-config-help", "Playback: {format_runtime_playback_label(snapshot)}" }
                        p {
                            id: "runtime-sample-readiness",
                            class: "side-config-help",
                            "Samples ready: {snapshot.sample_readiness.loaded}/{snapshot.sample_readiness.attempted}"
                        }
                        p {
                            id: "runtime-sample-cache",
                            class: "side-config-help",
                            "Sample cache: {snapshot.sample_cache.hits} hits, {snapshot.sample_cache.misses} misses, {snapshot.sample_cache.writes} writes"
                        }
                        if let Some(error) = runtime_error {
                            p {
                                id: "runtime-error-detail",
                                class: "side-config-port-note",
                                "Latest runtime message: {error}"
                            }
                        }
                    }
                }
            }
        } else {
            div {
                class: "side-config-group",
                p { class: "side-config-empty", "{tile_help}" }
                p {
                    class: "side-config-help",
                    "Start by choosing a tile from the library, then click a tile on the canvas to inspect it here."
                }
                if let Some(error) = runtime_error {
                    p {
                        id: "runtime-error-detail",
                        class: "side-config-port-note",
                        "{error}"
                    }
                }
            }
        }
    }
}

// Inspector-specific async actions
// ---------------------------------------------------------------------------

pub(crate) fn short_side_label(side: &str) -> &'static str {
    match side {
        "left" => "Left",
        "top" => "Top",
        "bottom" => "Bottom",
        _ => "Right",
    }
}

pub(crate) fn format_inline_param_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "<unprintable>".to_string()),
    }
}

async fn commit_node_label(state: Signal<EditorShellState>, position: GridPos) {
    let label = state.read().inspector.label_input.trim().to_string();
    let next_selected = state.read().selected_node;
    apply_graph_ops(
        state,
        "native_set_label",
        vec![GraphOp::NodeSetLabel {
            position,
            label: (!label.is_empty()).then_some(label),
        }],
        next_selected,
    )
    .await;
}

async fn apply_repair_suggestion(state: Signal<EditorShellState>, suggestion: RepairSuggestion) {
    match suggestion {
        RepairSuggestion::MoveNode { node, to } => move_node(state, node, to).await,
        RepairSuggestion::SetOutputSide { position, side } => {
            set_output_side(state, position, tile_side_to_string(side).to_string()).await;
        }
        RepairSuggestion::SetParamSide {
            position,
            param_id,
            side,
        } => {
            set_param_side(
                state,
                position,
                param_id,
                tile_side_to_string(side).to_string(),
            )
            .await;
        }
        RepairSuggestion::DisconnectEdge { edge_id } => disconnect_edge(state, edge_id.0).await,
    }
}

fn format_probe_reason(reason: Option<&EdgeConnectProbeReason>) -> String {
    match reason {
        Some(EdgeConnectProbeReason::UnsupportedDomain) => "Unsupported domain crossing".into(),
        Some(EdgeConnectProbeReason::TypeMismatch) => "Type mismatch".into(),
        Some(EdgeConnectProbeReason::TargetParamOccupied) => "Target param already occupied".into(),
        Some(EdgeConnectProbeReason::NoCompatibleParam) => "No compatible target param".into(),
        Some(EdgeConnectProbeReason::NotAdjacent) => "Tiles are not adjacent".into(),
        Some(EdgeConnectProbeReason::SideMismatch) => "Tile sides do not face each other".into(),
        Some(other) => format!("{other:?}"),
        None => "Connection rejected".to_string(),
    }
}

fn format_repair_suggestion(suggestion: &RepairSuggestion) -> String {
    match suggestion {
        RepairSuggestion::MoveNode { node, to } => format!(
            "Move node [{}, {}] to [{}, {}]",
            node.col, node.row, to.col, to.row
        ),
        RepairSuggestion::SetOutputSide { position, side } => format!(
            "Set output side for [{}, {}] to {}",
            position.col,
            position.row,
            tile_side_to_string(*side)
        ),
        RepairSuggestion::SetParamSide {
            position,
            param_id,
            side,
        } => format!(
            "Set param {param_id} on [{}, {}] to {}",
            position.col,
            position.row,
            tile_side_to_string(*side)
        ),
        RepairSuggestion::DisconnectEdge { edge_id } => {
            format!("Disconnect edge {}", edge_id.0)
        }
    }
}
