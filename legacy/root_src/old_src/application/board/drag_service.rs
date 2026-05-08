use crate::application::board::placement_service::validate_placement;
use crate::domain::board::{PlacementVerdict, TileInstanceId};

use super::{
    BoardPointer, BoardStore, DragPayload, DragSession, DragSource, InteractionStore,
    ResolvedTarget, SelectionStore, slot_resolution::resolve_slot,
};

pub fn start_palette_drag(
    interactions: &mut InteractionStore,
    board_id: crate::domain::board::BoardSurfaceId,
    piece_id: String,
    class: crate::domain::board::TileClass,
) {
    interactions.interaction = super::BoardInteraction::Dragging(DragSession {
        source: DragSource::Palette(piece_id.clone()),
        payload: DragPayload::NewPiece { piece_id, class },
        origin: board_id,
        current_pointer: None,
        resolved_target: None,
        preview_offset: None,
    });
}

pub fn start_tile_drag(
    interactions: &mut InteractionStore,
    store: &BoardStore,
    tile_id: TileInstanceId,
) -> Result<(), String> {
    let tile = store
        .tile(&tile_id)
        .ok_or_else(|| "missing tile for drag".to_string())?;

    interactions.interaction = super::BoardInteraction::Dragging(DragSession {
        source: DragSource::BoardTile(tile_id.clone()),
        payload: DragPayload::ExistingTile {
            tile_id,
            class: tile.class,
        },
        origin: tile.parent_surface.clone(),
        current_pointer: None,
        resolved_target: None,
        preview_offset: None,
    });
    Ok(())
}

pub fn update_drag(
    interactions: &mut InteractionStore,
    store: &BoardStore,
    focus: &super::BoardFocus,
    pointer: BoardPointer,
) {
    let super::BoardInteraction::Dragging(session) = &mut interactions.interaction else {
        return;
    };

    session.current_pointer = Some(pointer.clone());
    let slot = resolve_slot(focus.active_layout(), pointer.board_position);
    let class = match &session.payload {
        DragPayload::NewPiece { class, .. } | DragPayload::ExistingTile { class, .. } => *class,
    };
    let ignore_tile = match &session.payload {
        DragPayload::ExistingTile { tile_id, .. } => Some(tile_id),
        DragPayload::NewPiece { .. } => None,
    };
    let verdict = validate_placement(store, &pointer.board_id, slot, class, ignore_tile);
    session.resolved_target = Some(ResolvedTarget {
        board_id: pointer.board_id,
        slot_coord: slot,
        validity: verdict,
    });
}

pub fn commit_drag(
    interactions: &mut InteractionStore,
    store: &mut BoardStore,
    selection: &mut SelectionStore,
) -> Result<(), String> {
    let super::BoardInteraction::Dragging(session) = &interactions.interaction else {
        return Ok(());
    };

    let target = session
        .resolved_target
        .clone()
        .ok_or_else(|| "drag has no resolved target".to_string())?;

    if !matches!(target.validity, PlacementVerdict::Valid) {
        interactions.interaction = super::BoardInteraction::Idle;
        return Err("cannot commit invalid drag target".to_string());
    }

    match &session.payload {
        DragPayload::NewPiece { piece_id, class } => {
            store.create_tile(
                target.board_id.clone(),
                piece_id.clone(),
                *class,
                target.slot_coord,
            )?;
        }
        DragPayload::ExistingTile { tile_id, .. } => {
            store.move_tile(tile_id, target.board_id.clone(), target.slot_coord)?;
        }
    }

    selection.set_selected_slot(target.board_id.clone(), Some(target.slot_coord));
    interactions.interaction = super::BoardInteraction::Idle;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::application::board::{BoardFocus, SelectionStore};
    use crate::domain::board::{
        BoardExtent, BoardSurface, BoardSurfaceId, BoardSurfaceKind, SlotCoord, TileClass,
    };

    use super::*;

    fn empty_store() -> (BoardStore, BoardFocus, SelectionStore) {
        let root = BoardSurfaceId::new();
        let mut surfaces = BTreeMap::new();
        surfaces.insert(
            root.clone(),
            BoardSurface::new(root.clone(), BoardSurfaceKind::Flow, BoardExtent::new(8, 8)),
        );
        let store = BoardStore {
            root_board: root.clone(),
            surfaces,
            tiles: BTreeMap::new(),
            slot_index: BTreeMap::new(),
        };
        let focus = BoardFocus::new(root.clone());
        let selection = SelectionStore::new(root);
        (store, focus, selection)
    }

    #[test]
    fn palette_drag_commit_creates_new_tile_without_pre_mutating_store() {
        let (mut store, focus, mut selection) = empty_store();
        let mut interactions = InteractionStore::default();
        start_palette_drag(
            &mut interactions,
            store.root_board.clone(),
            "cadence.fast".into(),
            TileClass::Transform,
        );
        assert!(store.tiles.is_empty());

        update_drag(
            &mut interactions,
            &store,
            &focus,
            BoardPointer {
                board_id: store.root_board.clone(),
                board_position: crate::application::board::BoardSpaceVec2 { x: 0.0, y: 0.0 },
            },
        );
        commit_drag(&mut interactions, &mut store, &mut selection).unwrap();

        assert_eq!(store.tiles.len(), 1);
        assert_eq!(
            selection.selected_slot(&store.root_board),
            Some(SlotCoord::new(0, 0))
        );
    }

    #[test]
    fn canceling_drag_leaves_store_unchanged() {
        let (store, ..) = empty_store();
        let mut interactions = InteractionStore::default();
        start_palette_drag(
            &mut interactions,
            store.root_board.clone(),
            "cadence.fast".into(),
            TileClass::Transform,
        );
        interactions.interaction = super::super::BoardInteraction::Idle;
        assert!(store.tiles.is_empty());
    }
}
