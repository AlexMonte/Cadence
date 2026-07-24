//! Bevy ECS integration for Tessera — intended for Cadence and other hosts.

mod components;
mod events;
mod plugin;
mod reflect;
mod resources;
mod systems;

pub use components::*;
pub use events::*;
pub use plugin::TesseraPlugin;
pub use reflect::{register_tessera_types, type_registry_contains};
pub use resources::*;
pub use systems::*;
