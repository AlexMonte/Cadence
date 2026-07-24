//! Pure pick classification — maps raw board hits to editor commands.

use crate::domain::document::queries::DocumentQueries;

use super::picking::{BoardPick, PickHit, PickModifiers, command_from_pick_checked};
use super::session::EditorSession;
use super::types::{BoardPickEvent, BoardPickHit, BoardPickTargetKind};
use crate::application::command::EditorCommand;
use crate::application::editor::{
    EditorAttention, WorkspaceMode, transaction::PlacementTarget, workspace::FocusTarget,
};

pub fn classify_board_pick(
    queries: &DocumentQueries<'_>,
    attention: &EditorAttention,
    session: &EditorSession,
    pick: &BoardPickEvent,
) -> Option<EditorCommand> {
    match pick {
        BoardPickEvent::Miss => Some(EditorCommand::ClearSelection),
        BoardPickEvent::Hit(hit) => {
            classify_board_pick_hit(queries, attention, session, hit, PickModifiers::default())
        }
    }
}

fn classify_board_pick_hit(
    queries: &DocumentQueries<'_>,
    attention: &EditorAttention,
    session: &EditorSession,
    hit: &BoardPickHit,
    modifiers: PickModifiers,
) -> Option<EditorCommand> {
    if let BoardPickTargetKind::Connection { from, to } = &hit.kind {
        if !session.is_connecting() {
            return Some(EditorCommand::CycleConnection {
                from: from.clone(),
                to: to.clone(),
            });
        }
        return None;
    }

    if let BoardPickTargetKind::PortSide { tile_id, side } = &hit.kind {
        return Some(EditorCommand::BindOutputSide {
            node: tile_id.clone(),
            side: *side,
        });
    }

    let pick_hit = board_pick_hit_to_pick_hit(hit);

    if let Some(command) = context_command_from_pick_hit(queries, attention, &pick_hit) {
        return Some(command);
    }

    if let Some(command) = connection_command_from_pick_hit(session, &pick_hit) {
        return Some(command);
    }

    if let Some(command) = placement_command_from_pick_hit(session, &pick_hit) {
        return Some(command);
    }

    command_from_pick_checked(
        queries,
        BoardPick {
            hit: pick_hit,
            modifiers,
        },
    )
}

fn board_pick_hit_to_pick_hit(hit: &BoardPickHit) -> PickHit {
    match &hit.kind {
        BoardPickTargetKind::Slot { slot } => PickHit::EmptySlot {
            surface: hit.surface_id,
            slot: *slot,
        },
        BoardPickTargetKind::StackInsert { index } => PickHit::StackInsert {
            surface: hit.surface_id,
            index: *index,
        },
        BoardPickTargetKind::BoardTile { tile_id } | BoardPickTargetKind::StackTile { tile_id } => {
            PickHit::Tile {
                node: tile_id.clone(),
            }
        }
        BoardPickTargetKind::Connection { .. } | BoardPickTargetKind::PortSide { .. } => {
            unreachable!("connection and port picks are handled earlier")
        }
    }
}

fn placement_command_from_pick_hit(
    session: &EditorSession,
    hit: &PickHit,
) -> Option<EditorCommand> {
    let target = match hit {
        PickHit::EmptySlot { surface, slot } => PlacementTarget::BoardSlot {
            surface: *surface,
            slot: *slot,
        },
        PickHit::StackInsert { surface, index } => PlacementTarget::StackIndex {
            surface: *surface,
            index: *index,
        },
        _ => return None,
    };

    session
        .armed_tile()
        .cloned()
        .map(|tile| EditorCommand::PlaceTile { target, tile })
}

fn context_command_from_pick_hit(
    queries: &DocumentQueries<'_>,
    attention: &EditorAttention,
    hit: &PickHit,
) -> Option<EditorCommand> {
    if attention.workspace_mode != WorkspaceMode::Compose {
        return None;
    }

    let PickHit::Tile { node } = hit else {
        return None;
    };

    let FocusTarget::Tile { node: focused } = &attention.focus else {
        return None;
    };

    if focused != node || !queries.is_container(node) {
        return None;
    }

    Some(EditorCommand::EnterContainer {
        container: node.clone(),
    })
}

fn connection_command_from_pick_hit(
    session: &EditorSession,
    hit: &PickHit,
) -> Option<EditorCommand> {
    let from = session.connection_source()?.clone();
    let PickHit::Tile { node } = hit else {
        return None;
    };
    if from == *node {
        return None;
    }
    Some(EditorCommand::ConnectTiles {
        from,
        to: node.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::document::MusaicDocument;

    #[test]
    fn miss_pick_clears_focus() {
        let document = MusaicDocument::new_empty();
        let queries = DocumentQueries::new(&document);
        let attention = EditorAttention::new(document.root_surface);
        let session = EditorSession::default();

        assert_eq!(
            classify_board_pick(&queries, &attention, &session, &BoardPickEvent::Miss),
            Some(EditorCommand::ClearSelection)
        );
    }
}
