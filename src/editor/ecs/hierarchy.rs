//! Hierarchy traversal helpers for ancestor/descendant checks in systems.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

#[derive(SystemParam)]
pub struct HierarchyAccess<'w, 's> {
    parents: Query<'w, 's, &'static ChildOf>,
}

impl<'w, 's> HierarchyAccess<'w, 's> {
    pub fn is_descendant_or_self(&self, entity: Entity, root: Entity) -> bool {
        let mut current = Some(entity);
        while let Some(candidate) = current {
            if candidate == root {
                return true;
            }
            current = self
                .parents
                .get(candidate)
                .ok()
                .map(|parent| parent.parent());
        }
        false
    }
}
