use dioxus::prelude::*;
use serde_json::json;

use super::*;

// ---------------------------------------------------------------------------
// Side-config panel (primary export)
// ---------------------------------------------------------------------------

pub(super) fn side_config_content(
    mut state: Signal<EditorShellState>,
    snapshot: &EditorShellState,
) -> Element {
    let runtime_error = snapshot
        .runtime_status
        .last_error
        .clone()
        .or_else(|| snapshot.status_message.clone());
    let probe = snapshot.pending_probe.clone();
    let selected_node = selected_node_view(snapshot).cloned();
    let selected_piece = selected_piece_def(snapshot).cloned();
    let node_help = side_config_text(snapshot);

    rsx! {
        div {
            class: "side-config-group",
            p { class: "side-config-section-title", "Runtime Insight" }
            p { class: "side-config-help", "{format_runtime_boot_label(&snapshot.runtime_boot)}" }
            p {
                class: "side-config-help",
                "Samples: {snapshot.sample_readiness.loaded}/{snapshot.sample_readiness.attempted} ready"
            }
            p {
                class: "side-config-help",
                "Cache: {snapshot.sample_cache.hits} hits / {snapshot.sample_cache.misses} misses / {snapshot.sample_cache.writes} writes"
            }
            p {
                class: "side-config-help",
                "Playback: {format_runtime_playback_label(snapshot)}"
            }
            if let Some(error) = runtime_error {
                p { class: "side-config-port-note", "Last runtime error: {error}" }
            }
        }

        if let Some(probe) = probe {
            div {
                class: "side-config-group",
                p { class: "side-config-section-title", "Wiring Feedback" }
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
                                "{repair_suggestion_short_label(&suggestion)}"
                            }
                        }
                    }
                }
            }
        }

        if let (Some(node), Some(piece)) = (selected_node, selected_piece) {
            div {
                class: "side-config-group",
                p { class: "side-config-section-title", "Tile" }
                p { class: "side-config-help", "{node_help}" }
                div {
                    class: "side-config-value",
                    label {
                        class: "side-config-field",
                        span { class: "side-config-label", "Label" }
                        input {
                            r#type: "text",
                            value: snapshot.label_input.clone(),
                            placeholder: piece.label.clone(),
                            oninput: move |event| {
                                state.write().label_input = event.value();
                            },
                            onblur: move |_| {
                                let state = state;
                                let position = node.position;
                                spawn(async move {
                                    commit_node_label(state, position).await;
                                });
                            }
                        }
                    }
                }
            }

            if piece.output_type.is_some() {
                div {
                    class: "side-config-group",
                    p { class: "side-config-section-title", "Output Side" }
                    div {
                        class: "init-actions",
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

            for param in piece.params {
                {
                    let position = node.position;
                    let clear_param_id = param.id.clone();
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
                    let input_value = snapshot
                        .param_inputs
                        .get(&param.id)
                        .cloned()
                        .unwrap_or_else(|| param_input_value(&node, &param));
                    let active_side = node
                        .input_sides
                        .get(&param.id)
                        .cloned()
                        .unwrap_or_else(|| canonical_side(param.side.as_str()).to_string());
                    rsx! {
                        div {
                            key: "param-{param.id}",
                            class: "side-config-group",
                            p {
                                class: "side-config-section-title",
                                "{param.label} ({param.id})"
                            }
                            p {
                                class: "side-config-help",
                                "{format_param_schema(&param.schema)} | side {active_side}"
                            }
                            div {
                                class: "side-config-value",
                                {param_input_control(state, position, &param, input_value)}
                                div {
                                    class: "init-actions",
                                    button {
                                        class: "side-config-button",
                                        r#type: "button",
                                        title: "Apply inline value",
                                        onclick: move |_| {
                                            let state = state;
                                            let param = param.clone();
                                            spawn(async move {
                                                commit_param_inline(state, position, param).await;
                                            });
                                        },
                                        "A"
                                    }
                                    button {
                                        class: "side-config-button is-none",
                                        r#type: "button",
                                        title: "Clear inline value",
                                        onclick: move |_| {
                                            let state = state;
                                            let param_id = clear_param_id.clone();
                                            spawn(async move {
                                                clear_param_inline(state, position, param_id).await;
                                            });
                                        },
                                        "X"
                                    }
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
                                            "D"
                                        }
                                    }
                                }
                                div {
                                    class: "init-actions",
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
                                        "L"
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
                                        "T"
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
                                        "R"
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
                                        "B"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        } else {
            p { class: "side-config-empty", "{node_help}" }
        }
    }
}

// ---------------------------------------------------------------------------
// Param input widget
// ---------------------------------------------------------------------------

fn param_input_control(
    mut state: Signal<EditorShellState>,
    position: GridPos,
    param: &ParamDef,
    input_value: String,
) -> Element {
    let param_id = param.id.clone();
    let param = param.clone();
    match &param.schema {
        ParamSchema::Bool { .. } => rsx! {
            label {
                class: "side-config-field",
                span { class: "side-config-label", "Inline value" }
                select {
                    value: input_value,
                    onchange: move |event| {
                        state
                            .write()
                            .param_inputs
                            .insert(param_id.clone(), event.value());
                    },
                    onblur: move |_| {
                        let state = state;
                        let param = param.clone();
                        spawn(async move {
                            commit_param_inline(state, position, param).await;
                        });
                    },
                    option { value: "false", "false" }
                    option { value: "true", "true" }
                }
            }
        },
        ParamSchema::Enum { options, .. } => rsx! {
            label {
                class: "side-config-field",
                span { class: "side-config-label", "Inline value" }
                select {
                    value: input_value.clone(),
                    onchange: move |event| {
                        state
                            .write()
                            .param_inputs
                            .insert(param_id.clone(), event.value());
                    },
                    onblur: move |_| {
                        let state = state;
                        let param = param.clone();
                        spawn(async move {
                            commit_param_inline(state, position, param).await;
                        });
                    },
                    for option_value in options {
                        option {
                            key: "{option_value}",
                            value: "{option_value}",
                            "{option_value}"
                        }
                    }
                }
            }
        },
        ParamSchema::Custom {
            value_kind: tauri::ParamValueKind::Json,
            ..
        } => rsx! {
            label {
                class: "side-config-field",
                span { class: "side-config-label", "Inline JSON" }
                textarea {
                    value: input_value,
                    oninput: move |event| {
                        state
                            .write()
                            .param_inputs
                            .insert(param_id.clone(), event.value());
                    },
                    onblur: move |_| {
                        let state = state;
                        let param = param.clone();
                        spawn(async move {
                            commit_param_inline(state, position, param).await;
                        });
                    }
                }
            }
        },
        _ => rsx! {
            label {
                class: "side-config-field",
                span { class: "side-config-label", "Inline value" }
                input {
                    r#type: input_type_for_param(&param),
                    value: input_value,
                    oninput: move |event| {
                        state
                            .write()
                            .param_inputs
                            .insert(param_id.clone(), event.value());
                    },
                    onblur: move |_| {
                        let state = state;
                        let param = param.clone();
                        spawn(async move {
                            commit_param_inline(state, position, param).await;
                        });
                    }
                }
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Inspector-specific async actions
// ---------------------------------------------------------------------------

async fn commit_node_label(state: Signal<EditorShellState>, position: GridPos) {
    let label = state.read().label_input.trim().to_string();
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

async fn commit_param_inline(
    mut state: Signal<EditorShellState>,
    position: GridPos,
    param: ParamDef,
) {
    let raw = state
        .read()
        .param_inputs
        .get(&param.id)
        .cloned()
        .unwrap_or_default();
    match parse_param_input(&param, raw.as_str()) {
        Ok(Some(value)) => {
            let next_selected = state.read().selected_node;
            apply_graph_ops(
                state,
                "native_param_inline",
                vec![GraphOp::ParamSetInline {
                    position,
                    param_id: param.id,
                    value,
                }],
                next_selected,
            )
            .await;
        }
        Ok(None) => {
            clear_param_inline(state, position, param.id).await;
        }
        Err(error) => {
            state.write().status_message = Some(error);
        }
    }
}

async fn clear_param_inline(state: Signal<EditorShellState>, position: GridPos, param_id: String) {
    let next_selected = state.read().selected_node;
    apply_graph_ops(
        state,
        "native_param_clear_inline",
        vec![GraphOp::ParamClearInline { position, param_id }],
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

// ---------------------------------------------------------------------------
// Inspector-specific formatting helpers
// ---------------------------------------------------------------------------

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

fn repair_suggestion_short_label(suggestion: &RepairSuggestion) -> &'static str {
    match suggestion {
        RepairSuggestion::MoveNode { .. } => "M",
        RepairSuggestion::SetOutputSide { .. } => "O",
        RepairSuggestion::SetParamSide { .. } => "P",
        RepairSuggestion::DisconnectEdge { .. } => "X",
    }
}

fn input_type_for_param(param: &ParamDef) -> &'static str {
    match &param.schema {
        ParamSchema::Number { .. } => "number",
        ParamSchema::Custom {
            value_kind: tauri::ParamValueKind::Number,
            ..
        } => "number",
        _ => "text",
    }
}

fn parse_param_input(param: &ParamDef, raw: &str) -> Result<Option<Value>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match &param.schema {
        ParamSchema::Number { .. } => trimmed
            .parse::<f64>()
            .map(|value| Some(json!(value)))
            .map_err(|error| format!("{} expects a number: {error}", param.label)),
        ParamSchema::Text { .. } | ParamSchema::Enum { .. } => {
            Ok(Some(Value::String(trimmed.to_string())))
        }
        ParamSchema::Bool { .. } => match trimmed {
            "true" => Ok(Some(Value::Bool(true))),
            "false" => Ok(Some(Value::Bool(false))),
            _ => Err(format!("{} expects true or false.", param.label)),
        },
        ParamSchema::Custom { value_kind, .. } => match value_kind {
            tauri::ParamValueKind::Number => trimmed
                .parse::<f64>()
                .map(|value| Some(json!(value)))
                .map_err(|error| format!("{} expects a number: {error}", param.label)),
            tauri::ParamValueKind::Text => Ok(Some(Value::String(trimmed.to_string()))),
            tauri::ParamValueKind::Bool => match trimmed {
                "true" => Ok(Some(Value::Bool(true))),
                "false" => Ok(Some(Value::Bool(false))),
                _ => Err(format!("{} expects true or false.", param.label)),
            },
            tauri::ParamValueKind::Json => serde_json::from_str::<Value>(trimmed)
                .map(Some)
                .map_err(|error| format!("{} expects valid JSON: {error}", param.label)),
            tauri::ParamValueKind::None => Ok(Some(Value::Null)),
        },
    }
}

fn short_side_label(side: &str) -> &'static str {
    match side {
        "left" => "L",
        "top" => "T",
        "bottom" => "B",
        _ => "R",
    }
}
