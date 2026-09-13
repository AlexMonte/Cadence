//! Authored linked instances, including tiles inside nested patterns.
//! Query on demand; navigation resolves identities against the current document.
use crate::{
    application::editor::tile_inspect_title,
    domain::document::{DocumentNodeKind, DocumentQueries, MusaicDocument, PlacementAddress},
};
use tessera::prelude::NodeId;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedUse {
    pub node: NodeId,
    pub location: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedUses {
    pub id: u64,
    pub name: String,
    pub source: NodeId,
    pub source_location: String,
    pub tiles: Vec<LinkedUse>,
}
pub fn location(document: &MusaicDocument, node: &NodeId) -> String {
    let mut path = Vec::new();
    let mut current = node.clone();
    let mut visited = std::collections::BTreeSet::new();
    while visited.insert(current.clone()) {
        let Some(site) = document.graph.location_of(&current) else {
            path.push("Missing tile".into());
            break;
        };
        let position = match site.address {
            PlacementAddress::BoardSlot(slot) => {
                crate::application::pipeline::selected_tile::grid_coordinate(slot.x, slot.y)
            }
            PlacementAddress::StackIndex(index) => format!("Tile {}", index.0 + 1),
        };
        path.push(position);
        if site.surface == document.root_surface {
            path.push("Pattern board".into());
            break;
        }
        let Some(parent) = document.graph.container_node_for_surface(site.surface) else {
            break;
        };
        path.push(tile_inspect_title(&DocumentQueries::new(document), &parent));
        current = parent;
    }
    path.reverse();
    path.join(" / ")
}
pub fn linked_uses(document: &MusaicDocument, id: u64) -> Option<LinkedUses> {
    let definition = document.tricks.get(&id)?;
    let mut tiles = document
        .graph
        .nodes()
        .filter_map(|node| match &node.kind {
            DocumentNodeKind::TrickInstance(t) if t.prototype.0 == id => Some(LinkedUse {
                node: node.id.clone(),
                location: location(document, &node.id),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    tiles.sort_by(|a, b| a.location.cmp(&b.location).then(a.node.cmp(&b.node)));
    Some(LinkedUses {
        id,
        name: definition.name.clone(),
        source: definition.source.clone(),
        source_location: location(document, &definition.source),
        tiles,
    })
}

/// Definitions rooted at this tile or one of its owning containers.
/// A copied container has different identities and therefore no implicit link.
pub fn source_owners(document: &MusaicDocument, node: &NodeId) -> Vec<(u64, String, usize)> {
    let mut ancestors = std::collections::BTreeSet::new();
    let mut current = node.clone();
    while ancestors.insert(current.clone()) {
        let Some(site) = document.graph.location_of(&current) else {
            break;
        };
        let Some(parent) = document.graph.container_node_for_surface(site.surface) else {
            break;
        };
        current = parent;
    }
    document
        .tricks
        .iter()
        .filter(|(_, d)| ancestors.contains(&d.source))
        .map(|(id, d)| (*id, d.name.clone(), instance_count(document, *id)))
        .collect()
}
pub fn instance_count(document: &MusaicDocument, id: u64) -> usize {
    document
        .graph
        .nodes()
        .filter(|n| matches!(&n.kind,DocumentNodeKind::TrickInstance(t) if t.prototype.0==id))
        .count()
}
