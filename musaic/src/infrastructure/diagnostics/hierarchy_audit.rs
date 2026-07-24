//! Runtime audit for Bevy B0004 parent/child component mismatches.

use bevy::{camera::visibility::InheritedVisibility, ecs::hierarchy::ChildOf, prelude::*};
use std::collections::HashSet;

#[derive(Resource, Default)]
struct HierarchyAuditState {
    logged: HashSet<(Entity, Entity, &'static str)>,
}

pub struct HierarchyAuditPlugin;

impl Plugin for HierarchyAuditPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HierarchyAuditState>().add_systems(
            PostUpdate,
            audit_hierarchy_relationships
                .after(TransformSystems::Propagate)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
    }
}

fn audit_hierarchy_relationships(
    mut state: ResMut<HierarchyAuditState>,
    global_children: Query<(Entity, &ChildOf), With<GlobalTransform>>,
    visibility_children: Query<(Entity, &ChildOf), With<InheritedVisibility>>,
    parent_global: Query<(), With<GlobalTransform>>,
    parent_visibility: Query<(), With<InheritedVisibility>>,
    names: Query<&Name>,
) {
    let mut new_violations = 0u32;

    for (child, child_of) in &global_children {
        if parent_global.contains(child_of.parent()) {
            continue;
        }
        if log_violation(
            &mut state,
            child,
            child_of.parent(),
            "GlobalTransform",
            &names,
        ) {
            new_violations += 1;
        }
    }

    for (child, child_of) in &visibility_children {
        if parent_visibility.contains(child_of.parent()) {
            continue;
        }
        if log_violation(
            &mut state,
            child,
            child_of.parent(),
            "InheritedVisibility",
            &names,
        ) {
            new_violations += 1;
        }
    }

    if new_violations > 0 {
        bevy::log::warn!(
            "hierarchy audit: {new_violations} new B0004-style parent/child mismatches (see debug log)"
        );
    }
}

fn log_violation(
    state: &mut HierarchyAuditState,
    child: Entity,
    parent: Entity,
    component: &'static str,
    names: &Query<&Name>,
) -> bool {
    let key = (child, parent, component);
    if !state.logged.insert(key) {
        return false;
    }

    let _child_name = names
        .get(child)
        .map(|n| n.to_string())
        .unwrap_or_else(|_| format!("{child}"));
    let _parent_name = names
        .get(parent)
        .map(|n| n.to_string())
        .unwrap_or_else(|_| format!("{parent}"));

    true
}
