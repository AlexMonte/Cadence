use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use tessera::prelude::{AuthoredTesseraProgram, NodeId};

pub mod connection_policy;
pub mod export;
pub mod footprint;
pub mod graph;
pub mod patch;
pub mod port_endpoints;
pub mod queries;
pub mod tessera_sync;
pub use connection_policy::{AuthoredEdge, ConnectionPolicyError, authorize_connection};
pub use export::{
    DocumentBoardExport, export_document_to_board, finish_board_export,
    map_tessera_container_kind_to_document,
};
pub use footprint::{RootBoardTileKind, root_board_tile_footprint};
pub use graph::{
    Accidental, AtomValue, ContainerKind, DeletedSubtree, DocumentEdgeId, DocumentGraph,
    DocumentGraphError, DocumentNode, DocumentNodeKind, NodeLocation, NoteName, OperatorValue,
    PlacementAddress, StackIndex, TilePrototypeId as GraphTilePrototypeId, TileSpawnKind,
};
pub use patch::{DocumentPatch, apply_document_patch, capture_subtree_patch};
pub use port_endpoints::{
    PortEndpointConfig, PortEndpointStore, PortSlotState, cycle_port_state,
    default_connection_kind, port_state_for_side, set_port_state,
};
pub use queries::DocumentQueries;
pub use tessera_sync::{
    AuthoredBoardConnection, LegacyConnectionStore, RemovedBoardBinding, bind_tiles_on_board,
    connection_exists, connections_from_program, empty_container_stack, export_container_stack,
    export_container_stack_excluding, export_container_stack_with_insert,
    hydrate_board_from_document, map_container_kind_for_document, migrate_legacy_connections,
    neighbor_at_side, opposite_spatial_side, spatial_side_between, stack_nodes_on_surface,
    sync_authored_program_to_document, sync_document_tessera_from_board,
    sync_root_slot_to_document, unbind_output_side_on_board,
};

use crate::domain::board::{BoardSurface, BoardSurfaceId, BoardSurfaceKind, BoardSurfaces};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusaicDocument {
    pub graph: DocumentGraph,
    pub surfaces: BoardSurfaces,
    pub tiles: TileStore,
    #[serde(default, skip_serializing, rename = "connections")]
    pub(crate) legacy_connections: LegacyConnectionStore,
    #[serde(default)]
    pub port_endpoints: PortEndpointStore,
    pub root_surface: BoardSurfaceId,
    pub revision: DocumentRevision,
    pub tessera: TesseraDocumentState,
    pub playback: PlaybackDefaults,
}

impl Default for MusaicDocument {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl MusaicDocument {
    pub fn new_empty() -> Self {
        let root_surface = BoardSurfaceId(0);
        let mut surfaces = BoardSurfaces::default();
        surfaces
            .insert(BoardSurface {
                id: root_surface,
                kind: BoardSurfaceKind::RootBoard,
            })
            .expect("new document should create exactly one root surface");

        Self {
            graph: DocumentGraph::default(),
            surfaces,
            tiles: TileStore::default(),
            legacy_connections: LegacyConnectionStore::default(),
            port_endpoints: PortEndpointStore::default(),
            root_surface,
            revision: DocumentRevision(0),
            tessera: TesseraDocumentState::default(),
            playback: PlaybackDefaults::default(),
        }
    }

    pub fn bump_revision(&mut self) {
        self.revision.0 += 1;
    }

    pub fn sync_tile_store_from_graph(&mut self) {
        self.tiles = TileStore::from_graph(&self.graph);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TesseraDocumentState {
    pub authored_program: AuthoredTesseraProgram,
}

impl Default for TesseraDocumentState {
    fn default() -> Self {
        Self {
            authored_program: AuthoredTesseraProgram::empty(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackDefaults {
    #[serde(default = "default_bpm")]
    pub bpm: f64,
}

impl Default for PlaybackDefaults {
    fn default() -> Self {
        Self { bpm: default_bpm() }
    }
}

fn default_bpm() -> f64 {
    120.0
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TileStore {
    pub by_id: BTreeMap<NodeId, AuthoredTile>,
}

impl TileStore {
    pub fn apply_patch(
        &mut self,
        graph: &DocumentGraph,
        inserted: &[NodeId],
        deleted: &crate::domain::document::graph::DeletedSubtree,
    ) {
        for node_id in &deleted.nodes {
            self.by_id.remove(node_id);
        }
        for node_id in inserted {
            if let Some(node) = graph.node(node_id) {
                self.by_id.insert(
                    node_id.clone(),
                    AuthoredTile {
                        id: node_id.clone(),
                        kind: node.kind.clone(),
                        placement: graph.location_of(node_id).map(|location| {
                            AuthoredTilePlacement {
                                surface: location.surface,
                                address: location.address,
                            }
                        }),
                    },
                );
            }
        }
    }

    pub fn from_graph(graph: &DocumentGraph) -> Self {
        let by_id = graph
            .nodes()
            .map(|node| {
                (
                    node.id.clone(),
                    AuthoredTile {
                        id: node.id.clone(),
                        kind: node.kind.clone(),
                        placement: graph.location_of(&node.id).map(|location| {
                            AuthoredTilePlacement {
                                surface: location.surface,
                                address: location.address,
                            }
                        }),
                    },
                )
            })
            .collect();

        Self { by_id }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthoredTile {
    pub id: NodeId,
    pub kind: DocumentNodeKind,
    pub placement: Option<AuthoredTilePlacement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoredTilePlacement {
    pub surface: BoardSurfaceId,
    pub address: PlacementAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PortId(pub u64);

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct DocumentRevision(pub u64);
