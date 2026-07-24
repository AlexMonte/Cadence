use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tessera::prelude::{NodeId, SpatialSide};

/// Authoring state for a port on one side of a tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PortSlotState {
    #[default]
    None,
    Input,
    Output,
}

impl PortSlotState {
    pub fn label(self) -> &'static str {
        match self {
            PortSlotState::None => "-",
            PortSlotState::Input => "I",
            PortSlotState::Output => "O",
        }
    }

    pub fn is_bindable(self) -> bool {
        matches!(self, PortSlotState::Input | PortSlotState::Output)
    }

    pub fn disconnects(self) -> bool {
        matches!(self, PortSlotState::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortEndpointConfig {
    pub north: PortSlotState,
    pub east: PortSlotState,
    pub south: PortSlotState,
    pub west: PortSlotState,
}

impl Default for PortEndpointConfig {
    fn default() -> Self {
        Self {
            north: PortSlotState::None,
            east: PortSlotState::None,
            south: PortSlotState::None,
            west: PortSlotState::None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PortEndpointStore {
    pub by_node: BTreeMap<NodeId, PortEndpointConfig>,
}

impl PortEndpointStore {
    pub fn config_for(&self, node: &NodeId) -> PortEndpointConfig {
        self.by_node.get(node).copied().unwrap_or_default()
    }

    pub fn side_state(&self, node: &NodeId, side: SpatialSide) -> PortSlotState {
        port_state_for_side(&self.config_for(node), side)
    }

    pub fn set_side(&mut self, node: &NodeId, side: SpatialSide, state: PortSlotState) {
        let config = self.by_node.entry(node.clone()).or_default();
        set_port_state(config, side, state);
    }

    pub fn retain_nodes(&mut self, keep: impl Fn(&NodeId) -> bool) {
        self.by_node.retain(|node, _| keep(node));
    }
}

pub fn cycle_port_state(current: PortSlotState) -> PortSlotState {
    match current {
        PortSlotState::None => PortSlotState::Input,
        PortSlotState::Input => PortSlotState::Output,
        PortSlotState::Output => PortSlotState::None,
    }
}

pub fn port_state_for_side(config: &PortEndpointConfig, side: SpatialSide) -> PortSlotState {
    match side {
        SpatialSide::North => config.north,
        SpatialSide::East => config.east,
        SpatialSide::South => config.south,
        SpatialSide::West => config.west,
        _ => PortSlotState::None,
    }
}

pub fn set_port_state(config: &mut PortEndpointConfig, side: SpatialSide, state: PortSlotState) {
    match side {
        SpatialSide::North => config.north = state,
        SpatialSide::East => config.east = state,
        SpatialSide::South => config.south = state,
        SpatialSide::West => config.west = state,
        _ => {}
    }
}

/// Default outbound port kind for a new connection based on tile geometry.
pub fn default_connection_kind(from_x: i32, to_x: i32) -> PortSlotState {
    if from_x == to_x {
        PortSlotState::Input
    } else {
        PortSlotState::Output
    }
}

/// Deserialize legacy persisted port states into the simplified model.
pub fn migrate_legacy_port_state(raw: &str) -> PortSlotState {
    match raw {
        "Scalar" | "ControlMap" | "Closed" => PortSlotState::Input,
        "Off" | "None" | "-" => PortSlotState::None,
        "Input" | "I" => PortSlotState::Input,
        "Output" | "O" => PortSlotState::Output,
        _ => PortSlotState::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_port_state_order() {
        assert_eq!(cycle_port_state(PortSlotState::None), PortSlotState::Input);
        assert_eq!(
            cycle_port_state(PortSlotState::Input),
            PortSlotState::Output
        );
        assert_eq!(cycle_port_state(PortSlotState::Output), PortSlotState::None);
    }

    #[test]
    fn vertical_adjacency_defaults_to_input() {
        assert_eq!(default_connection_kind(2, 2), PortSlotState::Input);
    }

    #[test]
    fn horizontal_adjacency_defaults_to_output() {
        assert_eq!(default_connection_kind(1, 2), PortSlotState::Output);
    }

    #[test]
    fn legacy_scalar_maps_to_input() {
        assert_eq!(migrate_legacy_port_state("Scalar"), PortSlotState::Input);
        assert_eq!(migrate_legacy_port_state("Off"), PortSlotState::None);
    }
}
