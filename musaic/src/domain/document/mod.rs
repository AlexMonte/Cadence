use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tessera::prelude::{NodeId, NodeSpatialBindings, RootRelation};

pub mod connection_policy;
pub mod connections;
pub mod export;
pub mod footprint;
pub mod graph;
pub mod patch;
pub mod ports;
pub mod queries;
pub use connection_policy::{AuthoredEdge, ConnectionPolicyError, authorize_connection};
pub use connections::{
    DocumentConnectionView, bind_authorized_edge, bind_tiles, connection_exists,
    connections_from_program, empty_container_stack, export_container_stack,
    export_container_stack_excluding, export_container_stack_with_insert,
    map_container_kind_for_document, neighbor_at_side, opposite_spatial_side, spatial_side_between,
    stack_nodes_on_surface, unbind_connection, unbind_output_side,
};
pub use export::{export_document_program, map_tessera_container_kind_to_document};
pub use footprint::{RootBoardTileKind, root_board_tile_footprint};
pub use graph::{
    Accidental, AtomValue, ContainerKind, DeletedSubtree, DocumentGraph, DocumentGraphError,
    DocumentNode, DocumentNodeKind, DrumHit, NodeLocation, NoteName, OperatorValue,
    PlacementAddress, SoundNode, StackIndex, TilePrototypeId as GraphTilePrototypeId,
    TileSpawnKind,
};
pub use patch::{DocumentPatch, apply_document_patch, capture_subtree_patch};
pub use ports::{
    PortEndpointConfig, PortSlotState, cycle_port_state, port_config, port_state_for_side,
    set_port_state,
};
pub use queries::DocumentQueries;

use crate::domain::board::{BoardSurface, BoardSurfaceId, BoardSurfaceKind, BoardSurfaces};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MusaicDocument {
    pub channels: u16,
    pub tricks: BTreeMap<u64, crate::domain::tricks::TrickDefinition>,
    pub graph: DocumentGraph,
    pub surfaces: BoardSurfaces,
    pub connections: DocumentConnections,
    pub root_surface: BoardSurfaceId,
    pub revision: DocumentRevision,
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
            channels: default_channels(),
            tricks: BTreeMap::new(),
            graph: DocumentGraph::default(),
            surfaces,
            connections: DocumentConnections::default(),
            root_surface,
            revision: DocumentRevision(0),
            playback: PlaybackDefaults::default(),
        }
    }

    pub fn bump_revision(&mut self) {
        self.revision.0 += 1;
    }

    pub fn replace_connections_from(&mut self, program: &tessera::prelude::AuthoredTesseraProgram) {
        self.connections.bindings = program.root_surface.bindings.clone();
        self.connections.explicit_relations = program.root_surface.explicit_relations.clone();
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.channels == 0 {
            return Err("A project must have at least one output channel".into());
        }
        if !self.playback.bpm.is_finite() || !(1.0..=999.0).contains(&self.playback.bpm) {
            return Err("Tempo must be between 1 and 999 BPM".into());
        }
        if !(1..=64).contains(&self.playback.beats_per_cycle) {
            return Err("Beats per cycle must be between 1 and 64".into());
        }
        self.graph.validate(&self.surfaces, self.root_surface)?;

        let program = export_document_program(self)
            .map_err(|error| format!("The document cannot be exported: {error:?}"))?;
        for (node, bindings) in &self.connections.bindings {
            let Some(kind) = program.root_surface.nodes.get(node) else {
                return Err(format!(
                    "Connection bindings refer to missing tile {}",
                    node.0
                ));
            };
            if !program.root_surface.placements.contains_key(node) {
                return Err(format!(
                    "Connection bindings refer to a non-root tile {}",
                    node.0
                ));
            }
            let declared = tessera::prelude::default_spatial_bindings(kind);
            if bindings
                .inputs
                .keys()
                .any(|endpoint| !declared.inputs.contains_key(endpoint))
                || bindings
                    .outputs
                    .keys()
                    .any(|endpoint| !declared.outputs.contains_key(endpoint))
            {
                return Err(format!(
                    "Connection bindings use an invalid endpoint on {}",
                    node.0
                ));
            }
        }
        for relation in &self.connections.explicit_relations {
            let edge = connection_policy::explicit_connection(relation);
            if !program.root_surface.placements.contains_key(&edge.from)
                || !program.root_surface.placements.contains_key(&edge.to)
            {
                return Err("A connection refers to a missing root-board tile".into());
            }
            let source =
                tessera::prelude::default_spatial_bindings(&program.root_surface.nodes[&edge.from]);
            let target =
                tessera::prelude::default_spatial_bindings(&program.root_surface.nodes[&edge.to]);
            if !source.outputs.contains_key(&edge.output)
                || !target.inputs.contains_key(&edge.input)
            {
                return Err("A connection uses an invalid endpoint".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentConnections {
    pub bindings: BTreeMap<NodeId, NodeSpatialBindings>,
    pub explicit_relations: Vec<RootRelation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaybackDefaults {
    pub bpm: f64,
    pub beats_per_cycle: u32,
}

impl Default for PlaybackDefaults {
    fn default() -> Self {
        Self {
            bpm: default_bpm(),
            beats_per_cycle: default_beats_per_cycle(),
        }
    }
}

fn default_bpm() -> f64 {
    120.0
}

fn default_beats_per_cycle() -> u32 {
    4
}

impl PlaybackDefaults {
    pub fn cycles_per_second(&self) -> cadence::prelude::Time {
        let bpm = if self.bpm.is_finite() {
            self.bpm.clamp(1.0, 999.0)
        } else {
            default_bpm()
        };
        cadence::prelude::Time::new(
            (bpm * 1000.0).round() as i64,
            60_000 * i64::from(self.beats_per_cycle.clamp(1, 64)),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PortId(pub u64);

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct DocumentRevision(pub u64);

fn default_channels() -> u16 {
    16
}
