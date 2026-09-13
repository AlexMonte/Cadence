use tessera::prelude::{NodeId, SpatialSide};

mod contextual;
pub use contextual::{ContextualConnectionPlan, plan_contextual_connections, reconnect_after_edit};

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

/// Builds the visible port compass directly from canonical connection bindings.
pub fn connection_endpoint_view(
    queries: &DocumentQueries<'_>,
    node: &NodeId,
    _from_slot: Option<BoardSlot>,
    _to_slot: Option<BoardSlot>,
) -> Option<ConnectionEndpointView> {
    if !matches!(
        queries.location_of(node)?.address,
        crate::domain::document::PlacementAddress::BoardSlot(_)
    ) {
        // Pattern cells compose through order and modifier ownership. Offering
        // root-board sockets here creates controls that cannot be connected.
        return None;
    }
    let program = crate::domain::document::export_document_program(queries.document).ok()?;
    let bindings = crate::domain::document::connection_policy::effective_bindings(&program, node);
    let config = crate::domain::flow::port_config(&bindings);
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
    let state = crate::domain::document::export_document_program(queries.document)
        .ok()
        .map(|program| {
            let bindings =
                crate::domain::document::connection_policy::effective_bindings(&program, from);
            crate::domain::document::port_state_for_side(
                &crate::domain::flow::port_config(&bindings),
                spatial_side,
            )
        })
        .unwrap_or_default();
    match state {
        PortSlotState::Input => PortSlotState::Input,
        _ => PortSlotState::Output,
    }
}
