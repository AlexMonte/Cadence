use bevy_ecs::component::Component;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::Reflect;

use crate::domain::{BoardSlot, NodeId, RootSurfaceNodeKind, TileFootprint};

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct TesseraTile {
    pub node_id: NodeId,
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(Component)]
pub struct TilePlacement {
    pub slot: BoardSlot,
    pub footprint: TileFootprint,
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct TileNodeKind {
    pub kind: RootSurfaceNodeKind,
}

#[derive(Debug, Clone, Copy, Default, Component, Reflect)]
#[reflect(Component)]
pub struct NeedsCompile;
