//! Canonical project model and graph data types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod id;
pub mod relationships;
pub mod sequencer;

pub use id::{EdgeId, NodeId, ScopeId};
pub use relationships::{ConnectedTo, EdgeStyle, EdgeVisualMetadata, OwnedByNode};
pub use sequencer::{SequencerPatternData, SequencerPatternError};

/// Represents a single computation unit (node) in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub parent_scope: ScopeId,
    pub params: HashMap<String, serde_json::Value>,
    pub input_ports: Vec<PortDescriptor>,
    pub output_ports: Vec<PortDescriptor>,
}

/// Type descriptor for a node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeKind {
    Binding { name: String },
    Pattern { pattern_type: String },
    Transform { transform_type: String },
    Output,
    Custom { node_type: String },
}

/// Port metadata for node inputs/outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortDescriptor {
    pub name: String,
    pub port_type: String,
}

/// Represents a connection between two node ports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EdgeId,
    pub from_node: NodeId,
    pub from_port: String,
    pub to_node: NodeId,
    pub to_port: String,
}

/// A scope is a container for nodes and child scopes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scope {
    pub id: ScopeId,
    pub parent_scope: Option<ScopeId>,
    pub child_scopes: Vec<ScopeId>,
    pub node_ids: Vec<NodeId>,
}

/// Canvas layout state for a scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    pub scope_id: ScopeId,
    pub node_positions: HashMap<NodeId, (f32, f32)>,
    pub camera_pos: (f32, f32),
    pub zoom: f32,
}

/// Semantic model: nodes, scopes, and edges (immutable diff)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelData {
    pub root_scope: ScopeId,
    pub scopes: HashMap<ScopeId, Scope>,
    pub nodes: HashMap<NodeId, ModelNode>,
    pub edges: HashMap<EdgeId, Edge>,
}

/// Layout data: visual state (high churn, separate from model)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutData {
    pub layouts: HashMap<ScopeId, Layout>,
}

/// The root project containing both model and layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub model: ModelData,
    pub layout: LayoutData,
}

fn default_scope(scope_id: ScopeId) -> Scope {
    Scope {
        id: scope_id,
        parent_scope: None,
        child_scopes: Vec::new(),
        node_ids: Vec::new(),
    }
}

pub(crate) fn default_layout(scope_id: ScopeId) -> Layout {
    Layout {
        scope_id,
        node_positions: HashMap::new(),
        camera_pos: (0.0, 0.0),
        zoom: 1.0,
    }
}

impl Project {
    /// Create a new project with a root scope
    pub fn new(name: String) -> Self {
        let root_scope_id = ScopeId::new();
        let mut scopes = HashMap::new();
        scopes.insert(root_scope_id, default_scope(root_scope_id));

        let mut layouts = HashMap::new();
        layouts.insert(root_scope_id, default_layout(root_scope_id));

        Project {
            name,
            model: ModelData {
                root_scope: root_scope_id,
                scopes,
                nodes: HashMap::new(),
                edges: HashMap::new(),
            },
            layout: LayoutData { layouts },
        }
    }

    /// Add a node to a scope
    pub fn add_node(&mut self, scope_id: ScopeId, node: ModelNode) -> NodeId {
        let mut node = node;
        node.parent_scope = scope_id;
        let node_id = node.id;
        self.model.nodes.insert(node_id, node);
        if let Some(scope) = self.model.scopes.get_mut(&scope_id) {
            scope.node_ids.push(node_id);
        }
        node_id
    }

    /// Remove a node and all related references from model + layout state.
    pub fn remove_node(&mut self, node_id: NodeId) -> bool {
        let removed = self.model.nodes.remove(&node_id).is_some();
        if !removed {
            return false;
        }

        for scope in self.model.scopes.values_mut() {
            scope.node_ids.retain(|id| *id != node_id);
        }

        self.model
            .edges
            .retain(|_, edge| edge.from_node != node_id && edge.to_node != node_id);

        for layout in self.layout.layouts.values_mut() {
            layout.node_positions.remove(&node_id);
        }

        true
    }

    /// Add an edge connecting two nodes
    pub fn add_edge(&mut self, edge: Edge) -> EdgeId {
        let edge_id = edge.id;
        self.model.edges.insert(edge_id, edge);
        edge_id
    }

    /// Get all nodes in a scope
    pub fn get_scope_nodes(&self, scope_id: ScopeId) -> Vec<&ModelNode> {
        self.model
            .scopes
            .get(&scope_id)
            .map(|scope| {
                scope
                    .node_ids
                    .iter()
                    .filter_map(|node_id| self.model.nodes.get(node_id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the root scope ID
    pub fn root_scope(&self) -> ScopeId {
        self.model.root_scope
    }
}

/// Symbol types in the language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Symbol {
    Bool(bool),
    Number(f64),
    String(String),
    Object(serde_json::Value),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_node_cleans_scope_edges_and_layout() {
        let mut project = Project::new("remove-node-test".to_string());
        let scope_id = project.root_scope();

        let first = ModelNode {
            id: NodeId::new(),
            kind: NodeKind::Pattern {
                pattern_type: "first".to_string(),
            },
            parent_scope: scope_id,
            params: HashMap::new(),
            input_ports: vec![],
            output_ports: vec![],
        };
        let second = ModelNode {
            id: NodeId::new(),
            kind: NodeKind::Pattern {
                pattern_type: "second".to_string(),
            },
            parent_scope: scope_id,
            params: HashMap::new(),
            input_ports: vec![],
            output_ports: vec![],
        };

        let first_id = project.add_node(scope_id, first);
        let second_id = project.add_node(scope_id, second);

        let edge_id = project.add_edge(Edge {
            id: EdgeId::new(),
            from_node: first_id,
            from_port: "out".to_string(),
            to_node: second_id,
            to_port: "in".to_string(),
        });

        let layout = project.layout.layouts.get_mut(&scope_id).unwrap();
        layout.node_positions.insert(first_id, (10.0, 20.0));
        layout.node_positions.insert(second_id, (30.0, 40.0));

        assert!(project.remove_node(first_id));
        assert!(!project.model.nodes.contains_key(&first_id));
        assert!(!project.model.edges.contains_key(&edge_id));

        let scope = project.model.scopes.get(&scope_id).unwrap();
        assert!(!scope.node_ids.contains(&first_id));
        assert!(scope.node_ids.contains(&second_id));

        let layout = project.layout.layouts.get(&scope_id).unwrap();
        assert!(!layout.node_positions.contains_key(&first_id));
        assert!(layout.node_positions.contains_key(&second_id));
    }
}
