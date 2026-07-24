//! Pure command execution — applies an [`EditorCommand`](super::types::EditorCommand)
//! or [`EditorInverse`](super::types::EditorInverse) to document / board / attention.

use tessera::bevy::TesseraBoard;

use super::types::{EditorCommand, EditorInverse};
use crate::application::editor::transaction::{
    self, EditorTransactionResult, Invalidation, TimelineSourceResolver,
};
use crate::application::editor::workspace::FocusTarget;
use crate::application::editor::{EditorAttention, SelectionState};
use crate::domain::DomainError;
use crate::domain::document::{DocumentQueries, MusaicDocument, apply_document_patch};

pub fn execute_command<R>(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    resolver: &R,
    command: &EditorCommand,
) -> Result<EditorTransactionResult, DomainError>
where
    R: TimelineSourceResolver,
{
    let result = match command {
        // ── Ephemeral attention/selection ──
        EditorCommand::Focus { target } => {
            let queries = DocumentQueries::new(document);
            match attention.focus(&queries, target.clone()) {
                Ok(()) => EditorTransactionResult::accepted(Invalidation::none()),
                Err(error) => EditorTransactionResult::rejected(error.to_string()),
            }
        }
        EditorCommand::SelectNode { node, mode } => {
            let queries = DocumentQueries::new(document);
            selection.select(node.clone(), *mode);
            match attention.focus(&queries, FocusTarget::Tile { node: node.clone() }) {
                Ok(()) => EditorTransactionResult::accepted(Invalidation::none()),
                Err(error) => EditorTransactionResult::rejected(error.to_string()),
            }
        }
        EditorCommand::ClearSelection => {
            let queries = DocumentQueries::new(document);
            selection.clear();
            attention
                .focus(&queries, FocusTarget::None)
                .expect("FocusTarget::None is always valid");
            EditorTransactionResult::accepted(Invalidation::none())
        }
        // ── Durable document mutations ──
        EditorCommand::PlaceTile { target, tile } => {
            transaction::place_tile(document, board, attention, selection, *target, tile.clone())
        }
        EditorCommand::EnterContainer { container } => {
            transaction::enter_container(document, attention, selection, container)
        }
        EditorCommand::NavigateToSurface { surface } => {
            transaction::navigate_to_surface(document, attention, selection, *surface)
        }
        EditorCommand::ConnectTiles { from, to } => transaction::connect_tiles(
            document,
            board,
            attention,
            selection,
            from.clone(),
            to.clone(),
        ),
        EditorCommand::BindOutputSide { node, side } => {
            transaction::bind_output_side(document, board, attention, node, *side)
        }
        EditorCommand::CycleConnection { from, to } => {
            transaction::cycle_connection(document, board, attention, from, to)
        }
        EditorCommand::DeleteSelection => {
            transaction::delete_selection(document, board, attention, selection)
        }
        EditorCommand::JumpToTimelineSource { event } => {
            transaction::jump_to_timeline_source(document, attention, selection, resolver, *event)
        }
        EditorCommand::SetBpm { bpm } => {
            document.playback.bpm = bpm.max(1.0);
            EditorTransactionResult::accepted(Invalidation {
                runtime: true,
                save: true,
                ..Invalidation::default()
            })
        }

        // Handled by the dispatcher (session/tool/panel/project/view), never here.
        EditorCommand::ArmPlacementTool { .. }
        | EditorCommand::CancelPlacement
        | EditorCommand::StartConnection { .. }
        | EditorCommand::AbortConnection
        | EditorCommand::ToggleDrawer
        | EditorCommand::ToggleMinimap
        | EditorCommand::EnterTimelineMode
        | EditorCommand::EnterCompose
        | EditorCommand::SetViewSettings { .. }
        | EditorCommand::NewProject
        | EditorCommand::OpenProject { .. }
        | EditorCommand::AdoptProject { .. }
        | EditorCommand::SaveProject
        | EditorCommand::SaveProjectAs { .. }
        | EditorCommand::Undo
        | EditorCommand::Redo
        | EditorCommand::TransportPlay
        | EditorCommand::TransportStop
        | EditorCommand::TransportToggle
        | EditorCommand::TransportSeek { .. } => {
            return Err(DomainError::Message(format!(
                "command {command:?} must be handled by the dispatcher"
            )));
        }
    };

    Ok(result)
}

/// Applies a stored undo inverse. Public only to the command/history layer.
pub fn execute_inverse(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    attention: &mut EditorAttention,
    selection: &mut SelectionState,
    inverse: &EditorInverse,
) -> Result<EditorTransactionResult, DomainError> {
    let result = match inverse {
        EditorInverse::DeleteNode { node } => {
            transaction::delete_node(document, board, attention, selection, node)
        }
        EditorInverse::RestoreSubtree { patch } => {
            match apply_document_patch(document, board, patch.clone()) {
                Ok(()) => EditorTransactionResult::accepted(Invalidation::document_replaced()),
                Err(message) => EditorTransactionResult::rejected(message),
            }
        }
        EditorInverse::DisconnectTiles { from, to } => {
            transaction::disconnect_tiles(document, board, from, to)
        }
        EditorInverse::RestorePortBinding {
            node,
            side,
            port_state,
            removed_binding,
        } => transaction::restore_port_binding(
            document,
            board,
            node,
            *side,
            *port_state,
            removed_binding.clone(),
        ),
    };
    Ok(result)
}
