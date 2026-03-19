use super::*;

pub(super) fn filtered_catalog(state: &EditorShellState) -> Vec<PieceDef> {
    let query = state.picker_query.trim().to_ascii_lowercase();
    state
        .catalog
        .iter()
        .filter(|piece| {
            if query.is_empty() {
                return true;
            }
            piece.id.to_ascii_lowercase().contains(&query)
                || piece.label.to_ascii_lowercase().contains(&query)
                || piece.category.to_ascii_lowercase().contains(&query)
                || piece
                    .description
                    .as_ref()
                    .map(|value| value.to_ascii_lowercase().contains(&query))
                    .unwrap_or(false)
        })
        .cloned()
        .collect()
}

pub(super) fn compact_piece_label(
    piece: Option<&PieceDef>,
    node: Option<&GraphNodeView>,
) -> String {
    match (piece, node) {
        (Some(piece), Some(node)) => node
            .label
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| piece.label.clone()),
        (Some(piece), None) => piece.label.clone(),
        (None, Some(node)) => node.piece_id.clone(),
        (None, None) => "None".to_string(),
    }
}

pub(super) fn compact_project_name(name: &str, dirty: bool) -> String {
    let trimmed = name.trim();
    if dirty {
        format!(
            "{}*",
            if trimmed.is_empty() {
                "Untitled"
            } else {
                trimmed
            }
        )
    } else if trimmed.is_empty() {
        "Untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn side_config_text(state: &EditorShellState) -> String {
    let position = format_selected_position(
        state
            .selected_node
            .as_ref()
            .or(state.selected_cell.as_ref()),
    );
    match (
        selected_node_view(state),
        selected_piece_def(state),
        &state.workspace_mode,
    ) {
        (Some(node), Some(piece), _) => format!(
            "{} at {}",
            compact_piece_label(Some(piece), Some(node)),
            position
        ),
        (Some(node), None, _) => format!("{} at {}", node.piece_id, position),
        (None, _, WorkspaceMode::Init) => {
            "Init-stage configuration and trick controls.".to_string()
        }
        _ => format!("No tile selected at {position}"),
    }
}

pub(super) fn compile_modal_view(state: &EditorShellState) -> CompileModalViewData {
    let diagnostics = state
        .project_preview
        .diagnostics
        .iter()
        .map(|diagnostic| CompileDiagnosticRow {
            severity: format_diagnostic_severity(&diagnostic.severity).to_string(),
            summary: diagnostic_summary(diagnostic),
            location: diagnostic_location(diagnostic),
        })
        .collect::<Vec<_>>();

    let banner_text = if !state.tauri_available {
        Some(BACKEND_REQUIRED_MESSAGE.to_string())
    } else if !state.project_preview.can_render {
        Some("Project compile preview contains blocking issues.".to_string())
    } else if !state.project_preview.can_play {
        Some("Project can render, but runtime playback is not ready yet.".to_string())
    } else {
        None
    };

    CompileModalViewData {
        is_open: state.compile_open,
        banner_text,
        compiled_text: state.project_preview.code.clone().unwrap_or_default(),
        diagnostic_count: diagnostics.len(),
        diagnostics,
        delay_slots: state
            .project_preview
            .compile_meta
            .delay_slots
            .iter()
            .map(format_delay_slot)
            .collect(),
        domain_bridges: state
            .project_preview
            .compile_meta
            .domain_bridges
            .iter()
            .map(format_domain_bridge)
            .collect(),
        activity_events: state
            .project_preview
            .compile_meta
            .activity_events
            .iter()
            .map(format_activity_event)
            .collect(),
        editor_active: state.tauri_available,
    }
}

pub(super) fn inspector_modal_view(state: &EditorShellState) -> InspectorModalViewData {
    let selected_node = selected_node_view(state);
    let selected_piece = selected_piece_def(state);
    let piece_panel = selected_piece_panel(state);
    let tile_label = selected_piece
        .map(|piece| compact_piece_label(Some(piece), selected_node))
        .unwrap_or_else(|| "No Tile Selected".to_string());
    let project_title = compact_project_name(&state.project.name, state.project.dirty);
    let project_meta = format!(
        "N:{} E:{} | {}",
        state.project.node_count,
        state.project.edge_count,
        backend_mode_label(state.tauri_available)
    );
    let selected_node_label = selected_node
        .map(|node| format!("Node: [{}, {}]", node.position.col, node.position.row))
        .unwrap_or_else(|| "Node: none".to_string());
    let selected_role_label = selected_piece
        .map(|piece| {
            if piece.semantic_kind.trim().is_empty() {
                "Role: unknown".to_string()
            } else {
                format!("Role: {}", piece.semantic_kind)
            }
        })
        .unwrap_or_else(|| "Role: unknown".to_string());

    InspectorModalViewData {
        is_open: state.inspector_open,
        project_title,
        project_meta,
        selected_node_label,
        selected_role_label,
        mode_label: state.workspace_mode.label().to_string(),
        tile_preview_label: tile_label,
        tile_preview_sub: selected_piece
            .and_then(|piece| piece.description.clone())
            .unwrap_or_else(|| "Select a tile to inspect its metadata.".to_string()),
        detail_lines: piece_panel.detail_lines,
        param_lines: piece_panel.param_lines,
        editor_active: state.tauri_available,
    }
}

pub(super) fn selected_piece_panel(state: &EditorShellState) -> PieceInfoPanelData {
    let Some(node) = selected_node_view(state) else {
        return PieceInfoPanelData::default();
    };
    let Some(piece) = selected_piece_def(state) else {
        return PieceInfoPanelData {
            detail_lines: vec![format!(
                "Catalog details for {} are unavailable. Reload the editor to restore tile metadata.",
                node.piece_id
            )],
            param_lines: Vec::new(),
        };
    };

    let mut detail_lines = Vec::new();
    if let Some(description) = piece
        .description
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        detail_lines.push(description.clone());
    }
    detail_lines.push(format!(
        "Output: {}",
        piece
            .output_type
            .as_ref()
            .map(format_port_type)
            .unwrap_or_else(|| "terminal".to_string())
    ));
    detail_lines.push(format!(
        "Category: {} | Namespace: {}",
        piece.category, piece.namespace
    ));
    if !piece.semantic_kind.trim().is_empty() {
        detail_lines.push(format!("Role: {}", piece.semantic_kind));
    }
    if let Some(side) = node
        .output_side
        .as_deref()
        .or(piece.output_side.as_deref())
        .map(canonical_side)
    {
        detail_lines.push(format!("Output side: {side}"));
    }
    if let Some(label) = node.label.as_ref().filter(|value| !value.trim().is_empty()) {
        detail_lines.push(format!("Label: {label}"));
    }
    if !piece.tags.is_empty() {
        detail_lines.push(format!("Tags: {}", piece.tags.join(", ")));
    }

    let node_position = node.position;
    let param_lines = if piece.params.is_empty() {
        vec!["No inputs or inline parameters.".to_string()]
    } else {
        piece
            .params
            .iter()
            .map(|param| {
                let side = node
                    .input_sides
                    .get(&param.id)
                    .map(String::as_str)
                    .unwrap_or(param.side.as_str());
                let status = if let Some(value) = node.inline_params.get(&param.id) {
                    format!("inline {}", format_inline_value(value))
                } else if state
                    .graph
                    .edges
                    .iter()
                    .any(|edge| edge.to_node == node_position && edge.to_param == param.id)
                {
                    "connected".to_string()
                } else if param.required {
                    "required".to_string()
                } else {
                    "optional".to_string()
                };

                format!(
                    "{} ({}) | {} | side {} | {}",
                    param.label,
                    param.id,
                    format_param_schema(&param.schema),
                    canonical_side(side),
                    status
                )
            })
            .collect()
    };

    PieceInfoPanelData {
        detail_lines,
        param_lines,
    }
}

pub(super) fn selected_node_view(state: &EditorShellState) -> Option<&GraphNodeView> {
    let selected = state.selected_node.as_ref()?;
    state
        .graph
        .nodes
        .iter()
        .find(|node| &node.position == selected)
}

pub(super) fn selected_piece_def(state: &EditorShellState) -> Option<&PieceDef> {
    let node = selected_node_view(state)?;
    piece_def_for_id(&state.catalog, &node.piece_id)
}

pub(super) fn piece_def_for_id<'a>(
    catalog: &'a [PieceDef],
    piece_id: &str,
) -> Option<&'a PieceDef> {
    catalog.iter().find(|piece| piece.id == piece_id)
}

pub(super) fn piece_category_class(category: &str) -> &'static str {
    match category {
        "generator" => "piece-generator",
        "transform" => "piece-transform",
        "constant" => "piece-constant",
        "output" => "piece-output",
        "control" => "piece-control",
        "trick" => "piece-trick",
        "connector" => "piece-connector",
        _ => "piece-generator",
    }
}

pub(super) fn format_port_type(port_type: &PortType) -> String {
    match port_type.domain() {
        Some("control") | None => port_type.kind().to_string(),
        Some(domain) => format!("{} ({domain})", port_type.kind()),
    }
}

pub(super) fn format_param_schema(schema: &ParamSchema) -> String {
    match schema {
        ParamSchema::Number { .. } => "number".to_string(),
        ParamSchema::Text { .. } => "text".to_string(),
        ParamSchema::Enum { options, .. } => format!("enum {} opts", options.len()),
        ParamSchema::Bool { .. } => "bool".to_string(),
        ParamSchema::Custom { port_type, .. } => format!("port {}", format_port_type(port_type)),
    }
}

pub(super) fn format_inline_value(value: &Value) -> String {
    let rendered = serde_json::to_string(value).unwrap_or_else(|_| "<invalid json>".to_string());
    if rendered.len() > 40 {
        format!("{}...", &rendered[..37])
    } else {
        rendered
    }
}

pub(super) fn format_selected_position(position: Option<&GridPos>) -> String {
    position
        .map(|pos| format!("[{}, {}]", pos.col, pos.row))
        .unwrap_or_else(|| "[-, -]".to_string())
}

pub(super) fn sync_editor_inputs(state: &mut EditorShellState) {
    let Some(node) = selected_node_view(state).cloned() else {
        state.label_input.clear();
        state.param_inputs.clear();
        return;
    };
    state.label_input = node.label.clone().unwrap_or_default();
    state.param_inputs = selected_piece_def(state)
        .map(|piece| {
            piece
                .params
                .iter()
                .map(|param| (param.id.clone(), param_input_value(&node, param)))
                .collect()
        })
        .unwrap_or_default();
}

pub(super) fn param_input_value(node: &GraphNodeView, param: &ParamDef) -> String {
    node.inline_params
        .get(&param.id)
        .map(|value| match value {
            Value::String(text) => text.clone(),
            Value::Bool(flag) => flag.to_string(),
            Value::Number(number) => number.to_string(),
            other => serde_json::to_string(other).unwrap_or_default(),
        })
        .unwrap_or_else(|| match &param.schema {
            ParamSchema::Number { default, .. } => default.to_string(),
            ParamSchema::Text { default, .. } => default.clone(),
            ParamSchema::Enum { default, .. } => default.clone(),
            ParamSchema::Bool { default, .. } => default.to_string(),
            ParamSchema::Custom { default, .. } => default
                .as_ref()
                .map(|value| match value {
                    Value::String(text) => text.clone(),
                    Value::Bool(flag) => flag.to_string(),
                    Value::Number(number) => number.to_string(),
                    other => serde_json::to_string(other).unwrap_or_default(),
                })
                .unwrap_or_default(),
        })
}

pub(super) fn backend_mode_label(tauri_available: bool) -> &'static str {
    if tauri_available {
        "LIVE BACKEND"
    } else {
        "BACKEND REQUIRED"
    }
}

pub(super) fn backend_mode_detail(tauri_available: bool) -> &'static str {
    if tauri_available {
        "Connected to the desktop backend. Edits and playback use live project data."
    } else {
        BACKEND_REQUIRED_MESSAGE
    }
}

pub(super) fn backend_mode_state_class(tauri_available: bool) -> &'static str {
    if tauri_available {
        "is-live"
    } else {
        "is-unavailable"
    }
}

pub(super) fn format_runtime_boot_label(status: &runtime::RuntimeBootStatus) -> String {
    match status.phase {
        runtime::RuntimeBootPhase::Idle => "H:IDLE".to_string(),
        runtime::RuntimeBootPhase::Booting => "H:BOOT".to_string(),
        runtime::RuntimeBootPhase::Ready => "H:RDY".to_string(),
        runtime::RuntimeBootPhase::Error => format!(
            "H:ERR {}",
            status
                .detail
                .clone()
                .unwrap_or_else(|| "runtime".to_string())
        ),
    }
}

pub(super) fn format_runtime_playback_label(state: &EditorShellState) -> String {
    if state.runtime_status.playing {
        format!("PLY {:04}ms", state.runtime_status.play_elapsed_ms)
    } else if state.runtime_status.has_program {
        "RDY".to_string()
    } else if let Some(error) = state.runtime_status.last_error.as_ref() {
        format!("ERR {error}")
    } else {
        "IDLE".to_string()
    }
}

pub(super) fn render_console_output(state: &EditorShellState) -> String {
    let mut lines = Vec::new();
    if let Some(message) = state.status_message.as_ref() {
        lines.push(message.clone());
    }
    for diagnostic in &state.diagnostics.entries {
        lines.push(format!("[{}] {}", diagnostic.kind, diagnostic.message));
    }
    if lines.is_empty() {
        "Console idle.".to_string()
    } else {
        lines.join("\n")
    }
}

pub(super) fn parse_recovery_path(message: &str) -> Option<String> {
    message
        .split_once("wrote recovery snapshot to ")
        .map(|(_, path)| path.trim().to_string())
}

fn format_diagnostic_severity(severity: &DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Info => "info",
    }
}

fn diagnostic_location(diagnostic: &DiagnosticDto) -> String {
    diagnostic
        .site
        .as_ref()
        .map(|location| format!("[{}, {}]", location.col, location.row))
        .or_else(|| {
            diagnostic
                .edge_id
                .as_ref()
                .map(|edge_id| format!("edge {}", edge_id.0))
        })
        .unwrap_or_else(|| "global".to_string())
}

fn diagnostic_summary(diagnostic: &DiagnosticDto) -> String {
    match &diagnostic.kind {
        DiagnosticKind::UnknownPiece { piece_id } => format!("Unknown piece {piece_id}"),
        DiagnosticKind::UnknownNode { pos } => {
            format!("Unknown node at [{}, {}]", pos.col, pos.row)
        }
        DiagnosticKind::UnknownParam { piece_id, param } => {
            format!("Unknown param {param} on {piece_id}")
        }
        DiagnosticKind::InvalidOperation { reason } => reason.clone(),
        DiagnosticKind::DuplicateConnection { to_node, to_param } => {
            format!(
                "Duplicate connection into {} at [{}, {}]",
                to_param, to_node.col, to_node.row
            )
        }
        DiagnosticKind::Cycle { involved } => format!("Cycle involving {} tiles", involved.len()),
        DiagnosticKind::NoTerminalNode => "No terminal node".to_string(),
        DiagnosticKind::MultipleTerminalNodes { positions } => {
            format!("Multiple terminal nodes ({})", positions.len())
        }
        DiagnosticKind::UnreachableNode { position } => {
            format!("Unreachable node at [{}, {}]", position.col, position.row)
        }
        DiagnosticKind::TypeMismatch {
            expected,
            got,
            param,
        } => {
            format!(
                "Type mismatch on {param}: expected {}, got {}",
                format_port_type(expected),
                format_port_type(got)
            )
        }
        DiagnosticKind::UnsupportedDomainCrossing {
            expected,
            got,
            param,
        } => {
            format!(
                "Unsupported domain crossing on {param}: expected {}, got {}",
                format_port_type(expected),
                format_port_type(got)
            )
        }
        DiagnosticKind::DelayTypeMismatch { default, feedback } => {
            format!(
                "Delay mismatch: default {}, feedback {}",
                format_port_type(default),
                format_port_type(feedback)
            )
        }
        DiagnosticKind::SideMismatch {
            from_pos,
            to_pos,
            expected_side,
        } => format!(
            "Side mismatch from [{}, {}] to [{}, {}] (expected {})",
            from_pos.col,
            from_pos.row,
            to_pos.col,
            to_pos.row,
            tile_side_to_string(*expected_side)
        ),
        DiagnosticKind::NotAdjacent { from_pos, to_pos } => format!(
            "Nodes are not adjacent: [{}, {}] -> [{}, {}]",
            from_pos.col, from_pos.row, to_pos.col, to_pos.row
        ),
        DiagnosticKind::OutputFromTerminal { position } => {
            format!(
                "Terminal at [{}, {}] cannot output",
                position.col, position.row
            )
        }
        DiagnosticKind::MissingRequiredParam { param } => format!("Missing required param {param}"),
        DiagnosticKind::InlineNotAllowed { param } => {
            format!("Inline value not allowed for {param}")
        }
        DiagnosticKind::InlineTypeMismatch {
            param, expected, ..
        } => {
            format!(
                "Inline value for {param} must be {}",
                format_port_type(expected)
            )
        }
    }
}

fn format_delay_slot(slot: &tauri::DelaySlotDto) -> String {
    let default_expr =
        serde_json::to_string(&slot.default_expr).unwrap_or_else(|_| "<invalid>".to_string());
    let port_type = slot
        .port_type
        .as_ref()
        .map(|value| format!(" ({})", format_port_type(value)))
        .unwrap_or_default();
    format!(
        "{} @ [{}, {}] = {}{}",
        slot.slot, slot.node.col, slot.node.row, default_expr, port_type
    )
}

fn format_domain_bridge(bridge: &DomainBridgeDto) -> String {
    let kind = match bridge.kind {
        tauri::DomainBridgeKind::ControlToAudio => "control->audio",
        tauri::DomainBridgeKind::AudioToControl => "audio->control",
        tauri::DomainBridgeKind::EventToControl => "event->control",
    };
    format!(
        "[{}, {}] -> [{}, {}] {} ({kind})",
        bridge.source_pos.col,
        bridge.source_pos.row,
        bridge.target_pos.col,
        bridge.target_pos.row,
        bridge.param
    )
}

fn format_activity_event(event: &tauri::ActivityEventDto) -> String {
    let kind = match &event.kind {
        tauri::ActivityKindDto::Trigger { label, intensity } => label
            .as_ref()
            .map(|label| format!("trigger {label} ({intensity:.2})"))
            .unwrap_or_else(|| format!("trigger ({intensity:.2})")),
        tauri::ActivityKindDto::Sustain { progress } => format!("sustain ({progress:.2})"),
        tauri::ActivityKindDto::Processing { label } => format!("processing {label}"),
        tauri::ActivityKindDto::RuntimeError { message } => format!("runtime error {message}"),
    };
    let param = event
        .param
        .as_ref()
        .map(|param| format!(" param {param}"))
        .unwrap_or_default();
    let at = event
        .at
        .as_ref()
        .map(|value| {
            let rendered = serde_json::to_string(value).unwrap_or_else(|_| "<invalid>".to_string());
            format!(" at {rendered}")
        })
        .unwrap_or_default();
    format!(
        "{kind} [{}, {}]{}{}",
        event.site.col, event.site.row, param, at
    )
}
