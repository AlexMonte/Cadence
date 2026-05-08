use crate::domain::board::TileClass;

use super::{BoardFocus, BoardStore, SelectionStore, ViewportState};

pub fn open_container(
    focus: &mut BoardFocus,
    selection: &mut SelectionStore,
    store: &BoardStore,
    tile_id: &crate::domain::board::TileInstanceId,
) -> Result<(), String> {
    let tile = store
        .tile(tile_id)
        .ok_or_else(|| "missing tile for open container".to_string())?;
    if tile.class != TileClass::Container {
        return Err("only container tiles can open local boards".to_string());
    }
    let local_surface = tile
        .local_surface_id
        .clone()
        .ok_or_else(|| "container missing local board surface".to_string())?;
    focus.active_board = local_surface.clone();
    focus.focus_path.push(local_surface.clone());
    focus
        .viewport_by_board
        .entry(local_surface.clone())
        .or_insert_with(ViewportState::default);
    selection
        .selection_by_board
        .entry(local_surface.clone())
        .or_insert(None);
    focus.layout_by_board.entry(local_surface).or_default();
    Ok(())
}

pub fn close_container(focus: &mut BoardFocus) {
    if focus.focus_path.len() > 1 {
        focus.focus_path.pop();
        if let Some(active) = focus.focus_path.last() {
            focus.active_board = active.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::domain::board::{BoardExtent, BoardSurface, BoardSurfaceId, BoardSurfaceKind, TileClass, TileInstance, TileInstanceId};

    use super::*;

    fn root_store_with_container() -> (BoardStore, TileInstanceId, BoardSurfaceId) {
        let root = BoardSurfaceId::new();
        let local = BoardSurfaceId::new();
        let tile_id = TileInstanceId::new();
        let mut surfaces = BTreeMap::new();
        surfaces.insert(
            root.clone(),
            BoardSurface::new(root.clone(), BoardSurfaceKind::Flow, BoardExtent::new(8, 8)),
        );
        surfaces.insert(
            local.clone(),
            BoardSurface::new(
                local.clone(),
                BoardSurfaceKind::ContainerLocal,
                BoardExtent::new(8, 8),
            ),
        );
        let mut store = BoardStore {
            root_board: root.clone(),
            surfaces,
            tiles: BTreeMap::new(),
            slot_index: BTreeMap::new(),
        };
        store
            .place_existing_tile(
                TileInstance {
                    id: tile_id.clone(),
                    piece_id: "cadence.container.basic".into(),
                    class: TileClass::Container,
                    parent_surface: root.clone(),
                    local_surface_id: Some(local.clone()),
                },
                crate::domain::board::SlotCoord::new(0, 0),
            )
            .unwrap();
        (store, tile_id, local)
    }

    #[test]
    fn open_and_close_container_updates_focus_path() {
        let (store, tile_id, local) = root_store_with_container();
        let mut focus = BoardFocus::new(store.root_board.clone());
        let mut selection = SelectionStore::new(store.root_board.clone());

        open_container(&mut focus, &mut selection, &store, &tile_id).unwrap();
        assert_eq!(focus.active_board, local);
        assert_eq!(focus.focus_path.len(), 2);

        close_container(&mut focus);
        assert_eq!(focus.active_board, store.root_board);
        assert_eq!(focus.focus_path.len(), 1);
    }
}
