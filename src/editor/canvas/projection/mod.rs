//! Projection map between model node IDs and spawned canvas entities.

use crate::core::NodeId;
use bevy::prelude::*;

use std::collections::BTreeMap;

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(NodeEntityMap::new());
}
/// Maps NodeIds to Bevy Entities for efficient lookups.
///
/// Used as a Bevy Resource to centralize spawn/update/despawn logic
/// in projection systems without global statics.
#[derive(Debug, Default, Resource)]
pub struct NodeEntityMap {
    map: BTreeMap<NodeId, Entity>,
}

impl NodeEntityMap {
    pub fn new() -> Self {
        NodeEntityMap {
            map: BTreeMap::new(),
        }
    }

    /// Register a node's entity
    pub fn insert(&mut self, node_id: NodeId, entity: Entity) {
        self.map.insert(node_id, entity);
    }

    /// Look up an entity by node ID
    pub fn get(&self, node_id: NodeId) -> Option<Entity> {
        self.map.get(&node_id).copied()
    }

    pub fn get_id(&self, entity: Entity) -> Option<NodeId> {
        self.map
            .iter()
            .find(|(_, e)| **e == entity)
            .map(|(&id, _)| id)
    }

    /// Remove a node's entity mapping
    pub fn remove(&mut self, node_id: NodeId) -> Option<Entity> {
        self.map.remove(&node_id)
    }

    /// Get all mappings
    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &Entity)> {
        self.map.iter()
    }

    /// Clear all mappings
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Get count of mapped entities
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_map_insert_get() {
        let mut map = NodeEntityMap::new();
        let node_id = NodeId::new();
        // Create an entity using from_bits which is const
        let entity = Entity::try_from_bits(42).unwrap();

        map.insert(node_id, entity);
        assert_eq!(map.get(node_id), Some(entity));
    }

    #[test]
    fn test_entity_map_remove() {
        let mut map = NodeEntityMap::new();
        let node_id = NodeId::new();
        let entity = Entity::try_from_bits(42).unwrap();

        map.insert(node_id, entity);
        assert_eq!(map.remove(node_id), Some(entity));
        assert_eq!(map.get(node_id), None);
    }

    #[test]
    fn test_entity_map_len_empty() {
        let mut map = NodeEntityMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);

        let node_id = NodeId::new();
        let entity = Entity::try_from_bits(42).unwrap();
        map.insert(node_id, entity);

        assert!(!map.is_empty());
        assert_eq!(map.len(), 1);

        map.clear();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }

    #[test]
    fn test_entity_map_as_resource() {
        // Verify it can be used as a Bevy Resource
        let map = NodeEntityMap::new();
        assert!(map.is_empty());
        assert_eq!(map.len(), 0);
    }
}
