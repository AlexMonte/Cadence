use bevy::prelude::Resource;

use crate::application::board::{
    BoardFocus, BoardStore, DragPayload, InteractionStore, ResolvedTarget, SelectionStore,
};
use crate::domain::board::{BoardSurfaceId, PlacementVerdict, SlotCoord, TileClass, TileInstanceId};

use super::layout_projection::{LogicalRect, slot_rect};
use super::visual_mapping::{TileVisualKind, visual_kind_for_class};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardCellState {
    Idle,
    Selected,
    GhostValid,
    GhostInvalid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoardCellScene {
    pub board_id: BoardSurfaceId,
    pub coord: SlotCoord,
    pub logical_rect: LogicalRect,
    pub state: BoardCellState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TileScene {
    pub tile_instance_id: TileInstanceId,
    pub piece_id: String,
    pub class: TileClass,
    pub visual_kind: TileVisualKind,
    pub coord: SlotCoord,
    pub logical_rect: LogicalRect,
    pub selected: bool,
    pub openable: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GhostScene {
    pub visual_kind: TileVisualKind,
    pub target_coord: SlotCoord,
    pub logical_rect: LogicalRect,
    pub validity: PlacementVerdict,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectionScene {
    pub selected_slot: Option<SlotCoord>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlayScene;

#[derive(Debug, Clone, PartialEq)]
pub struct BoardScene {
    pub active_board: BoardSurfaceId,
    pub cells: Vec<BoardCellScene>,
    pub tiles: Vec<TileScene>,
    pub ghost: Option<GhostScene>,
    pub selection: SelectionScene,
    pub focus_path: Vec<BoardSurfaceId>,
    pub overlays: Vec<OverlayScene>,
}

#[derive(Debug, Resource, Clone)]
pub struct BoardSceneState {
    pub scene: BoardScene,
}

impl BoardSceneState {
    pub fn from_store(
        store: &BoardStore,
        focus: &BoardFocus,
        selection: &SelectionStore,
        interactions: &InteractionStore,
    ) -> Option<Self> {
        Some(Self {
            scene: project_board_scene(store, focus, selection, interactions)?,
        })
    }
}

pub fn project_board_scene(
    store: &BoardStore,
    focus: &BoardFocus,
    selection: &SelectionStore,
    interactions: &InteractionStore,
) -> Option<BoardScene> {
    let surface = store.surface(&focus.active_board)?;
    let layout = focus.active_layout();
    let selected_slot = selection.selected_slot(&focus.active_board);

    let mut cells = Vec::with_capacity((surface.extent.cols * surface.extent.rows) as usize);
    for row in 0..surface.extent.rows as i32 {
        for col in 0..surface.extent.cols as i32 {
            let coord = SlotCoord::new(col, row);
            let mut state = if selected_slot == Some(coord) {
                BoardCellState::Selected
            } else {
                BoardCellState::Idle
            };

            if let Some(target) = active_ghost_target(interactions)
                && target.board_id == focus.active_board
                && target.slot_coord == coord
            {
                state = if matches!(target.validity, PlacementVerdict::Valid) {
                    BoardCellState::GhostValid
                } else {
                    BoardCellState::GhostInvalid
                };
            }

            cells.push(BoardCellScene {
                board_id: focus.active_board.clone(),
                coord,
                logical_rect: slot_rect(layout, coord),
                state,
            });
        }
    }

    let mut tiles = Vec::new();
    for (coord, tile_id) in surface.occupancy.iter() {
        let tile = store.tile(tile_id)?;
        tiles.push(TileScene {
            tile_instance_id: tile.id.clone(),
            piece_id: tile.piece_id.clone(),
            class: tile.class,
            visual_kind: visual_kind_for_class(tile.class),
            coord: *coord,
            logical_rect: slot_rect(layout, *coord),
            selected: selected_slot == Some(*coord),
            openable: tile.class == TileClass::Container,
        });
    }

    let ghost = active_ghost_target(interactions).map(|target| {
        let (visual_kind, reason) = ghost_visual(interactions, &target);
        GhostScene {
            visual_kind,
            target_coord: target.slot_coord,
            logical_rect: slot_rect(layout, target.slot_coord),
            validity: target.validity,
            reason,
        }
    });

    Some(BoardScene {
        active_board: focus.active_board.clone(),
        cells,
        tiles,
        ghost,
        selection: SelectionScene { selected_slot },
        focus_path: focus.focus_path.clone(),
        overlays: Vec::new(),
    })
}

fn active_ghost_target(interactions: &InteractionStore) -> Option<ResolvedTarget> {
    match &interactions.interaction {
        crate::application::board::BoardInteraction::Dragging(session) => {
            session.resolved_target.clone()
        }
        _ => None,
    }
}

fn ghost_visual(
    interactions: &InteractionStore,
    target: &ResolvedTarget,
) -> (TileVisualKind, Option<String>) {
    let visual = match &interactions.interaction {
        crate::application::board::BoardInteraction::Dragging(session) => match &session.payload {
            DragPayload::NewPiece { class, .. } | DragPayload::ExistingTile { class, .. } => {
                if matches!(target.validity, PlacementVerdict::Valid) {
                    visual_kind_for_class(*class)
                } else {
                    TileVisualKind::InvalidPreview
                }
            }
        },
        _ => TileVisualKind::GhostTile,
    };

    let reason = match &target.validity {
        PlacementVerdict::Valid => None,
        PlacementVerdict::Invalid { reason } => Some(reason.clone()),
    };

    (visual, reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::board::{BoardFocus, SelectionStore};
    use crate::domain::board::{BoardExtent, BoardSurface, BoardSurfaceId, BoardSurfaceKind, SlotCoord, TileClass};
    use std::collections::BTreeMap;

    fn seeded_store() -> (BoardStore, BoardFocus, SelectionStore) {
        let root = BoardSurfaceId::new();
        let mut surfaces = BTreeMap::new();
        surfaces.insert(
            root.clone(),
            BoardSurface::new(root.clone(), BoardSurfaceKind::Flow, BoardExtent::new(4, 4)),
        );
        let mut store = BoardStore {
            root_board: root.clone(),
            surfaces,
            tiles: BTreeMap::new(),
            slot_index: BTreeMap::new(),
        };
        store
            .create_tile(
                root.clone(),
                "cadence.fast".into(),
                TileClass::Transform,
                SlotCoord::new(1, 1),
            )
            .unwrap();
        let focus = BoardFocus::new(root.clone());
        let selection = SelectionStore::new(root);
        (store, focus, selection)
    }

    #[test]
    fn projecting_same_state_twice_yields_equivalent_scene() {
        let (store, focus, selection) = seeded_store();
        let interactions = InteractionStore::default();
        let first = project_board_scene(&store, &focus, &selection, &interactions).unwrap();
        let second = project_board_scene(&store, &focus, &selection, &interactions).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn ghost_preview_does_not_mutate_store() {
        let (store, focus, selection) = seeded_store();
        let before = store.tiles.len();
        let mut interactions = InteractionStore::default();
        super::super::super::application::board::drag_service::start_palette_drag(
            &mut interactions,
            store.root_board.clone(),
            "cadence.fast".into(),
            TileClass::Transform,
        );
        super::super::super::application::board::drag_service::update_drag(
            &mut interactions,
            &store,
            &focus,
            crate::application::board::BoardPointer {
                board_id: store.root_board.clone(),
                board_position: crate::application::board::BoardSpaceVec2 { x: 0.0, y: 0.0 },
            },
        );
        let scene = project_board_scene(&store, &focus, &selection, &interactions).unwrap();
        assert!(scene.ghost.is_some());
        assert_eq!(store.tiles.len(), before);
    }
}
