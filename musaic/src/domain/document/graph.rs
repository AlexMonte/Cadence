use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tessera::prelude::{Arrangement, ContainerId, NodeId};

use crate::domain::board::{
    BoardSlot, BoardSurface, BoardSurfaceId, BoardSurfaceKind, BoardSurfaces,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DocumentEdgeId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TilePrototypeId(pub u64);

/// Authored document structure for the board language.
///
/// Important domain split:
/// - `placements` are authored tile occupancy: a tile/node exists at one board slot.
/// - Empty board slots are not document nodes. They are editor/render affordances.
/// - Container nodes own child board surfaces through `container_surfaces`.
///
/// In other words, a board surface is an authoring space, a slot is an address,
/// and a document node placed at that address is the AST/program structure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentGraph {
    nodes: BTreeMap<NodeId, DocumentNode>,
    board_placements: BTreeMap<(BoardSurfaceId, BoardSlot), NodeId>,
    stack_placements: BTreeMap<(BoardSurfaceId, StackIndex), NodeId>,
    node_locations: BTreeMap<NodeId, NodeLocation>,
    container_surfaces: BTreeMap<NodeId, BoardSurfaceId>,
    touching_edges: BTreeMap<NodeId, BTreeSet<DocumentEdgeId>>,
    next_node_id: u64,
    next_surface_id: u64,
}

impl Default for DocumentGraph {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            board_placements: BTreeMap::new(),
            stack_placements: BTreeMap::new(),
            node_locations: BTreeMap::new(),
            container_surfaces: BTreeMap::new(),
            touching_edges: BTreeMap::new(),
            next_node_id: 1,
            next_surface_id: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentNode {
    pub id: NodeId,
    pub kind: DocumentNodeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentNodeKind {
    Tile(TileNode),
    Atom(AtomNode),
    Container(ContainerNode),
    Output(OutputNode),
    TrickInstance(TrickInstanceNode),
    Arrangement(ArrangementNode),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangementNode {
    pub arrangement: Arrangement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileNode {
    pub prototype: TilePrototypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomNode {
    pub atom: AtomValue,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerNode {
    pub kind: ContainerKind,
    pub local_surface: BoardSurfaceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputNode {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrickInstanceNode {
    pub prototype: TilePrototypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerKind {
    Sequence,
    Subdivision,
    Alternating,
    Parallel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomValue {
    NoteName(NoteName),
    Octave(i8),
    Accidental(Accidental),
    Operator(OperatorValue),
    Number(i32),
    Rest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoteName {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accidental {
    Sharp,
    Flat,
    Natural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperatorValue {
    Power,
    At,
    Multiply,
    Divide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StackIndex(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlacementAddress {
    BoardSlot(BoardSlot),
    StackIndex(StackIndex),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeLocation {
    pub surface: BoardSurfaceId,
    pub address: PlacementAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileSpawnKind {
    Tile { prototype: TilePrototypeId },
    Atom { atom: AtomValue },
    Container { kind: ContainerKind },
    Output { name: String },
    TrickInstance { prototype: TilePrototypeId },
}

impl DocumentGraph {
    pub fn nodes(&self) -> impl Iterator<Item = &DocumentNode> {
        self.nodes.values()
    }

    /// Returns authored nodes placed on a surface.
    ///
    /// This intentionally does not synthesize empty slots. Empty slots belong to
    /// the editor/render layer as possible placement targets, not to the document
    /// graph as program nodes.
    pub fn nodes_on_surface(&self, surface: BoardSurfaceId) -> Vec<(NodeLocation, &DocumentNode)> {
        self.node_locations
            .iter()
            .filter_map(|(node_id, location)| {
                if location.surface == surface {
                    self.nodes.get(node_id).map(|node| (*location, node))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn contains_node(&self, node: &NodeId) -> bool {
        self.nodes.contains_key(node)
    }

    pub fn node(&self, node: &NodeId) -> Option<&DocumentNode> {
        self.nodes.get(node)
    }

    pub fn location_of(&self, node: &NodeId) -> Option<NodeLocation> {
        self.node_locations.get(node).copied()
    }

    /// Returns the authored node occupying this address, if any.
    ///
    /// `None` means the slot is empty and may be used as a placement target; it
    /// does not mean an "empty tile" exists in the document.
    pub fn node_at_board_slot(&self, surface: BoardSurfaceId, slot: BoardSlot) -> Option<NodeId> {
        self.board_placements.get(&(surface, slot)).cloned()
    }

    pub fn node_at_stack_index(
        &self,
        surface: BoardSurfaceId,
        index: StackIndex,
    ) -> Option<NodeId> {
        self.stack_placements.get(&(surface, index)).cloned()
    }

    pub fn is_board_slot_empty(&self, surface: BoardSurfaceId, slot: BoardSlot) -> bool {
        self.node_at_board_slot(surface, slot).is_none()
    }

    pub fn is_stack_index_empty(&self, surface: BoardSurfaceId, index: StackIndex) -> bool {
        self.node_at_stack_index(surface, index).is_none()
    }

    /// Returns the child authoring surface owned by a container node.
    pub fn container_surface(&self, container: &NodeId) -> Option<BoardSurfaceId> {
        self.container_surfaces.get(container).copied()
    }

    pub fn container_node_for_surface(&self, surface: BoardSurfaceId) -> Option<NodeId> {
        self.container_surfaces
            .iter()
            .find(|(_, local)| **local == surface)
            .map(|(node, _)| node.clone())
    }

    /// Inserts a new authored tile/node into a surface slot.
    ///
    /// This is the document-side authorship operation: the slot is merely an
    /// address until this method succeeds. Container insertion also creates the
    /// container-local child surface that nested authoring will happen inside.
    pub fn insert_tile(
        &mut self,
        surfaces: &mut BoardSurfaces,
        surface: BoardSurfaceId,
        address: PlacementAddress,
        spawn: TileSpawnKind,
    ) -> Result<NodeId, DocumentGraphError> {
        if !surfaces.contains(surface) {
            return Err(DocumentGraphError::MissingSurface(surface));
        }
        if !self.is_address_empty(surface, address) {
            return Err(DocumentGraphError::OccupiedAddress { surface, address });
        }

        let node_id = self.alloc_node_id();
        let kind = match spawn {
            TileSpawnKind::Tile { prototype } => DocumentNodeKind::Tile(TileNode { prototype }),
            TileSpawnKind::Atom { atom } => DocumentNodeKind::Atom(AtomNode { atom }),
            TileSpawnKind::Container { kind } => {
                let local_surface = self.alloc_surface_id();
                let container_id = ContainerId::new(node_id.0.clone());
                surfaces
                    .insert(BoardSurface {
                        id: local_surface,
                        kind: BoardSurfaceKind::ContainerStack {
                            container: container_id,
                        },
                    })
                    .map_err(DocumentGraphError::Surface)?;
                self.container_surfaces
                    .insert(node_id.clone(), local_surface);
                DocumentNodeKind::Container(ContainerNode {
                    kind,
                    local_surface,
                })
            }
            TileSpawnKind::Output { name } => DocumentNodeKind::Output(OutputNode { name }),
            TileSpawnKind::TrickInstance { prototype } => {
                DocumentNodeKind::TrickInstance(TrickInstanceNode { prototype })
            }
        };

        let node = DocumentNode {
            id: node_id.clone(),
            kind,
        };
        self.nodes.insert(node_id.clone(), node);
        match address {
            PlacementAddress::BoardSlot(slot) => {
                self.board_placements
                    .insert((surface, slot), node_id.clone());
            }
            PlacementAddress::StackIndex(index) => {
                self.stack_placements
                    .insert((surface, index), node_id.clone());
            }
        }
        self.node_locations
            .insert(node_id.clone(), NodeLocation { surface, address });
        Ok(node_id)
    }

    pub fn delete_subtree(
        &mut self,
        root: &NodeId,
        _surfaces: &mut BoardSurfaces,
    ) -> Result<DeletedSubtree, DocumentGraphError> {
        if !self.contains_node(root) {
            return Err(DocumentGraphError::MissingNode(root.clone()));
        }

        let mut deleted = DeletedSubtree::default();
        self.collect_subtree(root, &mut deleted.nodes);

        for node in deleted.nodes.iter() {
            if let Some(location) = self.node_locations.remove(node) {
                match location.address {
                    PlacementAddress::BoardSlot(slot) => {
                        self.board_placements.remove(&(location.surface, slot));
                    }
                    PlacementAddress::StackIndex(index) => {
                        self.stack_placements.remove(&(location.surface, index));
                    }
                }
            }
            if let Some(edges) = self.touching_edges.remove(node) {
                deleted.edges.extend(edges);
            }
            if let Some(local_surface) = self.container_surfaces.remove(node) {
                deleted.surfaces.insert(local_surface);
            }
            self.nodes.remove(node);
        }

        Ok(deleted)
    }

    fn collect_subtree(&self, root: &NodeId, out: &mut BTreeSet<NodeId>) {
        if !out.insert(root.clone()) {
            return;
        }
        let Some(local_surface) = self.container_surfaces.get(root).copied() else {
            return;
        };

        let children = self
            .node_locations
            .iter()
            .filter_map(|(node, location)| {
                if location.surface == local_surface {
                    Some(node.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for child in children {
            self.collect_subtree(&child, out);
        }
    }

    pub fn allocate_node_id(&mut self) -> NodeId {
        self.alloc_node_id()
    }

    pub fn reserve_node_id(&mut self, id: &NodeId) {
        if let Some(stem) = id.0.strip_prefix("doc_") {
            if let Ok(value) = stem.parse::<u64>() {
                if value >= self.next_node_id {
                    self.next_node_id = value + 1;
                }
            }
        }
    }

    pub fn reserve_surface_id(&mut self, id: BoardSurfaceId) {
        if id.0 >= self.next_surface_id {
            self.next_surface_id = id.0 + 1;
        }
    }

    /// Re-inserts a node exactly as captured (preserves ids and container surfaces).
    pub fn restore_node_unchanged(
        &mut self,
        _surfaces: &mut BoardSurfaces,
        node: DocumentNode,
        location: NodeLocation,
    ) -> Result<(), DocumentGraphError> {
        let node_id = node.id.clone();
        if self.contains_node(&node_id) {
            return Err(DocumentGraphError::MissingNode(node_id));
        }
        if let DocumentNodeKind::Container(container) = &node.kind {
            self.container_surfaces
                .insert(node_id.clone(), container.local_surface);
        }
        self.nodes.insert(node_id.clone(), node);
        match location.address {
            PlacementAddress::BoardSlot(slot) => {
                self.board_placements
                    .insert((location.surface, slot), node_id.clone());
            }
            PlacementAddress::StackIndex(index) => {
                self.stack_placements
                    .insert((location.surface, index), node_id.clone());
            }
        }
        self.node_locations.insert(node_id, location);
        Ok(())
    }

    pub fn link_container_surface(&mut self, container: NodeId, local_surface: BoardSurfaceId) {
        self.container_surfaces.insert(container, local_surface);
    }

    pub fn insert_tile_at_id(
        &mut self,
        surfaces: &mut BoardSurfaces,
        surface: BoardSurfaceId,
        address: PlacementAddress,
        node_id: NodeId,
        spawn: TileSpawnKind,
    ) -> Result<(), DocumentGraphError> {
        if !surfaces.contains(surface) {
            return Err(DocumentGraphError::MissingSurface(surface));
        }
        if !self.is_address_empty(surface, address) {
            return Err(DocumentGraphError::OccupiedAddress { surface, address });
        }

        let kind = match spawn {
            TileSpawnKind::Tile { prototype } => DocumentNodeKind::Tile(TileNode { prototype }),
            TileSpawnKind::Atom { atom } => DocumentNodeKind::Atom(AtomNode { atom }),
            TileSpawnKind::Container { kind } => {
                let local_surface = self.alloc_surface_id();
                let container_id = ContainerId::new(node_id.0.clone());
                surfaces
                    .insert(BoardSurface {
                        id: local_surface,
                        kind: BoardSurfaceKind::ContainerStack {
                            container: container_id,
                        },
                    })
                    .map_err(DocumentGraphError::Surface)?;
                self.container_surfaces
                    .insert(node_id.clone(), local_surface);
                DocumentNodeKind::Container(ContainerNode {
                    kind,
                    local_surface,
                })
            }
            TileSpawnKind::Output { name } => DocumentNodeKind::Output(OutputNode { name }),
            TileSpawnKind::TrickInstance { prototype } => {
                DocumentNodeKind::TrickInstance(TrickInstanceNode { prototype })
            }
        };

        let node = DocumentNode {
            id: node_id.clone(),
            kind,
        };
        self.nodes.insert(node_id.clone(), node);
        match address {
            PlacementAddress::BoardSlot(slot) => {
                self.board_placements
                    .insert((surface, slot), node_id.clone());
            }
            PlacementAddress::StackIndex(index) => {
                self.stack_placements
                    .insert((surface, index), node_id.clone());
            }
        }
        self.node_locations
            .insert(node_id, NodeLocation { surface, address });
        Ok(())
    }

    fn alloc_node_id(&mut self) -> NodeId {
        let id = NodeId::new(format!("doc_{}", self.next_node_id));
        self.next_node_id += 1;
        id
    }

    fn is_address_empty(&self, surface: BoardSurfaceId, address: PlacementAddress) -> bool {
        match address {
            PlacementAddress::BoardSlot(slot) => self.is_board_slot_empty(surface, slot),
            PlacementAddress::StackIndex(index) => self.is_stack_index_empty(surface, index),
        }
    }

    fn alloc_surface_id(&mut self) -> BoardSurfaceId {
        let id = BoardSurfaceId(self.next_surface_id);
        self.next_surface_id += 1;
        id
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedSubtree {
    pub nodes: BTreeSet<NodeId>,
    pub surfaces: BTreeSet<BoardSurfaceId>,
    pub edges: BTreeSet<DocumentEdgeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentGraphError {
    MissingSurface(BoardSurfaceId),
    MissingNode(NodeId),
    OccupiedAddress {
        surface: BoardSurfaceId,
        address: PlacementAddress,
    },
    Surface(crate::domain::board::BoardSurfaceError),
}
