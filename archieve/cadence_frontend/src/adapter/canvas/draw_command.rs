use crate::adapter::GridPos;
use crate::application::editor::{BoardTileKind, DragPreviewKind, EditorBoardViewModel};

#[derive(Clone, Debug, PartialEq)]
pub enum CanvasDrawCommand {
    DrawBackground {
        width_px: i32,
        height_px: i32,
    },
    DrawGrid {
        cols: u32,
        rows: u32,
        cell_width_px: i32,
        cell_height_px: i32,
        origin_x_px: i32,
        origin_y_px: i32,
    },
    DrawConnection {
        from_x_px: i32,
        from_y_px: i32,
        to_x_px: i32,
        to_y_px: i32,
    },
    DrawTileFrame {
        x_px: i32,
        y_px: i32,
        width_px: i32,
        height_px: i32,
        kind: BoardTileKind,
        selected: bool,
    },
    DrawTileLabel {
        x_px: i32,
        y_px: i32,
        text: String,
    },
    DrawPort {
        x_px: i32,
        y_px: i32,
        radius_px: i32,
        is_output: bool,
    },
    DrawSelectionCell {
        x_px: i32,
        y_px: i32,
        width_px: i32,
        height_px: i32,
    },
    DrawDragPreview {
        x_px: i32,
        y_px: i32,
        width_px: i32,
        height_px: i32,
        kind: DragPreviewKind,
    },
    DrawConnectionPreview {
        from_x_px: i32,
        from_y_px: i32,
        to_x_px: i32,
        to_y_px: i32,
    },
    DrawMarquee {
        left_px: i32,
        top_px: i32,
        width_px: i32,
        height_px: i32,
    },
    DrawDiagnosticsBanner {
        text: String,
    },
}

pub fn draw_commands_from_view_model(view_model: &EditorBoardViewModel) -> Vec<CanvasDrawCommand> {
    let mut commands = vec![
        CanvasDrawCommand::DrawBackground {
            width_px: view_model.viewport.width_px,
            height_px: view_model.viewport.height_px,
        },
        CanvasDrawCommand::DrawGrid {
            cols: view_model.grid.cols,
            rows: view_model.grid.rows,
            cell_width_px: view_model.viewport.cell_width_px,
            cell_height_px: view_model.viewport.cell_height_px,
            origin_x_px: view_model.viewport.origin_x_px,
            origin_y_px: view_model.viewport.origin_y_px,
        },
    ];

    commands.extend(view_model.connections.iter().map(|edge| CanvasDrawCommand::DrawConnection {
        from_x_px: edge.from_x_px,
        from_y_px: edge.from_y_px,
        to_x_px: edge.to_x_px,
        to_y_px: edge.to_y_px,
    }));

    for tile in &view_model.tiles {
        commands.push(CanvasDrawCommand::DrawTileFrame {
            x_px: tile.x_px,
            y_px: tile.y_px,
            width_px: tile.width_px,
            height_px: tile.height_px,
            kind: tile.kind.clone(),
            selected: tile.selected,
        });
        commands.push(CanvasDrawCommand::DrawTileLabel {
            x_px: tile.x_px + 8,
            y_px: tile.y_px + 20,
            text: tile.label.clone(),
        });
    }

    if let Some(cell) = view_model.selection.selected_cell {
        let (x_px, y_px) = cell_rect_origin(
            cell,
            view_model.viewport.origin_x_px,
            view_model.viewport.origin_y_px,
            view_model.viewport.cell_width_px,
            view_model.viewport.cell_height_px,
        );
        commands.push(CanvasDrawCommand::DrawSelectionCell {
            x_px,
            y_px,
            width_px: view_model.viewport.cell_width_px,
            height_px: view_model.viewport.cell_height_px,
        });
    }

    commands.extend(view_model.ports.iter().map(|port| CanvasDrawCommand::DrawPort {
        x_px: port.x_px,
        y_px: port.y_px,
        radius_px: port.radius_px,
        is_output: port.is_output,
    }));

    if let Some(preview) = &view_model.drag_preview {
        commands.push(CanvasDrawCommand::DrawDragPreview {
            x_px: preview.x_px,
            y_px: preview.y_px,
            width_px: preview.width_px,
            height_px: preview.height_px,
            kind: preview.kind.clone(),
        });
    }

    if let Some(preview) = &view_model.connection_preview {
        let (from_x_px, from_y_px) = tile_center_px(preview.from, view_model);
        commands.push(CanvasDrawCommand::DrawConnectionPreview {
            from_x_px,
            from_y_px,
            to_x_px: preview.to_x_px,
            to_y_px: preview.to_y_px,
        });
    }

    if let Some(marquee) = &view_model.marquee {
        commands.push(CanvasDrawCommand::DrawMarquee {
            left_px: marquee.left_px,
            top_px: marquee.top_px,
            width_px: marquee.width_px,
            height_px: marquee.height_px,
        });
    }

    if view_model.diagnostics.issue_count > 0 {
        commands.push(CanvasDrawCommand::DrawDiagnosticsBanner {
            text: format!("{} issue(s) active", view_model.diagnostics.issue_count),
        });
    }

    commands
}

fn cell_rect_origin(
    cell: GridPos,
    origin_x_px: i32,
    origin_y_px: i32,
    cell_width_px: i32,
    cell_height_px: i32,
) -> (i32, i32) {
    (
        origin_x_px + cell.col * cell_width_px,
        origin_y_px + cell.row * cell_height_px,
    )
}

fn tile_center_px(position: GridPos, view_model: &EditorBoardViewModel) -> (i32, i32) {
    let tile = view_model
        .tiles
        .iter()
        .find(|tile| tile.position == position)
        .expect("connection preview source tile should exist");
    (tile.x_px + tile.width_px / 2, tile.y_px + tile.height_px / 2)
}
