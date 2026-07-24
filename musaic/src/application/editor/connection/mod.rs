use tessera::prelude::{NodeId, SpatialSide};

use crate::domain::board::BoardSlot;
use crate::domain::document::{DocumentQueries, PortEndpointConfig};

pub use crate::domain::document::{
    PortSlotState, cycle_port_state, port_state_for_side, set_port_state,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionEndpointView {
    pub node: NodeId,
    pub north: PortSlotState,
    pub east: PortSlotState,
    pub south: PortSlotState,
    pub west: PortSlotState,
}

/// Builds connection endpoint view from the document port store (SSOT).
pub fn connection_endpoint_view(
    queries: &DocumentQueries<'_>,
    node: &NodeId,
    _from_slot: Option<BoardSlot>,
    _to_slot: Option<BoardSlot>,
) -> Option<ConnectionEndpointView> {
    if queries.node(node).is_none() {
        return None;
    }
    let config = queries.document.port_endpoints.config_for(node);
    Some(endpoint_view_from_config(node, &config))
}

pub fn endpoint_view_from_config(
    node: &NodeId,
    config: &PortEndpointConfig,
) -> ConnectionEndpointView {
    ConnectionEndpointView {
        node: node.clone(),
        north: config.north,
        east: config.east,
        south: config.south,
        west: config.west,
    }
}

pub fn connection_wire_kind(
    queries: &DocumentQueries<'_>,
    from: &NodeId,
    spatial_side: SpatialSide,
) -> PortSlotState {
    let state = queries
        .document
        .port_endpoints
        .side_state(from, spatial_side);
    match state {
        PortSlotState::Input => PortSlotState::Input,
        _ => PortSlotState::Output,
    }
}
