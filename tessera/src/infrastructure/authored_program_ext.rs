use crate::domain::{
    AuthoredTesseraProgram, BoardSlot, Container, ContainerId, ContainerKind, ContainerSurfaceTile,
    FlowControlKind, FlowControlNode, InputEndpoint, NodeId, OutputEndpoint, OutputNode,
    RootRelation, RootSurfaceNodeKind, SpatialSide, TileFootprint, TransformKind, TransformNode,
};

use super::board::Board;
use super::placement::insert_placed_node;

pub trait AuthoredTesseraProgramExt {
    fn place_container(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        kind: ContainerKind,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self;
    fn place_sequence(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self;
    fn place_alternate(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self;
    fn place_layer_container(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self;
    fn place_transform(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        kind: TransformKind,
    ) -> &mut Self;
    fn place_output(&mut self, id: impl Into<String>, slot: BoardSlot) -> &mut Self;
    fn place_flow_control(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        kind: FlowControlKind,
    ) -> &mut Self;
    fn place_layer(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self;
    fn place_split(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self;
    fn place_route(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self;
    fn bind_input_side(
        &mut self,
        node: impl Into<String>,
        endpoint: InputEndpoint,
        side: SpatialSide,
    ) -> &mut Self;
    fn bind_output_side(
        &mut self,
        node: impl Into<String>,
        endpoint: OutputEndpoint,
        side: SpatialSide,
    ) -> &mut Self;
    fn add_explicit_relation(&mut self, relation: RootRelation) -> &mut Self;
    fn rotate_node_cw(&mut self, node: impl Into<String>) -> &mut Self;
    fn rotate_node_ccw(&mut self, node: impl Into<String>) -> &mut Self;
}

impl AuthoredTesseraProgramExt for AuthoredTesseraProgram {
    fn place_container(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        kind: ContainerKind,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self {
        let id = id.into();
        let container_id = ContainerId::new(id.clone());
        let node_id = NodeId::new(id);
        self.containers.insert(
            container_id.clone(),
            Container {
                kind,
                axis: crate::domain::ContainerAxis::Time,
                stack,
            },
        );
        let node = RootSurfaceNodeKind::Container {
            container: container_id,
        };
        insert_placed_node(self, node_id, slot, node, TileFootprint::unit());
        self
    }

    fn place_sequence(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self {
        self.place_container(id, slot, ContainerKind::Sequence, stack)
    }

    fn place_alternate(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self {
        self.place_container(id, slot, ContainerKind::Alternate, stack)
    }

    fn place_layer_container(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self {
        self.place_container(id, slot, ContainerKind::Layer, stack)
    }

    fn place_transform(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        kind: TransformKind,
    ) -> &mut Self {
        let node_id = NodeId::new(id);
        let node = RootSurfaceNodeKind::Transform(TransformNode::new(kind));
        insert_placed_node(self, node_id, slot, node, TileFootprint::unit());
        self
    }

    fn place_output(&mut self, id: impl Into<String>, slot: BoardSlot) -> &mut Self {
        let node_id = NodeId::new(id);
        let node = RootSurfaceNodeKind::Output(OutputNode::default());
        insert_placed_node(self, node_id, slot, node, TileFootprint::unit());
        self
    }

    fn place_flow_control(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        kind: FlowControlKind,
    ) -> &mut Self {
        let node_id = NodeId::new(id);
        let node = RootSurfaceNodeKind::FlowControl(FlowControlNode::new(kind));
        insert_placed_node(self, node_id, slot, node, TileFootprint::unit());
        self
    }

    fn place_layer(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self {
        let mut node = FlowControlNode::new(FlowControlKind::Layer);
        node.members.inputs.insert(
            crate::domain::PortGroupId::new("streams"),
            members
                .into_iter()
                .map(crate::domain::PortMemberId::new)
                .collect(),
        );
        let node_id = NodeId::new(id);
        let node = RootSurfaceNodeKind::FlowControl(node);
        insert_placed_node(self, node_id, slot, node, TileFootprint::unit());
        self
    }

    fn place_split(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self {
        let mut node = FlowControlNode::new(FlowControlKind::Split);
        node.members.outputs.insert(
            crate::domain::PortGroupId::new("branches"),
            members
                .into_iter()
                .map(crate::domain::PortMemberId::new)
                .collect(),
        );
        let node_id = NodeId::new(id);
        let node = RootSurfaceNodeKind::FlowControl(node);
        insert_placed_node(self, node_id, slot, node, TileFootprint::unit());
        self
    }

    fn place_route(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self {
        let mut node = FlowControlNode::new(FlowControlKind::Route);
        node.members.outputs.insert(
            crate::domain::PortGroupId::new("routes"),
            members
                .into_iter()
                .map(crate::domain::PortMemberId::new)
                .collect(),
        );
        let node_id = NodeId::new(id);
        let node = RootSurfaceNodeKind::FlowControl(node);
        insert_placed_node(self, node_id, slot, node, TileFootprint::unit());
        self
    }

    fn bind_input_side(
        &mut self,
        node: impl Into<String>,
        endpoint: InputEndpoint,
        side: SpatialSide,
    ) -> &mut Self {
        let node = NodeId::new(node);
        self.root_surface
            .bindings
            .entry(node)
            .or_default()
            .inputs
            .insert(endpoint, side);
        self
    }

    fn bind_output_side(
        &mut self,
        node: impl Into<String>,
        endpoint: OutputEndpoint,
        side: SpatialSide,
    ) -> &mut Self {
        let node = NodeId::new(node);
        self.root_surface
            .bindings
            .entry(node)
            .or_default()
            .outputs
            .insert(endpoint, side);
        self
    }

    fn add_explicit_relation(&mut self, relation: RootRelation) -> &mut Self {
        self.root_surface.explicit_relations.push(relation);
        self
    }

    fn rotate_node_cw(&mut self, node: impl Into<String>) -> &mut Self {
        let node = NodeId::new(node);
        if let Some(bindings) = self.root_surface.bindings.get_mut(&node) {
            bindings.rotate_cw();
        }
        self
    }

    fn rotate_node_ccw(&mut self, node: impl Into<String>) -> &mut Self {
        let node = NodeId::new(node);
        if let Some(bindings) = self.root_surface.bindings.get_mut(&node) {
            bindings.rotate_ccw();
        }
        self
    }
}

impl AuthoredTesseraProgramExt for Board {
    fn place_container(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        kind: ContainerKind,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self {
        let slot_builder = self.at(slot.x, slot.y).named(id);
        match kind {
            ContainerKind::Sequence => {
                slot_builder.sequence(stack).expect("place sequence");
            }
            ContainerKind::Alternate => {
                slot_builder.alternate(stack).expect("place alternate");
            }
            ContainerKind::Layer => {
                slot_builder.layer_container(stack).expect("place layer");
            }
        }
        self
    }

    fn place_sequence(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self {
        self.at(slot.x, slot.y)
            .named(id)
            .sequence(stack)
            .expect("place sequence");
        self
    }

    fn place_alternate(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self {
        self.at(slot.x, slot.y)
            .named(id)
            .alternate(stack)
            .expect("place alternate");
        self
    }

    fn place_layer_container(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        stack: Vec<ContainerSurfaceTile>,
    ) -> &mut Self {
        self.at(slot.x, slot.y)
            .named(id)
            .layer_container(stack)
            .expect("place layer container");
        self
    }

    fn place_transform(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        kind: TransformKind,
    ) -> &mut Self {
        self.at(slot.x, slot.y)
            .named(id)
            .transform(kind)
            .expect("place transform");
        self
    }

    fn place_output(&mut self, id: impl Into<String>, slot: BoardSlot) -> &mut Self {
        self.at(slot.x, slot.y)
            .named(id)
            .output()
            .expect("place output");
        self
    }

    fn place_flow_control(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        kind: FlowControlKind,
    ) -> &mut Self {
        self.at(slot.x, slot.y)
            .named(id)
            .flow_control(kind)
            .expect("place flow control");
        self
    }

    fn place_layer(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self {
        self.at(slot.x, slot.y)
            .named(id)
            .layer(members)
            .expect("place layer");
        self
    }

    fn place_split(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self {
        self.at(slot.x, slot.y)
            .named(id)
            .split(members)
            .expect("place split");
        self
    }

    fn place_route(
        &mut self,
        id: impl Into<String>,
        slot: BoardSlot,
        members: impl IntoIterator<Item = impl Into<String>>,
    ) -> &mut Self {
        self.at(slot.x, slot.y)
            .named(id)
            .route(members)
            .expect("place route");
        self
    }

    fn bind_input_side(
        &mut self,
        node: impl Into<String>,
        endpoint: InputEndpoint,
        side: SpatialSide,
    ) -> &mut Self {
        let node_id = NodeId::new(node);
        if let Some(tile) = self.tile(&node_id) {
            let _ = Board::bind_input_side(self, &tile, endpoint, side);
        }
        self
    }

    fn bind_output_side(
        &mut self,
        node: impl Into<String>,
        endpoint: OutputEndpoint,
        side: SpatialSide,
    ) -> &mut Self {
        let node_id = NodeId::new(node);
        if let Some(tile) = self.tile(&node_id) {
            let _ = Board::bind_output_side(self, &tile, endpoint, side);
        }
        self
    }

    fn add_explicit_relation(&mut self, relation: RootRelation) -> &mut Self {
        Board::add_explicit_relation(self, relation);
        self
    }

    fn rotate_node_cw(&mut self, node: impl Into<String>) -> &mut Self {
        let node_id = NodeId::new(node);
        if let Some(tile) = self.tile(&node_id) {
            let _ = Board::rotate_node_cw(self, &tile);
        }
        self
    }

    fn rotate_node_ccw(&mut self, node: impl Into<String>) -> &mut Self {
        let node_id = NodeId::new(node);
        if let Some(tile) = self.tile(&node_id) {
            let _ = Board::rotate_node_ccw(self, &tile);
        }
        self
    }
}
