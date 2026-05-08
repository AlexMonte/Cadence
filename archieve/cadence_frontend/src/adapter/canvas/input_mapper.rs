use crate::adapter::GridPos;
use crate::adapter::canvas::CanvasHitTarget;
use crate::application::editor::{EditorCommand, EditorShellState, InteractionState};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasPointerInput {
    pub x: f64,
    pub y: f64,
    pub shift: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasPointerPhase {
    Down,
    Move,
    Up,
}

pub fn map_pointer_to_command(
    state: &EditorShellState,
    hit_target: CanvasHitTarget,
    input: CanvasPointerInput,
    phase: CanvasPointerPhase,
    grid_position: Option<GridPos>,
) -> Option<EditorCommand> {
    match phase {
        CanvasPointerPhase::Down => match hit_target {
            CanvasHitTarget::TilePort(position) => {
                Some(EditorCommand::BeginConnection { from: position })
            }
            CanvasHitTarget::TileBody(position) => Some(EditorCommand::BeginTileDrag { position }),
            CanvasHitTarget::CanvasBackground => {
                grid_position.map(|position| EditorCommand::BeginMarquee {
                    origin: position,
                    additive: input.shift,
                })
            }
            CanvasHitTarget::Connection(_) => None,
        },
        CanvasPointerPhase::Move => match state.interaction_state {
            Some(InteractionState::Dragging(
                crate::application::editor::DragSession::NodeMove { .. },
            ))
            | Some(InteractionState::Armed(
                crate::application::editor::ArmedInteraction::NodeBody { .. },
            )) => grid_position.map(|position| EditorCommand::PreviewTileDrag { position }),
            Some(InteractionState::Dragging(
                crate::application::editor::DragSession::EdgeFrom { .. },
            ))
            | Some(crate::application::editor::InteractionState::Armed(
                crate::application::editor::ArmedInteraction::EdgeHandle { .. },
            )) => grid_position.map(|position| EditorCommand::PreviewConnection { position }),
            Some(InteractionState::Dragging(
                crate::application::editor::DragSession::CanvasMarquee { .. },
            ))
            | Some(crate::application::editor::InteractionState::Armed(
                crate::application::editor::ArmedInteraction::CanvasMarquee { .. },
            )) => grid_position.map(|current| EditorCommand::PreviewMarquee { current }),
            _ => None,
        },
        CanvasPointerPhase::Up => match state.interaction_state {
            Some(InteractionState::Dragging(
                crate::application::editor::DragSession::NodeMove { .. },
            ))
            | Some(crate::application::editor::InteractionState::Armed(
                crate::application::editor::ArmedInteraction::NodeBody { .. },
            )) => match hit_target {
                CanvasHitTarget::TileBody(position) => {
                    Some(EditorCommand::CommitTileDrag { position })
                }
                CanvasHitTarget::CanvasBackground => {
                    grid_position.map(|position| EditorCommand::CommitTileDrag { position })
                }
                CanvasHitTarget::TilePort(position) => {
                    Some(EditorCommand::CommitTileDrag { position })
                }
                CanvasHitTarget::Connection(_) => None,
            },
            Some(InteractionState::Dragging(
                crate::application::editor::DragSession::EdgeFrom { .. },
            ))
            | Some(crate::application::editor::InteractionState::Armed(
                crate::application::editor::ArmedInteraction::EdgeHandle { .. },
            )) => match hit_target {
                CanvasHitTarget::TileBody(position) | CanvasHitTarget::TilePort(position) => {
                    Some(EditorCommand::CommitConnection { position })
                }
                _ => Some(EditorCommand::ClearBoardInteraction),
            },
            Some(InteractionState::Dragging(
                crate::application::editor::DragSession::CanvasMarquee { .. },
            ))
            | Some(crate::application::editor::InteractionState::Armed(
                crate::application::editor::ArmedInteraction::CanvasMarquee { .. },
            )) => Some(EditorCommand::CommitMarquee),
            _ => match hit_target {
                CanvasHitTarget::TileBody(position) => Some(EditorCommand::SelectTile {
                    position,
                    additive: input.shift,
                }),
                CanvasHitTarget::CanvasBackground => {
                    grid_position.map(|position| EditorCommand::SelectCell {
                        position,
                        additive: input.shift,
                    })
                }
                CanvasHitTarget::TilePort(position) => Some(EditorCommand::SelectTile {
                    position,
                    additive: input.shift,
                }),
                CanvasHitTarget::Connection(_) => None,
            },
        },
    }
}

pub fn grid_pos_from_canvas_cell(col: i32, row: i32) -> GridPos {
    GridPos { col, row }
}
