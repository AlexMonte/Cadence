//! Optional relationship markers and edge metadata used by editor rendering features.

/// Custom Bevy relationship for node ownership/hierarchy.
/// Links child entities (UI elements, ports, labels) to their owner nodes.
///
/// Example usage (when Relationship derive is available):
/// ```ignore
/// commands.entity(child_entity)
///     .add_relationship::<OwnedByNode>(node_entity);
/// ```
///
/// Relationship traits are not wired in this codebase yet; this is a marker used by systems that
/// need a typed ownership tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnedByNode;

/// Custom Bevy relationship for node connections/edges.
/// Links output ports of one node to input ports of another.
/// Can be extended later with visual metadata for wire rendering.
///
/// Example usage (when Relationship derive is available):
/// ```ignore
/// commands.entity(source_node_entity)
///     .add_relationship::<ConnectedTo>(target_node_entity);
/// ```
///
/// Relationship traits are not wired in this codebase yet; this is a marker used by systems that
/// need a typed connection tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectedTo;

/// Metadata associated with a ConnectedTo relationship.
/// Used when rendering wires: stores color, style, animation state.
#[derive(Debug, Clone)]
pub struct EdgeVisualMetadata {
    /// Edge identifier from the data model
    pub edge_id: crate::core::EdgeId,
    /// Port names for source and target
    pub from_port: String,
    pub to_port: String,
    /// Visual properties
    pub color: Option<[f32; 3]>,
    pub style: EdgeStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeStyle {
    /// Solid line
    Solid,
    /// Dashed line
    Dashed,
    /// Dotted line
    Dotted,
}

impl Default for EdgeVisualMetadata {
    fn default() -> Self {
        Self {
            edge_id: crate::core::EdgeId::new(),
            from_port: String::new(),
            to_port: String::new(),
            color: None,
            style: EdgeStyle::Solid,
        }
    }
}
