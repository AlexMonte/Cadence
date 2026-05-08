use std::collections::BTreeMap;

use bevy::prelude::Resource;

use crate::adapter::catalog::classify_piece::classify_piece_id;
use crate::domain::board::{
    BoardExtent, BoardSurface, BoardSurfaceId, BoardSurfaceKind, PlacementRule, SlotCoord,
    TileClass, TileInstance, TileInstanceId,
};
use crate::infrastructure::dto::{EditorSyncSnapshotDto, GraphNodeDto, PatternItemKind, PatternRoot};

#[derive(Debug, Clone, Resource)]
pub struct BoardStore {
    pub root_board: BoardSurfaceId,
    pub surfaces: BTreeMap<BoardSurfaceId, BoardSurface>,
    pub tiles: BTreeMap<TileInstanceId, TileInstance>,
    pub slot_index: BTreeMap<TileInstanceId, SlotCoord>,
}

impl BoardStore {
    pub fn from_editor_sync(sync: &EditorSyncSnapshotDto) -> Self {
        let root_board = BoardSurfaceId::new();
        let mut surfaces = BTreeMap::new();
        surfaces.insert(
            root_board.clone(),
            BoardSurface::new(
                root_board.clone(),
                BoardSurfaceKind::Flow,
                BoardExtent::new(sync.graph.cols, sync.graph.rows),
            ),
        );

        let mut store = Self {
            root_board: root_board.clone(),
            surfaces,
            tiles: BTreeMap::new(),
            slot_index: BTreeMap::new(),
        };

        for node in &sync.graph.nodes {
            let slot = SlotCoord::new(node.position.col, node.position.row);
            store.insert_graph_node(&root_board, node, slot);
        }

        store
    }

    fn insert_graph_node(
        &mut self,
        parent_surface: &BoardSurfaceId,
        node: &GraphNodeDto,
        slot: SlotCoord,
    ) {
        let class = classify_piece_id(node.piece_id.as_str());
        let tile_id = TileInstanceId::new();
        let local_surface_id = if class == TileClass::Container {
            let local_id = BoardSurfaceId::new();
            self.surfaces.insert(
                local_id.clone(),
                BoardSurface::new(
                    local_id.clone(),
                    BoardSurfaceKind::ContainerLocal,
                    BoardExtent::new(8, 8),
                ),
            );

            if let Some(pattern) = &node.pattern_source {
                for root in &pattern.roots {
                    self.insert_pattern_root(&local_id, root);
                }
            }

            Some(local_id)
        } else {
            None
        };

        let tile = TileInstance {
            id: tile_id.clone(),
            piece_id: node.piece_id.clone(),
            class,
            parent_surface: parent_surface.clone(),
            local_surface_id,
        };

        self.place_existing_tile(tile, slot)
            .expect("seed graph nodes must satisfy board invariants");
    }

    fn insert_pattern_root(&mut self, surface_id: &BoardSurfaceId, root: &PatternRoot) {
        let piece_id = match root.container.kind {
            crate::infrastructure::dto::PatternContainerKind::Basic => "cadence.container.basic",
            crate::infrastructure::dto::PatternContainerKind::Subdivide => {
                "cadence.container.subdivide"
            }
            crate::infrastructure::dto::PatternContainerKind::Alternate => {
                "cadence.container.alternate"
            }
            crate::infrastructure::dto::PatternContainerKind::Parallel => {
                "cadence.container.parallel"
            }
        };

        let tile_id = TileInstanceId::new();
        let nested_local = BoardSurfaceId::new();
        self.surfaces.insert(
            nested_local.clone(),
            BoardSurface::new(
                nested_local.clone(),
                BoardSurfaceKind::ContainerLocal,
                BoardExtent::new(8, 8),
            ),
        );

        let tile = TileInstance {
            id: tile_id,
            piece_id: piece_id.to_string(),
            class: TileClass::Container,
            parent_surface: surface_id.clone(),
            local_surface_id: Some(nested_local.clone()),
        };

        self.place_existing_tile(tile, SlotCoord::new(root.position.col, root.position.row))
            .expect("pattern roots should fit container-local surface");

        for item in &root.container.items {
            match &item.kind {
                PatternItemKind::Atom(atom) => {
                    let piece_id = match atom {
                        crate::infrastructure::dto::PatternAtom::Note { .. } => "cadence.atom.note",
                        crate::infrastructure::dto::PatternAtom::Scalar { .. } => {
                            "cadence.atom.scalar"
                        }
                        crate::infrastructure::dto::PatternAtom::Rest => "cadence.atom.rest",
                        crate::infrastructure::dto::PatternAtom::Operator { operator } => match operator {
                            crate::infrastructure::dto::PatternOperatorKind::Elongation => {
                                "cadence.atom.operator.elongation"
                            }
                            crate::infrastructure::dto::PatternOperatorKind::PitchShift => {
                                "cadence.atom.operator.pitch_shift"
                            }
                            crate::infrastructure::dto::PatternOperatorKind::Slow => {
                                "cadence.atom.operator.slow"
                            }
                            crate::infrastructure::dto::PatternOperatorKind::Fast => {
                                "cadence.atom.operator.fast"
                            }
                        },
                    };

                    let atom_tile = TileInstance {
                        id: TileInstanceId::new(),
                        piece_id: piece_id.to_string(),
                        class: TileClass::Atom,
                        parent_surface: nested_local.clone(),
                        local_surface_id: None,
                    };

                    self.place_existing_tile(
                        atom_tile,
                        SlotCoord::new(item.position.col, item.position.row),
                    )
                    .expect("pattern atoms should fit local surface");
                }
                PatternItemKind::Container(container) => {
                    let piece_id = match container.kind {
                        crate::infrastructure::dto::PatternContainerKind::Basic => {
                            "cadence.container.basic"
                        }
                        crate::infrastructure::dto::PatternContainerKind::Subdivide => {
                            "cadence.container.subdivide"
                        }
                        crate::infrastructure::dto::PatternContainerKind::Alternate => {
                            "cadence.container.alternate"
                        }
                        crate::infrastructure::dto::PatternContainerKind::Parallel => {
                            "cadence.container.parallel"
                        }
                    };

                    let local = BoardSurfaceId::new();
                    self.surfaces.insert(
                        local.clone(),
                        BoardSurface::new(
                            local.clone(),
                            BoardSurfaceKind::ContainerLocal,
                            BoardExtent::new(8, 8),
                        ),
                    );
                    let nested = TileInstance {
                        id: TileInstanceId::new(),
                        piece_id: piece_id.to_string(),
                        class: TileClass::Container,
                        parent_surface: nested_local.clone(),
                        local_surface_id: Some(local),
                    };
                    self.place_existing_tile(
                        nested,
                        SlotCoord::new(item.position.col, item.position.row),
                    )
                    .expect("nested pattern containers should fit local surface");
                }
            }
        }
    }

    pub fn place_existing_tile(
        &mut self,
        tile: TileInstance,
        slot: SlotCoord,
    ) -> Result<(), String> {
        let surface = self
            .surfaces
            .get_mut(&tile.parent_surface)
            .ok_or_else(|| "missing parent surface".to_string())?;

        if !surface.extent.contains(slot) {
            return Err("slot out of bounds".to_string());
        }
        if !PlacementRule::accepts(surface.kind, tile.class) {
            return Err("tile class not accepted on this surface".to_string());
        }
        if surface.occupancy.get(slot).is_some() {
            return Err("slot already occupied".to_string());
        }

        let tile_id = tile.id.clone();
        surface.occupancy.insert(slot, tile_id.clone());
        self.slot_index.insert(tile_id.clone(), slot);
        self.tiles.insert(tile_id, tile);
        Ok(())
    }

    pub fn create_tile(
        &mut self,
        parent_surface: BoardSurfaceId,
        piece_id: String,
        class: TileClass,
        slot: SlotCoord,
    ) -> Result<TileInstanceId, String> {
        let local_surface_id = if class == TileClass::Container {
            Some(self.create_local_surface()?)
        } else {
            None
        };
        let tile = TileInstance {
            id: TileInstanceId::new(),
            piece_id,
            class,
            parent_surface,
            local_surface_id,
        };
        let tile_id = tile.id.clone();
        self.place_existing_tile(tile, slot)?;
        Ok(tile_id)
    }

    pub fn move_tile(
        &mut self,
        tile_id: &TileInstanceId,
        target_surface: BoardSurfaceId,
        target_slot: SlotCoord,
    ) -> Result<(), String> {
        let tile = self
            .tiles
            .get(tile_id)
            .cloned()
            .ok_or_else(|| "drag tile missing from board store".to_string())?;
        let source_slot = self
            .slot_index
            .get(tile_id)
            .copied()
            .ok_or_else(|| "source slot missing".to_string())?;

        if tile.parent_surface == target_surface && source_slot == target_slot {
            return Ok(());
        }

        if tile.class == TileClass::Container
            && let Some(local_surface_id) = &tile.local_surface_id
            && self.is_descendant_surface(local_surface_id, &target_surface)
        {
            return Err("container cannot move into its own descendant surface".to_string());
        }

        let source_surface_id = tile.parent_surface.clone();
        let source_surface = self
            .surfaces
            .get_mut(&source_surface_id)
            .ok_or_else(|| "source surface missing".to_string())?;
        source_surface.occupancy.remove(source_slot);
        self.slot_index.remove(tile_id);
        self.tiles.remove(tile_id);

        let mut moved = tile;
        moved.parent_surface = target_surface.clone();

        match self.place_existing_tile(moved.clone(), target_slot) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.place_existing_tile(
                    TileInstance {
                        parent_surface: source_surface_id,
                        ..moved
                    },
                    source_slot,
                )
                .expect("failed move should restore previous tile placement");
                Err(error)
            }
        }
    }

    pub fn delete_tile_subtree(&mut self, tile_id: &TileInstanceId) -> Result<Vec<TileInstanceId>, String> {
        let tile = self
            .tiles
            .get(tile_id)
            .cloned()
            .ok_or_else(|| "missing tile for deletion".to_string())?;
        let mut deleted = Vec::new();

        if let Some(local_surface_id) = &tile.local_surface_id {
            let child_tiles: Vec<TileInstanceId> = self
                .surface(local_surface_id)
                .ok_or_else(|| "missing local surface for deletion".to_string())?
                .occupancy
                .iter()
                .map(|(_, child_id)| child_id.clone())
                .collect();
            for child_id in child_tiles {
                deleted.extend(self.delete_tile_subtree(&child_id)?);
            }
            self.surfaces.remove(local_surface_id);
        }

        let slot = self
            .slot_index
            .remove(tile_id)
            .ok_or_else(|| "missing slot for deletion".to_string())?;
        let surface = self
            .surfaces
            .get_mut(&tile.parent_surface)
            .ok_or_else(|| "missing parent surface for deletion".to_string())?;
        surface.occupancy.remove(slot);
        self.tiles.remove(tile_id);
        deleted.push(tile_id.clone());
        Ok(deleted)
    }

    pub fn is_descendant_surface(
        &self,
        ancestor_surface: &BoardSurfaceId,
        candidate_surface: &BoardSurfaceId,
    ) -> bool {
        if ancestor_surface == candidate_surface {
            return true;
        }

        self.surfaces.get(ancestor_surface).is_some_and(|surface| {
            surface.occupancy.iter().any(|(_, tile_id)| {
                self.tiles.get(tile_id).and_then(|tile| tile.local_surface_id.as_ref()).is_some_and(
                    |child_surface| self.is_descendant_surface(child_surface, candidate_surface),
                )
            })
        })
    }

    fn create_local_surface(&mut self) -> Result<BoardSurfaceId, String> {
        let local_id = BoardSurfaceId::new();
        let previous = self.surfaces.insert(
            local_id.clone(),
            BoardSurface::new(
                local_id.clone(),
                BoardSurfaceKind::ContainerLocal,
                BoardExtent::new(8, 8),
            ),
        );
        if previous.is_some() {
            return Err("generated duplicate local surface id".to_string());
        }
        Ok(local_id)
    }

    pub fn surface(&self, id: &BoardSurfaceId) -> Option<&BoardSurface> {
        self.surfaces.get(id)
    }

    pub fn tile(&self, id: &TileInstanceId) -> Option<&TileInstance> {
        self.tiles.get(id)
    }

    pub fn slot_of(&self, id: &TileInstanceId) -> Option<SlotCoord> {
        self.slot_index.get(id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::board::PlacementVerdict;
    use crate::application::board::placement_service::validate_placement;

    fn empty_store() -> BoardStore {
        let root = BoardSurfaceId::new();
        let mut surfaces = BTreeMap::new();
        surfaces.insert(
            root.clone(),
            BoardSurface::new(root.clone(), BoardSurfaceKind::Flow, BoardExtent::new(8, 8)),
        );
        BoardStore {
            root_board: root,
            surfaces,
            tiles: BTreeMap::new(),
            slot_index: BTreeMap::new(),
        }
    }

    #[test]
    fn placing_container_creates_local_surface() {
        let mut store = empty_store();
        let tile_id = store
            .create_tile(
                store.root_board.clone(),
                "cadence.container.basic".into(),
                TileClass::Container,
                SlotCoord::new(0, 0),
            )
            .unwrap();
        let tile = store.tile(&tile_id).unwrap();
        let local = tile.local_surface_id.clone().unwrap();
        assert_eq!(store.surface(&local).unwrap().kind, BoardSurfaceKind::ContainerLocal);
    }

    #[test]
    fn atom_is_rejected_on_flow_board() {
        let store = empty_store();
        let verdict = validate_placement(
            &store,
            &store.root_board,
            SlotCoord::new(0, 0),
            TileClass::Atom,
            None,
        );
        assert!(matches!(verdict, PlacementVerdict::Invalid { .. }));
    }

    #[test]
    fn atom_is_allowed_in_container_local_board() {
        let mut store = empty_store();
        let container_id = store
            .create_tile(
                store.root_board.clone(),
                "cadence.container.basic".into(),
                TileClass::Container,
                SlotCoord::new(0, 0),
            )
            .unwrap();
        let local_surface = store
            .tile(&container_id)
            .unwrap()
            .local_surface_id
            .clone()
            .unwrap();
        let verdict = validate_placement(
            &store,
            &local_surface,
            SlotCoord::new(1, 1),
            TileClass::Atom,
            None,
        );
        assert!(matches!(verdict, PlacementVerdict::Valid));
    }

    #[test]
    fn moving_tile_updates_occupancy_atomically() {
        let mut store = empty_store();
        let tile_id = store
            .create_tile(
                store.root_board.clone(),
                "cadence.fast".into(),
                TileClass::Transform,
                SlotCoord::new(0, 0),
            )
            .unwrap();
        store
            .move_tile(&tile_id, store.root_board.clone(), SlotCoord::new(2, 0))
            .unwrap();
        let surface = store.surface(&store.root_board).unwrap();
        assert!(surface.occupancy.get(SlotCoord::new(0, 0)).is_none());
        assert_eq!(surface.occupancy.get(SlotCoord::new(2, 0)), Some(&tile_id));
    }

    #[test]
    fn deleting_container_cascades_subtree() {
        let mut store = empty_store();
        let container_id = store
            .create_tile(
                store.root_board.clone(),
                "cadence.container.basic".into(),
                TileClass::Container,
                SlotCoord::new(0, 0),
            )
            .unwrap();
        let local_surface = store
            .tile(&container_id)
            .unwrap()
            .local_surface_id
            .clone()
            .unwrap();
        let atom_id = store
            .create_tile(
                local_surface.clone(),
                "cadence.atom.note".into(),
                TileClass::Atom,
                SlotCoord::new(0, 0),
            )
            .unwrap();

        let deleted = store.delete_tile_subtree(&container_id).unwrap();
        assert!(deleted.contains(&container_id));
        assert!(deleted.contains(&atom_id));
        assert!(store.tile(&container_id).is_none());
        assert!(store.tile(&atom_id).is_none());
        assert!(store.surface(&local_surface).is_none());
    }
}
