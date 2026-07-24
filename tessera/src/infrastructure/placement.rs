use std::collections::BTreeSet;

use crate::domain::{
    AuthoredTesseraProgram, BoardSlot, ContainerId, ContainerSurfaceTile, NodeId, RootPlacement,
    RootSurfaceNodeKind, TileFootprint, apply_flow_member_default_bindings,
    default_spatial_bindings,
};

#[cfg(test)]
use crate::domain::Container;

pub(crate) fn remove_placed_node(program: &mut AuthoredTesseraProgram, node_id: &NodeId) {
    if let Some(node) = program.root_surface.nodes.remove(node_id) {
        if let RootSurfaceNodeKind::Container { container } = node {
            remove_container_tree(program, &container);
        }
    }
    program.root_surface.placements.remove(node_id);
    program.root_surface.bindings.remove(node_id);
    program
        .root_surface
        .explicit_relations
        .retain(|relation| !relation_references_node(relation, node_id));
}

fn remove_container_tree(program: &mut AuthoredTesseraProgram, root: &ContainerId) {
    let mut to_remove = BTreeSet::new();
    collect_nested_container_ids(program, root, &mut to_remove);
    for id in to_remove {
        program.containers.remove(&id);
    }
}

fn collect_nested_container_ids(
    program: &AuthoredTesseraProgram,
    root: &ContainerId,
    out: &mut BTreeSet<ContainerId>,
) {
    if !out.insert(root.clone()) {
        return;
    }
    let Some(container) = program.containers.get(root) else {
        return;
    };
    for tile in &container.stack {
        if let ContainerSurfaceTile::NestedContainer(nested) = tile {
            collect_nested_container_ids(program, nested, out);
        }
    }
}

fn relation_references_node(relation: &crate::domain::RootRelation, node_id: &NodeId) -> bool {
    use crate::domain::{RootRelation, StreamTarget};
    match relation {
        RootRelation::FlowsTo { from, to } => {
            from.node == *node_id
                || matches!(
                    to,
                    StreamTarget::TransformInput { node, .. }
                        | StreamTarget::FlowControlInput { node, .. }
                        | StreamTarget::OutputInput { node, .. } if node == node_id
                )
        }
        RootRelation::ChainedTo { from, to } => from.node == *node_id || to == node_id,
    }
}

pub(crate) fn container_id_for_node(
    program: &AuthoredTesseraProgram,
    node_id: &NodeId,
) -> Option<ContainerId> {
    match program.root_surface.nodes.get(node_id)? {
        RootSurfaceNodeKind::Container { container } => Some(container.clone()),
        _ => None,
    }
}

pub(crate) fn insert_placed_node(
    program: &mut AuthoredTesseraProgram,
    node_id: NodeId,
    slot: BoardSlot,
    node: RootSurfaceNodeKind,
    footprint: TileFootprint,
) {
    let mut bindings = default_spatial_bindings(&node);
    if let RootSurfaceNodeKind::FlowControl(flow) = &node {
        apply_flow_member_default_bindings(flow, &mut bindings);
    }
    program.root_surface.nodes.insert(node_id.clone(), node);
    program
        .root_surface
        .placements
        .insert(node_id.clone(), RootPlacement { slot, footprint });
    program.root_surface.bindings.insert(node_id, bindings);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ContainerKind;

    #[test]
    fn remove_placed_node_cleans_nested_containers() {
        let mut program = AuthoredTesseraProgram::default();
        program.containers.insert(
            ContainerId::new("child"),
            Container {
                kind: ContainerKind::Sequence,
                axis: crate::domain::ContainerAxis::Time,
                stack: Vec::new(),
            },
        );
        program.containers.insert(
            ContainerId::new("parent"),
            Container {
                kind: ContainerKind::Sequence,
                axis: crate::domain::ContainerAxis::Time,
                stack: vec![ContainerSurfaceTile::NestedContainer(ContainerId::new(
                    "child",
                ))],
            },
        );
        insert_placed_node(
            &mut program,
            NodeId::new("seq"),
            BoardSlot::new(0, 0),
            RootSurfaceNodeKind::Container {
                container: ContainerId::new("parent"),
            },
            TileFootprint::unit(),
        );

        remove_placed_node(&mut program, &NodeId::new("seq"));

        assert!(!program.containers.contains_key(&ContainerId::new("parent")));
        assert!(!program.containers.contains_key(&ContainerId::new("child")));
    }
}
