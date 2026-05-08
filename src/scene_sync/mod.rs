use std::collections::BTreeMap;

use bevy::prelude::*;
use tessera::prelude::{ContainerId, NodeId};

use crate::app::CadenceSet;

#[derive(Resource, Debug, Clone, Default)]
pub struct NodeEntityMap {
    pub by_node: BTreeMap<NodeId, Entity>,
    pub by_entity: BTreeMap<Entity, NodeId>,
}

#[derive(Component, Debug, Clone)]
pub struct TileEntity {
    pub node: NodeId,
}

#[derive(Component, Debug, Clone)]
pub struct AtomEntity {
    pub container: ContainerId,
    pub index: usize,
}

pub struct SceneSyncPlugin;

impl Plugin for SceneSyncPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<NodeEntityMap>()
            .add_systems(Update, mark_scene_clean.in_set(CadenceSet::SceneSync));
    }
}

fn mark_scene_clean(mut project: ResMut<'_, crate::document::CadenceProject>) {
    if project.runtime.dirty.scene {
        project.runtime.dirty.scene = false;
    }
}
