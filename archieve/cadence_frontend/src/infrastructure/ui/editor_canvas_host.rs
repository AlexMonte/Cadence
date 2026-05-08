use dioxus::prelude::*;

use crate::adapter::canvas::{
    CanvasPointerInput, CanvasPointerPhase, default_canvas_theme, draw_commands_from_view_model,
    grid_pos_from_canvas_cell, hit_test_board, map_pointer_to_command, render_canvas_by_id,
    render_frame,
};
use crate::adapter::dioxus::command_bridge::dispatch_editor_command;
use crate::application::editor::{EditorShellState, build_board_view_model};

#[component]
pub(crate) fn EditorCanvasHost(state: Signal<EditorShellState>, snapshot: EditorShellState) -> Element {
    let view_model = build_board_view_model(&snapshot);
    let frame = render_frame(
        draw_commands_from_view_model(&view_model),
        default_canvas_theme(),
    );
    let board_style = format!(
        "width: {}px; height: {}px;",
        view_model.viewport.width_px, view_model.viewport.height_px
    );
    let render_frame_snapshot = frame.clone();
    let render_width = view_model.viewport.width_px;
    let render_height = view_model.viewport.height_px;
    let pointerdown_snapshot = snapshot.clone();
    let pointermove_snapshot = snapshot.clone();
    let pointerup_snapshot = snapshot.clone();
    use_effect(move || {
        let _ = render_canvas_by_id(
            "editor-board-canvas",
            render_width,
            render_height,
            &render_frame_snapshot,
        );
    });

    rsx! {
        section {
            class: "editor-canvas-host canvas-workspace-shell",
            "aria-label": "Canvas composition board",
            div {
                class: "board-canvas-frame pixel-material pixel-panel pixel-panel--stone",
                canvas {
                    id: "editor-board-canvas",
                    class: "editor-board-canvas",
                    width: "{view_model.viewport.width_px}",
                    height: "{view_model.viewport.height_px}",
                    style: "{board_style}",
                    tabindex: "0",
                    "data-command-count": "{frame.commands.len()}",
                    "data-issue-count": "{view_model.diagnostics.issue_count}",
                    "data-selection-count": "{view_model.selection.selected_tiles.len()}",
                    "data-phase": "canvas-host",
                    onpointerdown: move |event| {
                        handle_canvas_pointer_event(state, &pointerdown_snapshot, event, CanvasPointerPhase::Down);
                    },
                    onpointermove: move |event| {
                        handle_canvas_pointer_event(state, &pointermove_snapshot, event, CanvasPointerPhase::Move);
                    },
                    onpointerup: move |event| {
                        handle_canvas_pointer_event(state, &pointerup_snapshot, event, CanvasPointerPhase::Up);
                    },
                    onpointerleave: move |_| {
                        let state = state;
                        spawn(async move {
                            let _ = dispatch_editor_command(state, crate::application::editor::EditorCommand::ClearBoardInteraction).await;
                        });
                    }
                }
            }
        }
    }
}

fn handle_canvas_pointer_event(
    state: Signal<EditorShellState>,
    snapshot: &EditorShellState,
    event: PointerEvent,
    phase: CanvasPointerPhase,
) {
    let coords = event.data().element_coordinates();
    let input = CanvasPointerInput {
        x: coords.x,
        y: coords.y,
        shift: event.data().modifiers().shift(),
    };
    let view_model = build_board_view_model(snapshot);
    let hit_target = hit_test_board(&view_model, input.x, input.y);
    let grid_position = grid_position_from_canvas_point(&view_model, input.x, input.y);
    if let Some(command) = map_pointer_to_command(snapshot, hit_target, input, phase, grid_position) {
        spawn(async move {
            let _ = dispatch_editor_command(state, command).await;
        });
    }
}

fn grid_position_from_canvas_point(
    view_model: &crate::application::editor::EditorBoardViewModel,
    x: f64,
    y: f64,
) -> Option<crate::adapter::GridPos> {
    let x = x - f64::from(view_model.viewport.origin_x_px);
    let y = y - f64::from(view_model.viewport.origin_y_px);
    if x < 0.0 || y < 0.0 {
        return None;
    }
    let col = (x / f64::from(view_model.viewport.cell_width_px)).floor() as i32;
    let row = (y / f64::from(view_model.viewport.cell_height_px)).floor() as i32;
    if col >= view_model.grid.cols as i32 || row >= view_model.grid.rows as i32 {
        return None;
    }
    Some(grid_pos_from_canvas_cell(col, row))
}
