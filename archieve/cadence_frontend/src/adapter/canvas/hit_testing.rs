use crate::adapter::GridPos;
use crate::application::editor::EditorBoardViewModel;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanvasHitTarget {
    CanvasBackground,
    TileBody(GridPos),
    TilePort(GridPos),
    Connection(String),
}

pub fn hit_test_board(
    view_model: &EditorBoardViewModel,
    board_x: f64,
    board_y: f64,
) -> CanvasHitTarget {
    for tile in &view_model.tiles {
        let left = f64::from(tile.x_px);
        let top = f64::from(tile.y_px);
        let right = left + f64::from(tile.width_px);
        let bottom = top + f64::from(tile.height_px);
        if board_x >= left && board_x <= right && board_y >= top && board_y <= bottom {
            return CanvasHitTarget::TileBody(tile.position);
        }
    }
    for port in &view_model.ports {
        let dx = board_x - f64::from(port.x_px);
        let dy = board_y - f64::from(port.y_px);
        let radius = f64::from(port.radius_px);
        if dx * dx + dy * dy <= radius * radius {
            return CanvasHitTarget::TilePort(port.tile_position);
        }
    }
    CanvasHitTarget::CanvasBackground
}
