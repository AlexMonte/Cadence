use bevy::prelude::*;

use super::{
    BoardCommand, BoardFocus, BoardStore, CommandError, InteractionStore, SelectionStore,
    commands::assert_store_invariants, drag_service, focus_service,
};

#[derive(Debug, Message, Clone)]
pub struct BoardCommandEvent(pub BoardCommand);

#[derive(Debug, Resource, Clone)]
pub struct LastBoardError(pub Option<String>);

impl Default for LastBoardError {
    fn default() -> Self {
        Self(None)
    }
}

pub fn apply_board_commands(
    mut commands: MessageReader<BoardCommandEvent>,
    mut board_store: ResMut<BoardStore>,
    mut board_focus: ResMut<BoardFocus>,
    mut selection_store: ResMut<SelectionStore>,
    mut interactions: ResMut<InteractionStore>,
    mut last_error: ResMut<LastBoardError>,
) {
    for BoardCommandEvent(command) in commands.read() {
        let result: Result<(), CommandError> = match command {
            BoardCommand::SelectSlot { board_id, slot } => {
                selection_store.set_selected_slot(board_id.clone(), *slot);
                Ok(())
            }
            BoardCommand::OpenContainer { tile_id } => {
                focus_service::open_container(
                    &mut board_focus,
                    &mut selection_store,
                    &board_store,
                    tile_id,
                )
                    .map_err(CommandError::from)
            }
            BoardCommand::CloseContainer => {
                focus_service::close_container(&mut board_focus);
                Ok(())
            }
            BoardCommand::StartPaletteDrag { piece_id, class } => {
                drag_service::start_palette_drag(
                    &mut interactions,
                    board_focus.active_board.clone(),
                    piece_id.clone(),
                    *class,
                );
                Ok(())
            }
            BoardCommand::StartTileDrag { tile_id } => {
                drag_service::start_tile_drag(&mut interactions, &board_store, tile_id.clone())
                    .map_err(CommandError::from)
            }
            BoardCommand::UpdateDrag { pointer } => {
                drag_service::update_drag(
                    &mut interactions,
                    &board_store,
                    &board_focus,
                    pointer.clone(),
                );
                Ok(())
            }
            BoardCommand::CommitDrag => {
                drag_service::commit_drag(
                    &mut interactions,
                    &mut board_store,
                    &mut selection_store,
                )
                    .map_err(CommandError::from)
            }
            BoardCommand::CancelDrag => {
                interactions.interaction = super::BoardInteraction::Idle;
                Ok(())
            }
            BoardCommand::ZoomViewport { factor } => {
                let active_board = board_focus.active_board.clone();
                let viewport = board_focus
                    .viewport_by_board
                    .entry(active_board)
                    .or_default();
                viewport.zoom = (viewport.zoom * *factor).clamp(0.4, 3.5);
                Ok(())
            }
            BoardCommand::PanViewport { delta } => {
                let active_board = board_focus.active_board.clone();
                let viewport = board_focus
                    .viewport_by_board
                    .entry(active_board)
                    .or_default();
                viewport.pan.x += delta.x / viewport.zoom.max(0.1);
                viewport.pan.y += delta.y / viewport.zoom.max(0.1);
                Ok(())
            }
        };

        if let Err(error) = result.and_then(|_| assert_store_invariants(&board_store)) {
            last_error.0 = Some(error.to_string());
        } else {
            last_error.0 = None;
        }
    }
}
