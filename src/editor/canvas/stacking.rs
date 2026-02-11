//! Stable node depth allocation and bring-to-front ordering for canvas entities.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::EditorSystemSet;
use crate::core::NodeId;
use crate::editor::canvas::CanvasNode;
use crate::editor::canvas::projection::NodeEntityMap;
use crate::editor::state::EditorReady;

// Node-local widget offsets reach roughly +0.3 on top of the root transform.
// Keep inter-node step much larger so each node (and its children) stays as a depth group.
const Z_STEP: f32 = 1.0;

pub(super) fn plugin(app: &mut App) {
    app.add_message::<NodeStackIntent>();
    app.init_resource::<NodeStackingOrder>();
    app.add_systems(
        Update,
        apply_node_stack_intents
            .run_if(in_state(EditorReady))
            .in_set(EditorSystemSet::RenderSelection),
    );
}

#[derive(Message, Debug, Clone)]
pub enum NodeStackIntent {
    BringToFront { node_ids: Vec<NodeId> },
}

#[derive(Resource, Debug)]
pub struct NodeStackingOrder {
    pub next_z: f32,
    pub z_by_node: HashMap<NodeId, f32>,
}

impl Default for NodeStackingOrder {
    fn default() -> Self {
        Self {
            next_z: 0.0,
            z_by_node: HashMap::new(),
        }
    }
}

impl NodeStackingOrder {
    pub fn ensure_node_z(&mut self, node_id: NodeId) -> f32 {
        if let Some(z) = self.z_by_node.get(&node_id).copied() {
            return z;
        }

        self.next_z += Z_STEP;
        let z = self.next_z;
        self.z_by_node.insert(node_id, z);
        z
    }

    pub fn remove_node(&mut self, node_id: NodeId) {
        self.z_by_node.remove(&node_id);
    }

    pub fn clear(&mut self) {
        self.next_z = 0.0;
        self.z_by_node.clear();
    }
}

pub fn apply_node_stack_intents(
    mut intents: MessageReader<NodeStackIntent>,
    mut stacking: ResMut<NodeStackingOrder>,
    entity_map: Res<NodeEntityMap>,
    mut nodes: Query<(&CanvasNode, &mut Transform)>,
) {
    for intent in intents.read() {
        match intent {
            NodeStackIntent::BringToFront { node_ids } => {
                // Rebase on the current top-most z so bring-to-front always wins.
                let mut current_top_z = stacking.next_z;
                for (_, transform) in nodes.iter_mut() {
                    current_top_z = current_top_z.max(transform.translation.z);
                }
                stacking.next_z = current_top_z;

                let mut ordered = Vec::with_capacity(node_ids.len());
                for node_id in node_ids {
                    let Some(entity) = entity_map.get(*node_id) else {
                        continue;
                    };

                    // Preserve relative order within the dragged set.
                    let current_z = if let Ok((_, transform)) = nodes.get(entity) {
                        transform.translation.z
                    } else {
                        continue;
                    };

                    ordered.push((current_z, *node_id, entity));
                }

                ordered.sort_by(|a, b| a.0.total_cmp(&b.0));

                for (_, node_id, entity) in ordered {
                    let Ok((_, mut transform)) = nodes.get_mut(entity) else {
                        continue;
                    };
                    stacking.next_z += Z_STEP;
                    let new_z = stacking.next_z;
                    transform.translation.z = new_z;
                    stacking.z_by_node.insert(node_id, new_z);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Invariants:
    // - z increases monotonically for moved nodes.
    // - z_by_node mirrors transform z for moved nodes.
    // - relative order inside moved group follows prior z order.
    // - unknown node IDs are ignored without side effects.

    fn seed_node(app: &mut App, id: NodeId, z: f32) -> Entity {
        let entity = app
            .world_mut()
            .spawn((
                CanvasNode {
                    node_id: id,
                    width: 100.0,
                    height: 40.0,
                },
                Transform::from_xyz(0.0, 0.0, z),
            ))
            .id();
        app.world_mut()
            .resource_mut::<NodeEntityMap>()
            .insert(id, entity);
        entity
    }

    #[test]
    fn stacking_bring_to_front_is_monotonic_and_preserves_relative_order() {
        let mut app = App::new();
        app.add_message::<NodeStackIntent>();
        app.insert_resource(NodeStackingOrder::default());
        app.insert_resource(NodeEntityMap::new());
        app.add_systems(Update, apply_node_stack_intents);

        let id_low = NodeId::new();
        let id_high = NodeId::new();
        let id_unknown = NodeId::new();

        seed_node(&mut app, id_low, 0.2);
        seed_node(&mut app, id_high, 0.9);
        let _ = id_unknown;

        app.world_mut()
            .write_message(NodeStackIntent::BringToFront {
                node_ids: vec![id_high, id_low, id_unknown],
            });
        app.update();

        let map = app.world().resource::<NodeEntityMap>();
        let low_entity = map.get(id_low).unwrap();
        let high_entity = map.get(id_high).unwrap();
        let low_z = app
            .world()
            .entity(low_entity)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        let high_z = app
            .world()
            .entity(high_entity)
            .get::<Transform>()
            .unwrap()
            .translation
            .z;
        assert!(low_z > 0.0);
        assert!(
            high_z > low_z,
            "higher-prior-z node should stay above within moved group"
        );

        let stacking = app.world().resource::<NodeStackingOrder>();
        assert_eq!(stacking.z_by_node.get(&id_low), Some(&low_z));
        assert_eq!(stacking.z_by_node.get(&id_high), Some(&high_z));
        assert!(
            !stacking.z_by_node.contains_key(&id_unknown),
            "unknown ids must not be inserted"
        );
    }

    #[test]
    fn ensure_node_z_allocates_stable_group_depths() {
        let mut stacking = NodeStackingOrder::default();
        let id_a = NodeId::new();
        let id_b = NodeId::new();

        let z_a = stacking.ensure_node_z(id_a);
        let z_b = stacking.ensure_node_z(id_b);
        let z_a_again = stacking.ensure_node_z(id_a);

        assert_eq!(z_a_again, z_a, "node z must be stable per node id");
        assert!(
            (z_b - z_a) > 0.3,
            "node z step must be larger than node-local widget z offsets"
        );
    }

    #[test]
    fn clear_resets_all_allocated_depth_state() {
        let mut stacking = NodeStackingOrder::default();
        let id = NodeId::new();
        let _ = stacking.ensure_node_z(id);

        assert!(!stacking.z_by_node.is_empty());
        assert!(stacking.next_z > 0.0);

        stacking.clear();

        assert!(stacking.z_by_node.is_empty());
        assert_eq!(stacking.next_z, 0.0);
    }
}
