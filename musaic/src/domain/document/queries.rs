use tessera::prelude::{NodeId, NodeSpatialBindings, SpatialSide};

use crate::domain::board::{BoardSlot, BoardSurfaceId, BoardSurfaceKind, SurfaceLayoutKind};

use super::{
    AuthoredBoardConnection, AuthoredTile, DocumentNode, DocumentNodeKind, MusaicDocument,
    NodeLocation, StackIndex, connections_from_program, neighbor_at_side,
};

pub struct DocumentQueries<'a> {
    pub document: &'a MusaicDocument,
}

impl<'a> DocumentQueries<'a> {
    pub fn new(document: &'a MusaicDocument) -> Self {
        Self { document }
    }

    pub fn active_surface_tiles(
        &self,
        surface: BoardSurfaceId,
    ) -> impl Iterator<Item = &'a AuthoredTile> {
        self.document
            .tiles
            .by_id
            .values()
            .filter(move |tile| tile.placement.map(|placement| placement.surface) == Some(surface))
    }

    pub fn node(&self, node: &NodeId) -> Option<&'a DocumentNode> {
        self.document.graph.node(node)
    }

    pub fn authored_tile(&self, node: &NodeId) -> Option<&'a super::AuthoredTile> {
        self.document.tiles.by_id.get(node)
    }

    pub fn connections_on_surface(&self, surface: BoardSurfaceId) -> Vec<AuthoredBoardConnection> {
        if surface != self.document.root_surface {
            return Vec::new();
        }
        let program = &self.document.tessera.authored_program;
        connections_from_program(program, &self.document.port_endpoints)
            .into_iter()
            .filter(|connection| {
                let from_on_surface = self
                    .authored_tile(&connection.from)
                    .and_then(|tile| tile.placement)
                    .is_some_and(|p| p.surface == surface);
                let to_on_surface = self
                    .authored_tile(&connection.to)
                    .and_then(|tile| tile.placement)
                    .is_some_and(|p| p.surface == surface);
                from_on_surface && to_on_surface
            })
            .collect()
    }

    pub fn node_kind(&self, node: &NodeId) -> Option<&'a DocumentNodeKind> {
        self.node(node).map(|node| &node.kind)
    }

    pub fn is_container(&self, node: &NodeId) -> bool {
        matches!(self.node_kind(node), Some(DocumentNodeKind::Container(_)))
    }

    pub fn container_surface(&self, node: &NodeId) -> Option<BoardSurfaceId> {
        self.document.graph.container_surface(node)
    }

    pub fn surface_kind(&self, surface: BoardSurfaceId) -> Option<BoardSurfaceKind> {
        self.document.surfaces.kind(surface)
    }

    pub fn surface_layout(&self, surface: BoardSurfaceId) -> Option<SurfaceLayoutKind> {
        self.surface_kind(surface).map(|kind| kind.layout_kind())
    }

    pub fn location_of(&self, node: &NodeId) -> Option<NodeLocation> {
        self.document.graph.location_of(node)
    }

    pub fn node_at_board_slot(
        &self,
        surface: BoardSurfaceId,
        slot: crate::domain::board::BoardSlot,
    ) -> Option<NodeId> {
        self.document.graph.node_at_board_slot(surface, slot)
    }

    pub fn contains_node(&self, node: &NodeId) -> bool {
        self.document.graph.contains_node(node)
    }

    pub fn has_surface(&self, surface: BoardSurfaceId) -> bool {
        self.document.surfaces.contains(surface)
    }

    pub fn is_board_slot_empty(&self, surface: BoardSurfaceId, slot: BoardSlot) -> bool {
        self.document.graph.is_board_slot_empty(surface, slot)
    }

    pub fn node_at_stack_index(
        &self,
        surface: BoardSurfaceId,
        index: StackIndex,
    ) -> Option<NodeId> {
        self.document.graph.node_at_stack_index(surface, index)
    }

    pub fn is_stack_index_empty(&self, surface: BoardSurfaceId, index: StackIndex) -> bool {
        self.document.graph.is_stack_index_empty(surface, index)
    }

    pub fn neighbor_at_side(&self, node: &NodeId, side: SpatialSide) -> Option<NodeId> {
        neighbor_at_side(&self.document.tessera.authored_program, node, side)
    }
}
