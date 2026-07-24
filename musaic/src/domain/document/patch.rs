use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use tessera::bevy::TesseraBoard;
use tessera::prelude::{NodeId, NodeSpatialBindings};

use super::graph::{DocumentGraph, DocumentNode, NodeLocation};
use super::{BoardSurface, BoardSurfaceId, MusaicDocument};

/// Snapshot of a deleted subtree for undo restore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DocumentPatch {
    pub nodes: BTreeMap<NodeId, DocumentNode>,
    pub locations: BTreeMap<NodeId, NodeLocation>,
    pub container_surfaces: BTreeMap<NodeId, BoardSurfaceId>,
    pub surfaces: BTreeMap<BoardSurfaceId, BoardSurface>,
    #[serde(default)]
    pub tessera_bindings: BTreeMap<NodeId, NodeSpatialBindings>,
}

impl DocumentPatch {
    pub fn merge(&mut self, other: DocumentPatch) {
        self.nodes.extend(other.nodes);
        self.locations.extend(other.locations);
        self.container_surfaces.extend(other.container_surfaces);
        self.surfaces.extend(other.surfaces);
        self.tessera_bindings.extend(other.tessera_bindings);
    }
}

/// Captures nodes, surfaces, and tessera bindings for the given roots (including nested children).
pub fn capture_subtree_patch(document: &MusaicDocument, roots: &[NodeId]) -> DocumentPatch {
    let mut node_ids = BTreeSet::new();
    for root in roots {
        if document.graph.contains_node(root) {
            collect_subtree_ids(&document.graph, root, &mut node_ids);
        }
    }

    let mut patch = DocumentPatch::default();
    for node_id in &node_ids {
        let Some(node) = document.graph.node(node_id).cloned() else {
            continue;
        };
        patch.nodes.insert(node_id.clone(), node);
        if let Some(location) = document.graph.location_of(node_id) {
            patch.locations.insert(node_id.clone(), location);
        }
        if let Some(local_surface) = document.graph.container_surface(node_id) {
            patch
                .container_surfaces
                .insert(node_id.clone(), local_surface);
            if let Some(surface) = document.surfaces.get(local_surface) {
                patch.surfaces.insert(local_surface, surface.clone());
            }
        }
        if let Some(bindings) = document
            .tessera
            .authored_program
            .root_surface
            .bindings
            .get(node_id)
        {
            patch
                .tessera_bindings
                .insert(node_id.clone(), bindings.clone());
        }
    }

    patch
}

fn collect_subtree_ids(graph: &DocumentGraph, root: &NodeId, out: &mut BTreeSet<NodeId>) {
    if !out.insert(root.clone()) {
        return;
    }
    let Some(local_surface) = graph.container_surface(root) else {
        return;
    };
    for (_, node) in graph.nodes_on_surface(local_surface) {
        collect_subtree_ids(graph, &node.id, out);
    }
}

/// Restores a captured subtree into the document and re-exports the live board.
pub fn apply_document_patch(
    document: &mut MusaicDocument,
    board: &mut TesseraBoard,
    patch: DocumentPatch,
) -> Result<(), String> {
    apply_patch_inner(document, &patch)?;
    crate::domain::document::hydrate_board_from_document(board, document)
        .map_err(|error| format!("board export after restore failed: {error:?}"))?;
    Ok(())
}

fn apply_patch_inner(document: &mut MusaicDocument, patch: &DocumentPatch) -> Result<(), String> {
    for surface in patch.surfaces.values() {
        if !document.surfaces.contains(surface.id) {
            document
                .surfaces
                .insert(surface.clone())
                .map_err(|error| format!("restore surface failed: {error:?}"))?;
        }
        document.graph.reserve_surface_id(surface.id);
    }

    for node in patch.nodes.values() {
        document.graph.reserve_node_id(&node.id);
        if let super::graph::DocumentNodeKind::Container(container) = &node.kind {
            document.graph.reserve_surface_id(container.local_surface);
        }
    }

    for node in patch.nodes.values() {
        let Some(location) = patch.locations.get(&node.id) else {
            continue;
        };
        if document.graph.contains_node(&node.id) {
            continue;
        }
        document
            .graph
            .restore_node_unchanged(&mut document.surfaces, node.clone(), *location)
            .map_err(|error| format!("restore node {} failed: {error:?}", node.id.0))?;
        if let Some(local_surface) = patch.container_surfaces.get(&node.id) {
            document
                .graph
                .link_container_surface(node.id.clone(), *local_surface);
        }
    }

    for (node_id, bindings) in &patch.tessera_bindings {
        document
            .tessera
            .authored_program
            .root_surface
            .bindings
            .insert(node_id.clone(), bindings.clone());
    }

    document.sync_tile_store_from_graph();
    document.bump_revision();
    Ok(())
}
