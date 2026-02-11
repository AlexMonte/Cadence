//! Canvas projection and lifecycle for model-backed node entities.

use bevy::{camera::visibility::RenderLayers, picking::hover::Hovered, prelude::*};
use std::collections::HashSet;

use crate::{
    EditorSystemSet,
    core::{NodeId, NodeKind},
    editor::{AppState, camera::CANVAS_RENDER_LAYER, gestures::drag::DropZone, state::EditorReady},
};
use projection::NodeEntityMap;
use stacking::NodeStackingOrder;

pub mod projection;
pub mod stacking;
pub use stacking::NodeStackIntent;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (spawn_nodes_from_model, despawn_deleted_nodes)
            .chain()
            .run_if(in_state(EditorReady))
            .in_set(EditorSystemSet::SelectionResolution),
    );
    app.add_plugins((projection::plugin, stacking::plugin));
}

/// Root marker for a model-backed node rendered in world space.
///
/// Child widgets (pattern grid, chrome, controls) are attached under this entity.
/// Only the root should carry `CanvasNode`.
#[derive(Component, Debug, Clone)]
#[require(
    Sprite::from_color(LinearRgba::rgb(0.2, 0.2, 0.2), Vec2::new(200.0, 100.0)),
    Transform::default(),
    // Attach Interaction so selection/changed queries work for canvas nodes
    Interaction::None,
    Hovered(false),
    // Add Pickable component for pointer interactions
    Pickable::default(),
    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
    DropZone,
)]
pub struct CanvasNode {
    pub node_id: NodeId,
    pub width: f32,
    pub height: f32,
}

impl CanvasNode {
    pub fn new(node_id: NodeId) -> Self {
        CanvasNode {
            node_id,
            width: 150.0,
            height: 60.0,
        }
    }

    pub fn with_size(node_id: NodeId, size: Vec2) -> Self {
        CanvasNode {
            node_id,
            width: size.x,
            height: size.y,
        }
    }
}

fn node_size_for_kind(kind: &NodeKind) -> Vec2 {
    match kind {
        NodeKind::Pattern { .. } => Vec2::new(360.0, 280.0),
        _ => Vec2::new(150.0, 60.0),
    }
}

/// Spawn CanvasNode entities for each node in the current scope.
/// This system runs when a new scope is loaded or nodes are added.
pub fn spawn_nodes_from_model(
    mut commands: Commands,
    mut entity_map: ResMut<NodeEntityMap>,
    mut stacking: ResMut<NodeStackingOrder>,
    app_state: Res<AppState>,
) {
    let scope_id = app_state.current_scope;
    let nodes = app_state.project.get_scope_nodes(scope_id);

    for node in nodes {
        // Skip if this node already has an entity
        if entity_map.get(node.id).is_some() {
            continue;
        }

        let size = node_size_for_kind(&node.kind);
        let z = stacking.ensure_node_z(node.id);
        let entity = commands
            .spawn((
                CanvasNode::with_size(node.id, size),
                Sprite::from_color(LinearRgba::rgb(0.2, 0.2, 0.2), size),
                Transform::from_xyz(0.0, 0.0, z),
            ))
            .id();

        // Register in the projection map
        entity_map.insert(node.id, entity);
    }
}

/// Remove CanvasNode entities whose corresponding model nodes no longer exist.
pub fn despawn_deleted_nodes(
    mut commands: Commands,
    mut entity_map: ResMut<NodeEntityMap>,
    mut stacking: ResMut<NodeStackingOrder>,
    app_state: Res<AppState>,
) {
    let current_node_ids: HashSet<_> = app_state
        .project
        .get_scope_nodes(app_state.current_scope)
        .iter()
        .map(|node| node.id)
        .collect();

    let nodes_to_despawn: Vec<_> = entity_map
        .iter()
        .filter(|(node_id, _)| !current_node_ids.contains(node_id))
        .map(|(node_id, _)| *node_id)
        .collect();

    for node_id in nodes_to_despawn {
        stacking.remove_node(node_id);
        if let Some(entity) = entity_map.remove(node_id) {
            commands.entity(entity).try_despawn();
        }
    }
}

/// Marker for selected nodes on canvas
#[derive(Component, Debug)]
pub struct Selected;

/// Wire/edge being drawn from a port
#[derive(Component, Debug)]
pub struct CanvasEdgeDraft {
    pub from_node: NodeId,
    pub from_port: String,
}

#[cfg(test)]
mod tests {}
