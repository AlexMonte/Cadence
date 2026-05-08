use crate::adapter::ParamDef;

use super::{EditorShellState, WorkspaceMode, selected_node_view, selected_piece_def};

pub const EDITING_SURFACE_INVARIANT: &str =
    "Input tiles are the editing surface. Inspector fields summarize routing and state.";

pub fn side_config_text(state: &EditorShellState) -> String {
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
        (Some(node), Some(piece), _) => {
            let label = node.label.as_deref().unwrap_or(piece.label.as_str());
            format!("{label} selected at {position}")
        }
        (Some(_node), None, _) => format!("Tile at {position} is selected"),
        (None, _, WorkspaceMode::Init) => {
            "Choose a setup card to manage samples and patterns.".to_string()
        }
        _ => "Choose a tile to shape its sound, timing, and routing.".to_string(),
    }
}

pub fn format_selected_position(position: Option<&crate::adapter::GridPos>) -> String {
    position
        .map(|pos| format!("[{}, {}]", pos.col, pos.row))
        .unwrap_or_else(|| "[-, -]".to_string())
}

pub fn format_param_schema(param: &ParamDef) -> String {
    param
        .text_semantics
        .clone()
        .or_else(|| param.input_context.as_ref().map(|context| format!("{context:?}")))
        .unwrap_or_else(|| {
            if param.required {
                "required input".to_string()
            } else {
                "optional input".to_string()
            }
        })
}
