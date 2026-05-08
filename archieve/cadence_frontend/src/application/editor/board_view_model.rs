use crate::adapter::GridPos;
use crate::domain::{CELL_H, CELL_W, GRID_ORIGIN_X, GRID_ORIGIN_Y, GraphView, NODE_H, NODE_W};

use super::{
    DragHoverStatus, DragPreviewFeedback, DragPreviewKind, DragPreviewOutcomeKind, DragSession,
    EditorShellState, InteractionState, PointerPoint,
    selected_node_view,
};

#[derive(Clone, Debug, PartialEq)]
pub struct BoardViewport {
    pub width_px: i32,
    pub height_px: i32,
    pub cell_width_px: i32,
    pub cell_height_px: i32,
    pub origin_x_px: i32,
    pub origin_y_px: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BoardTileKind {
    Atom,
    Container,
    Generator,
    Output,
    Transform,
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardTileViewModel {
    pub position: GridPos,
    pub label: String,
    pub piece_id: String,
    pub kind: BoardTileKind,
    pub selected: bool,
    pub x_px: i32,
    pub y_px: i32,
    pub width_px: i32,
    pub height_px: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardConnectionViewModel {
    pub id: String,
    pub from: GridPos,
    pub to: GridPos,
    pub from_x_px: i32,
    pub from_y_px: i32,
    pub to_x_px: i32,
    pub to_y_px: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardPortViewModel {
    pub tile_position: GridPos,
    pub x_px: i32,
    pub y_px: i32,
    pub radius_px: i32,
    pub is_output: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardSelectionViewModel {
    pub selected_cell: Option<GridPos>,
    pub selected_tiles: Vec<GridPos>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardDiagnosticsViewModel {
    pub issue_count: usize,
    pub drag_preview_label: Option<String>,
    pub drag_hover_label: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardDragPreviewViewModel {
    pub kind: DragPreviewKind,
    pub x_px: i32,
    pub y_px: i32,
    pub width_px: i32,
    pub height_px: i32,
    pub target: Option<GridPos>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardConnectionPreviewViewModel {
    pub from: GridPos,
    pub to_x_px: i32,
    pub to_y_px: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoardMarqueeViewModel {
    pub left_px: i32,
    pub top_px: i32,
    pub width_px: i32,
    pub height_px: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EditorBoardViewModel {
    pub viewport: BoardViewport,
    pub grid: GraphView,
    pub tiles: Vec<BoardTileViewModel>,
    pub connections: Vec<BoardConnectionViewModel>,
    pub ports: Vec<BoardPortViewModel>,
    pub selection: BoardSelectionViewModel,
    pub diagnostics: BoardDiagnosticsViewModel,
    pub drag_preview: Option<BoardDragPreviewViewModel>,
    pub connection_preview: Option<BoardConnectionPreviewViewModel>,
    pub marquee: Option<BoardMarqueeViewModel>,
}

pub fn build_board_view_model(state: &EditorShellState) -> EditorBoardViewModel {
    let width_px = GRID_ORIGIN_X * 2 + state.graph.cols as i32 * CELL_W;
    let height_px = GRID_ORIGIN_Y * 2 + state.graph.rows as i32 * CELL_H;
    let selected_tile = selected_node_view(state).map(|node| node.position);
    let tiles = state
        .graph
        .nodes
        .iter()
        .map(|node| {
            let (x_px, y_px) = tile_origin(node.position);
            let label = node
                .label
                .clone()
                .unwrap_or_else(|| node.piece_id.rsplit('.').next().unwrap_or("tile").to_string());
            BoardTileViewModel {
                position: node.position,
                label,
                piece_id: node.piece_id.clone(),
                kind: classify_tile_kind(&node.piece_id),
                selected: selected_tile == Some(node.position)
                    || state.selected_nodes.contains(&node.position),
                x_px,
                y_px,
                width_px: NODE_W,
                height_px: NODE_H,
            }
        })
        .collect();
    let connections = state
        .graph
        .edges
        .iter()
        .map(|edge| {
            let (from_x_px, from_y_px) = tile_center(edge.from);
            let (to_x_px, to_y_px) = tile_center(edge.to_node);
            BoardConnectionViewModel {
                id: edge.id.clone(),
                from: edge.from,
                to: edge.to_node,
                from_x_px,
                from_y_px,
                to_x_px,
                to_y_px,
            }
        })
        .collect();
    let ports = state
        .graph
        .nodes
        .iter()
        .flat_map(|node| {
            let (tile_x, tile_y) = tile_origin(node.position);
            let center_y = tile_y + NODE_H / 2;
            let left_x = tile_x - 6;
            let right_x = tile_x + NODE_W + 6;
            vec![
                BoardPortViewModel {
                    tile_position: node.position,
                    x_px: left_x,
                    y_px: center_y,
                    radius_px: 5,
                    is_output: false,
                },
                BoardPortViewModel {
                    tile_position: node.position,
                    x_px: right_x,
                    y_px: center_y,
                    radius_px: 5,
                    is_output: true,
                },
            ]
        })
        .collect();

    EditorBoardViewModel {
        viewport: BoardViewport {
            width_px,
            height_px,
            cell_width_px: CELL_W,
            cell_height_px: CELL_H,
            origin_x_px: GRID_ORIGIN_X,
            origin_y_px: GRID_ORIGIN_Y,
        },
        grid: state.graph.clone(),
        tiles,
        connections,
        ports,
        selection: BoardSelectionViewModel {
            selected_cell: state.selected_cell,
            selected_tiles: state.selected_nodes.clone(),
        },
        diagnostics: BoardDiagnosticsViewModel {
            issue_count: state.diagnostics.entries.len(),
            drag_preview_label: state.drag_preview.as_ref().map(drag_preview_label),
            drag_hover_label: state.drag_hover.as_ref().map(|hover| match hover.status {
                DragHoverStatus::Valid => "drop valid".to_string(),
                DragHoverStatus::Invalid => "drop invalid".to_string(),
                DragHoverStatus::Swap => "drop swaps".to_string(),
            }),
        },
        drag_preview: state.drag_preview.as_ref().map(board_drag_preview),
        connection_preview: board_connection_preview(state),
        marquee: board_marquee(state),
    }
}

fn tile_origin(position: GridPos) -> (i32, i32) {
    let cell_left = GRID_ORIGIN_X + position.col * CELL_W;
    let cell_top = GRID_ORIGIN_Y + position.row * CELL_H;
    (
        cell_left + (CELL_W - NODE_W) / 2,
        cell_top + (CELL_H - NODE_H) / 2,
    )
}

fn classify_tile_kind(piece_id: &str) -> BoardTileKind {
    if piece_id.starts_with("cadence.atom.") {
        BoardTileKind::Atom
    } else if piece_id.contains("output") {
        BoardTileKind::Output
    } else if piece_id.contains("transform") {
        BoardTileKind::Transform
    } else if piece_id.contains("container") {
        BoardTileKind::Container
    } else if piece_id.contains("generator") || piece_id.starts_with("cadence.") {
        BoardTileKind::Generator
    } else {
        BoardTileKind::Unknown
    }
}

fn tile_center(position: GridPos) -> (i32, i32) {
    let (x_px, y_px) = tile_origin(position);
    (x_px + NODE_W / 2, y_px + NODE_H / 2)
}

fn drag_preview_label(preview: &DragPreviewFeedback) -> String {
    match preview.kind {
        DragPreviewOutcomeKind::Connect => format!("preview connect at {}, {}", preview.position.col, preview.position.row),
        DragPreviewOutcomeKind::MoveOnly => {
            format!("preview move at {}, {}", preview.position.col, preview.position.row)
        }
        DragPreviewOutcomeKind::Blocked => {
            format!("preview blocked at {}, {}", preview.position.col, preview.position.row)
        }
    }
}

fn board_drag_preview(preview: &DragPreviewFeedback) -> BoardDragPreviewViewModel {
    let (x_px, y_px) = tile_origin(preview.position);
    BoardDragPreviewViewModel {
        kind: preview.drag_kind.clone(),
        x_px,
        y_px,
        width_px: NODE_W,
        height_px: NODE_H,
        target: preview.target,
    }
}

fn board_connection_preview(state: &EditorShellState) -> Option<BoardConnectionPreviewViewModel> {
    match state.interaction_state.as_ref() {
        Some(InteractionState::Dragging(DragSession::EdgeFrom {
            from,
            board_page_origin,
            current_page,
            ..
        })) => Some(BoardConnectionPreviewViewModel {
            from: *from,
            to_x_px: (current_page.x - board_page_origin.x).round() as i32,
            to_y_px: (current_page.y - board_page_origin.y).round() as i32,
        }),
        _ => None,
    }
}

fn board_marquee(state: &EditorShellState) -> Option<BoardMarqueeViewModel> {
    match state.interaction_state.as_ref() {
        Some(InteractionState::Dragging(DragSession::CanvasMarquee {
            anchor, current, ..
        })) => Some(rect_from_points(*anchor, *current)),
        Some(InteractionState::Armed(super::ArmedInteraction::CanvasMarquee {
            origin,
            board_page_origin,
            ..
        })) => {
            let anchor = PointerPoint {
                x: origin.x - board_page_origin.x,
                y: origin.y - board_page_origin.y,
            };
            Some(rect_from_points(anchor, anchor))
        }
        _ => None,
    }
}

fn rect_from_points(a: PointerPoint, b: PointerPoint) -> BoardMarqueeViewModel {
    let left_px = a.x.min(b.x).round() as i32;
    let top_px = a.y.min(b.y).round() as i32;
    let width_px = (a.x - b.x).abs().round() as i32;
    let height_px = (a.y - b.y).abs().round() as i32;
    BoardMarqueeViewModel {
        left_px,
        top_px,
        width_px,
        height_px,
    }
}
