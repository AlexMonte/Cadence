use serde::{Deserialize, Serialize};
use tessera::prelude::{NodeSpatialBindings, SpatialSide};

/// Paint-only role for one visible side of a tile.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortSlotState {
    #[default]
    None,
    Input,
    Output,
}

impl PortSlotState {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "-",
            Self::Input => "I",
            Self::Output => "O",
        }
    }

    pub fn is_bindable(self) -> bool {
        self != Self::None
    }

    pub fn disconnects(self) -> bool {
        self == Self::None
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortEndpointConfig {
    pub north: PortSlotState,
    pub east: PortSlotState,
    pub south: PortSlotState,
    pub west: PortSlotState,
}

pub fn port_state_for_side(config: &PortEndpointConfig, side: SpatialSide) -> PortSlotState {
    match side {
        SpatialSide::North => config.north,
        SpatialSide::East => config.east,
        SpatialSide::South => config.south,
        SpatialSide::West => config.west,
        SpatialSide::Off => PortSlotState::None,
    }
}

pub fn set_port_state(config: &mut PortEndpointConfig, side: SpatialSide, state: PortSlotState) {
    match side {
        SpatialSide::North => config.north = state,
        SpatialSide::East => config.east = state,
        SpatialSide::South => config.south = state,
        SpatialSide::West => config.west = state,
        SpatialSide::Off => {}
    }
}

pub fn port_config(bindings: &NodeSpatialBindings) -> PortEndpointConfig {
    let mut config = PortEndpointConfig::default();
    for side in bindings
        .inputs
        .values()
        .copied()
        .filter(|side| side.is_enabled())
    {
        set_port_state(&mut config, side, PortSlotState::Input);
    }
    for side in bindings
        .outputs
        .values()
        .copied()
        .filter(|side| side.is_enabled())
    {
        set_port_state(&mut config, side, PortSlotState::Output);
    }
    config
}

pub fn cycle_port_state(current: PortSlotState) -> PortSlotState {
    match current {
        PortSlotState::None => PortSlotState::Input,
        PortSlotState::Input => PortSlotState::Output,
        PortSlotState::Output => PortSlotState::None,
    }
}
