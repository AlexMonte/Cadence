use std::collections::BTreeMap;

use crate::adapter::{
    DiagnosticDto, DiagnosticKind, DiagnosticSeverity, DomainBridgeDto, GridPos, RuntimeStatusDto,
    backend,
};
use super::{BACKEND_REQUIRED_MESSAGE, EditorShellState, tile_side_to_string};
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct TileNotationCueView {
    pub(crate) text: String,
    pub(crate) title: String,
    pub(crate) badge: Option<String>,
    pub(crate) active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompileDiagnosticRow {
    pub severity: String,
    pub location: String,
    pub summary: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompileTimelineEventView {
    pub key: String,
    pub label: String,
    pub title: String,
    pub left_pct: f64,
    pub width_pct: f64,
    pub active: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompileTimelineRowView {
    pub key: String,
    pub label: String,
    pub is_pitch_row: bool,
    pub events: Vec<CompileTimelineEventView>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompileTimelineLaneView {
    pub key: String,
    pub label: String,
    pub subtitle: String,
    pub rows: Vec<CompileTimelineRowView>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompileModalViewData {
    pub is_open: bool,
    pub banner_text: Option<String>,
    pub debug_text: String,
    pub diagnostic_count: usize,
    pub diagnostics: Vec<CompileDiagnosticRow>,
    pub delay_edges: Vec<String>,
    pub domain_bridges: Vec<String>,
    pub timeline_lanes: Vec<CompileTimelineLaneView>,
    pub playhead_pct: f64,
    pub playhead_label: String,
    pub preview_event_count: usize,
    pub editor_active: bool,
}

pub(crate) fn compile_modal_view(state: &EditorShellState) -> CompileModalViewData {
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

    let has_error_diagnostics = state
        .project_preview
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error);

    let banner_text = if !state.backend_available {
        Some(BACKEND_REQUIRED_MESSAGE.to_string())
    } else if !state.project_preview.can_render || has_error_diagnostics {
        Some("Playback preview found blocking issues in the current song.".to_string())
    } else if !state.project_preview.can_play {
        Some("The song preview is ready, but playback is still warming up.".to_string())
    } else {
        None
    };
    let playhead_pct = preview_playhead_normalized(&state.runtime_status);

    CompileModalViewData {
        is_open: state.compile_open,
        banner_text,
        debug_text: state
            .project_preview
            .preview
            .debug_text
            .clone()
            .unwrap_or_default(),
        diagnostic_count: diagnostics.len(),
        diagnostics,
        delay_edges: state
            .project_preview
            .preview
            .delay_edges
            .iter()
            .map(format_delay_edge)
            .collect(),
        domain_bridges: state
            .project_preview
            .preview
            .domain_bridges
            .iter()
            .map(format_domain_bridge)
            .collect(),
        timeline_lanes: build_timeline_lanes(
            state.project_preview.preview.output_lanes.as_slice(),
            state.project_preview.preview.preview_events.as_slice(),
            playhead_pct,
        ),
        playhead_pct,
        playhead_label: format_playhead_label(&state.runtime_status),
        preview_event_count: state.project_preview.preview.preview_events.len(),
        editor_active: state.backend_available,
    }
}

pub(crate) fn tile_notation_cues(
    state: &EditorShellState,
) -> BTreeMap<GridPos, TileNotationCueView> {
    let playhead = preview_playhead_normalized(&state.runtime_status);
    let mut grouped = BTreeMap::<GridPos, Vec<&backend::PreviewEventDto>>::new();
    for event in &state.project_preview.preview.preview_events {
        if let Some(site) = event.site {
            grouped.entry(site).or_default().push(event);
        }
    }

    grouped
        .into_iter()
        .map(|(site, mut events)| {
            events.sort_by(|left, right| {
                left.start_normalized
                    .partial_cmp(&right.start_normalized)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        left.end_normalized
                            .partial_cmp(&right.end_normalized)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| left.label.cmp(&right.label))
            });

            let active_labels = unique_preview_labels(
                events
                    .iter()
                    .copied()
                    .filter(|event| preview_event_is_active(event, playhead)),
            );
            let all_labels = unique_preview_labels(events.iter().copied());
            let active = !active_labels.is_empty();
            let visible_labels = if active { &active_labels } else { &all_labels };
            let event_count = events.len();
            let text = preview_label_excerpt(visible_labels.as_slice());
            let title_labels = if active { &active_labels } else { &all_labels };
            let title = if title_labels.is_empty() {
                format!(
                    "{} preview event(s) at [{}, {}]",
                    event_count, site.col, site.row
                )
            } else if active {
                format!(
                    "{} preview event(s) at [{}, {}] | live: {}",
                    event_count,
                    site.col,
                    site.row,
                    title_labels.join(" | ")
                )
            } else {
                format!(
                    "{} preview event(s) at [{}, {}] | {}",
                    event_count,
                    site.col,
                    site.row,
                    title_labels.join(" | ")
                )
            };

            (
                site,
                TileNotationCueView {
                    text,
                    title,
                    badge: (event_count > 1).then(|| event_count.to_string()),
                    active,
                },
            )
        })
        .collect()
}

pub(crate) fn format_diagnostic_severity(severity: &DiagnosticSeverity) -> &'static str {
    match severity {
        DiagnosticSeverity::Error => "error",
        DiagnosticSeverity::Warning => "warning",
        DiagnosticSeverity::Info => "info",
    }
}

pub(crate) fn diagnostic_location(diagnostic: &DiagnosticDto) -> String {
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

pub(crate) fn diagnostic_summary(diagnostic: &DiagnosticDto) -> String {
    match &diagnostic.kind {
        DiagnosticKind::PieceSemantic {
            piece_id, message, ..
        } => {
            format!("{piece_id}: {message}")
        }
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
        DiagnosticKind::DuplicateInputSide { side, params } => {
            format!(
                "Duplicate {}-side inputs for {}",
                tile_side_to_string(*side),
                params.join(", ")
            )
        }
        DiagnosticKind::Cycle { involved } => format!("Cycle involving {} tiles", involved.len()),
        DiagnosticKind::NoOutputNode => "No output node".to_string(),
        DiagnosticKind::UnreachableNode { position } => {
            format!("Unreachable node at [{}, {}]", position.col, position.row)
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
        DiagnosticKind::InlineTypeMismatch { param, got_value } => {
            format!("Inline value for {param} is invalid: {got_value}")
        }
        DiagnosticKind::RoleMismatch {
            param,
            target_role,
            source_role,
        } => format!("Role mismatch on {param}: expected {target_role}, got {source_role}"),
    }
}

pub(crate) fn format_delay_edge(edge_id: &backend::EdgeId) -> String {
    format!("edge {}", edge_id.0)
}

pub(crate) fn format_domain_bridge(bridge: &DomainBridgeDto) -> String {
    let kind = match bridge.kind {
        backend::DomainBridgeKind::ControlToAudio => "control->audio",
        backend::DomainBridgeKind::AudioToControl => "audio->control",
        backend::DomainBridgeKind::EventToControl => "event->control",
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

pub(crate) fn build_timeline_lanes(
    output_lanes: &[GridPos],
    preview_events: &[backend::PreviewEventDto],
    playhead_pct: f64,
) -> Vec<CompileTimelineLaneView> {
    let mut grouped = BTreeMap::<GridPos, Vec<&backend::PreviewEventDto>>::new();
    for output_lane in output_lanes {
        grouped.entry(*output_lane).or_default();
    }
    for event in preview_events {
        grouped.entry(event.output_lane).or_default().push(event);
    }

    grouped
        .into_iter()
        .map(|(lane_pos, mut lane_events)| {
            lane_events.sort_by(|left, right| {
                left.start_normalized
                    .partial_cmp(&right.start_normalized)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        left.end_normalized
                            .partial_cmp(&right.end_normalized)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .then_with(|| left.label.cmp(&right.label))
            });

            let mut pitch_rows = BTreeMap::<String, Vec<&backend::PreviewEventDto>>::new();
            let mut clip_row = Vec::new();
            for event in lane_events {
                if let Some(pitch) = event
                    .pitch
                    .as_ref()
                    .filter(|pitch| !pitch.trim().is_empty())
                {
                    pitch_rows.entry(pitch.clone()).or_default().push(event);
                } else {
                    clip_row.push(event);
                }
            }

            let mut rows = pitch_rows
                .into_iter()
                .map(|(pitch, row_events)| CompileTimelineRowView {
                    key: format!("lane-{}-{}-pitch-{pitch}", lane_pos.col, lane_pos.row),
                    label: pitch,
                    is_pitch_row: true,
                    events: row_events
                        .into_iter()
                        .enumerate()
                        .map(|(index, event)| build_timeline_event_view(event, playhead_pct, index))
                        .collect(),
                })
                .collect::<Vec<_>>();

            rows.sort_by(|left, right| compare_pitch_labels(&left.label, &right.label));

            if !clip_row.is_empty() || rows.is_empty() {
                rows.push(CompileTimelineRowView {
                    key: format!("lane-{}-{}-clips", lane_pos.col, lane_pos.row),
                    label: if rows.is_empty() {
                        "clips".to_string()
                    } else {
                        "triggers".to_string()
                    },
                    is_pitch_row: false,
                    events: clip_row
                        .into_iter()
                        .enumerate()
                        .map(|(index, event)| build_timeline_event_view(event, playhead_pct, index))
                        .collect(),
                });
            }

            CompileTimelineLaneView {
                key: format!("lane-{}-{}", lane_pos.col, lane_pos.row),
                label: format!("Output [{}, {}]", lane_pos.col, lane_pos.row),
                subtitle: format!("{} rows", rows.len()),
                rows,
            }
        })
        .collect()
}

pub(crate) fn build_timeline_event_view(
    event: &backend::PreviewEventDto,
    playhead_pct: f64,
    index: usize,
) -> CompileTimelineEventView {
    let left_pct = event.start_normalized.clamp(0.0_f64, 1.0_f64) * 100.0_f64;
    let max_width = (100.0_f64 - left_pct).max(2.5_f64);
    let width_pct = ((event.end_normalized - event.start_normalized).max(0.025_f64) * 100.0_f64)
        .clamp(2.5_f64, max_width);
    CompileTimelineEventView {
        key: format!(
            "{}-{}-{index}",
            event.output_lane.col, event.output_lane.row
        ),
        label: event.label.clone(),
        title: format_preview_event_title(event),
        left_pct,
        width_pct,
        active: preview_event_is_active(event, playhead_pct),
    }
}

pub(crate) fn format_preview_event_title(event: &backend::PreviewEventDto) -> String {
    let site = event
        .site
        .map(|site| format!(" | site [{}, {}]", site.col, site.row))
        .unwrap_or_default();
    let pitch = event
        .pitch
        .as_ref()
        .map(|pitch| format!(" | pitch {pitch}"))
        .unwrap_or_default();
    format!(
        "{} | {:.3} -> {:.3}{}{}",
        event.label, event.start_normalized, event.end_normalized, site, pitch
    )
}

pub(crate) fn compare_pitch_labels(left: &str, right: &str) -> std::cmp::Ordering {
    pitch_sort_key(right)
        .cmp(&pitch_sort_key(left))
        .then_with(|| left.cmp(right))
}

pub(crate) fn pitch_sort_key(label: &str) -> (i32, String) {
    parse_pitch_sort_key(label)
        .map(|value| (value, label.to_ascii_lowercase()))
        .unwrap_or((i32::MIN, label.to_ascii_lowercase()))
}

pub(crate) fn parse_pitch_sort_key(label: &str) -> Option<i32> {
    let trimmed = label.trim().to_ascii_lowercase();
    let mut chars = trimmed.chars();
    let base = match chars.next()? {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let mut semitone = base;
    let mut remainder = chars.as_str();
    if let Some(next) = remainder.chars().next() {
        match next {
            '#' => {
                semitone += 1;
                remainder = &remainder[1..];
            }
            'b' => {
                semitone -= 1;
                remainder = &remainder[1..];
            }
            _ => {}
        }
    }
    let octave = remainder.parse::<i32>().ok()?;
    Some(octave * 12 + semitone)
}

pub(crate) fn format_playhead_label(status: &RuntimeStatusDto) -> String {
    format!(
        "playhead {:.3} | cps {:.3}",
        preview_playhead_normalized(status),
        rational_time_to_f64(&status.cps)
    )
}

pub(crate) fn preview_playhead_normalized(status: &RuntimeStatusDto) -> f64 {
    rational_time_to_f64(&status.cycle_position).rem_euclid(1.0)
}

pub(crate) fn rational_time_to_f64(value: &backend::RationalTimeDto) -> f64 {
    if value.denominator == 0 {
        0.0
    } else {
        value.numerator as f64 / value.denominator as f64
    }
}

pub(crate) fn preview_event_is_active(event: &backend::PreviewEventDto, playhead_pct: f64) -> bool {
    let start = event.start_normalized.clamp(0.0, 1.0);
    let end = event.end_normalized.clamp(0.0, 1.0);
    if end <= start {
        return (playhead_pct - start).abs() < 0.02;
    }
    playhead_pct >= start && playhead_pct < end
}

pub(crate) fn unique_preview_labels<'a>(
    events: impl IntoIterator<Item = &'a backend::PreviewEventDto>,
) -> Vec<String> {
    let mut labels = Vec::new();
    for event in events {
        let label = event.label.trim();
        if label.is_empty() {
            continue;
        }
        if labels
            .iter()
            .any(|existing: &String| existing.eq_ignore_ascii_case(label))
        {
            continue;
        }
        labels.push(label.to_string());
    }
    labels
}

pub(crate) fn preview_label_excerpt(labels: &[String]) -> String {
    match labels {
        [] => "Preview".to_string(),
        [only] => only.clone(),
        [first, second] => format!("{first} · {second}"),
        [first, second, rest @ ..] => {
            format!("{first} · {second} (+{} more)", rest.len())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_lanes_include_declared_outputs_without_events() {
        let output_lanes = [GridPos { col: 5, row: 4 }, GridPos { col: 6, row: 1 }];

        let lanes = build_timeline_lanes(&output_lanes, &[], 0.0);

        assert_eq!(lanes.len(), 2);
        assert_eq!(lanes[0].label, "Output [5, 4]");
        assert_eq!(lanes[1].label, "Output [6, 1]");
        assert!(lanes.iter().all(|lane| lane.rows.len() == 1));
    }
}
