//! Drop-zone feedback for node drag gestures (ghost previews and wobble).

use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::editor::{
    camera::CANVAS_RENDER_LAYER, canvas::CanvasNode, gestures::PointerState,
    state::EditorInteractive,
};

use super::DragContext;

const WOBBLE_FREQUENCY: f32 = 8.0;
const WOBBLE_AMPLITUDE: f32 = 8.0;

/// Tag Component marking a node as being a valid drop target
#[derive(Component, Default)]
pub struct DropZone;

/// Ghost preview of dragged node (semi-transparent copy following cursor)
/// Spawned reactively when dragging over drop zones with wobble-ready architecture
#[derive(Component)]
pub struct GhostPreview {
    /// Original dragged node entity for tracking ownership
    pub dragged_node_entity: Entity,
}

#[derive(Component, Clone, Copy)]
pub struct WobbleBaseline {
    translation: Vec3,
    rotation: Quat,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        animate_node_wobble.run_if(in_state(EditorInteractive)),
    );
    app.add_systems(OnExit(PointerState::DraggingNode), clear_drag_feedback);

    app.add_observer(spawn_ghost_on_drop_zone_enter);
    app.add_observer(despawn_ghost_on_drop_zone_exit);
    app.add_observer(detect_drag_over_drop_zone);
}

/// Spawn ghost preview when pointer enters drop zone during drag
pub fn spawn_ghost_on_drop_zone_enter(
    pointer_over: On<Pointer<Over>>,
    drop_zone: Query<&DropZone>,
    node_drag_state: Res<DragContext>,
    dragged_query: Query<&Sprite, With<CanvasNode>>,
    mut commands: Commands,
) {
    let Some(dragged_entity) = node_drag_state.dragged_node else {
        return;
    };

    if drop_zone.contains(pointer_over.entity)
        && let Ok(sprite) = dragged_query.get(dragged_entity)
    {
        let mut ghost_color = sprite.color;
        ghost_color.set_alpha(0.4);

        commands.spawn((
            Sprite {
                color: ghost_color,
                custom_size: sprite.custom_size,
                ..sprite.clone()
            },
            Transform::from_xyz(0.0, 0.0, 0.04),
            RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
            GhostPreview {
                dragged_node_entity: dragged_entity,
            },
        ));
    }
}

/// Despawn ghost preview when pointer exits drop zone
pub fn despawn_ghost_on_drop_zone_exit(
    _pointer_out: On<Pointer<Out>>,
    ghost_query: Query<(Entity, &GhostPreview)>,
    node_drag_state: Res<DragContext>,
    mut commands: Commands,
) {
    let Some(dragged_entity) = node_drag_state.dragged_node else {
        return;
    };

    for (ghost_entity, ghost) in ghost_query.iter() {
        if ghost.dragged_node_entity == dragged_entity {
            commands.entity(ghost_entity).try_despawn();
        }
    }
}

/// Detect when dragged node is over drop zone
pub fn detect_drag_over_drop_zone(
    pointer_over: On<Pointer<Over>>,
    node: Query<&CanvasNode, With<DropZone>>,
    mut node_drag_state: ResMut<DragContext>,
) {
    if let Ok(zone) = node.get(pointer_over.entity) {
        node_drag_state.hover_zone = Some(zone.node_id);
    }
}

/// Animate wobble on dragged node based on proximity to drop zone
pub fn animate_node_wobble(
    time: Res<Time>,
    node_drag_state: Res<DragContext>,
    ghost_query: Query<&GhostPreview>,
    mut dragged_query: Query<
        (Entity, &mut Transform, Option<&WobbleBaseline>),
        (With<CanvasNode>, Without<GhostPreview>),
    >,
    mut commands: Commands,
) {
    let Some(dragged_entity) = node_drag_state.dragged_node else {
        return;
    };

    let has_ghost = ghost_query
        .iter()
        .any(|g| g.dragged_node_entity == dragged_entity);

    if let Ok((entity, mut transform, baseline)) = dragged_query.get_mut(dragged_entity) {
        if has_ghost {
            let baseline = if let Some(existing) = baseline {
                *existing
            } else {
                let captured = WobbleBaseline {
                    translation: transform.translation,
                    rotation: transform.rotation,
                };
                commands.entity(entity).insert(captured);
                captured
            };

            transform.translation = baseline.translation;
            transform.rotation = baseline.rotation;
            let wobble_angle = (time.elapsed_secs() * WOBBLE_FREQUENCY * std::f32::consts::TAU)
                .sin()
                * (WOBBLE_AMPLITUDE / 360.0);
            transform.rotation = baseline.rotation * Quat::from_rotation_z(wobble_angle);
        } else if let Some(baseline) = baseline {
            transform.translation = baseline.translation;
            transform.rotation = baseline.rotation;
            commands.entity(entity).remove::<WobbleBaseline>();
        }
    }
}

fn clear_drag_feedback(
    mut commands: Commands,
    mut wobbling_nodes: Query<(Entity, &mut Transform, &WobbleBaseline), With<CanvasNode>>,
    ghost_query: Query<Entity, With<GhostPreview>>,
) {
    for (entity, mut transform, baseline) in &mut wobbling_nodes {
        transform.translation = baseline.translation;
        transform.rotation = baseline.rotation;
        commands.entity(entity).remove::<WobbleBaseline>();
    }

    for ghost in ghost_query.iter() {
        commands.entity(ghost).try_despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::NodeId;
    use std::time::Duration;

    // Invariants:
    // - wobble captures baseline once on first proximity frame.
    // - wobble does not drift translation.
    // - proximity loss restores exact baseline and removes WobbleBaseline.
    // - clear_drag_feedback restores all wobbling nodes and despawns ghosts.

    fn setup_app() -> App {
        let mut app = App::new();
        app.insert_resource(DragContext::default());
        app.insert_resource(Time::<()>::default());
        app.add_systems(Update, animate_node_wobble);
        app
    }

    #[test]
    fn wobble_captures_and_restores_baseline() {
        let mut app = setup_app();
        let dragged = app
            .world_mut()
            .spawn((
                CanvasNode {
                    node_id: NodeId::new(),
                    width: 100.0,
                    height: 40.0,
                },
                Transform::from_xyz(4.0, 7.0, 0.3),
            ))
            .id();
        let ghost = app
            .world_mut()
            .spawn(GhostPreview {
                dragged_node_entity: dragged,
            })
            .id();
        app.world_mut().resource_mut::<DragContext>().dragged_node = Some(dragged);
        app.world_mut()
            .resource_mut::<Time<()>>()
            .advance_by(Duration::from_millis(70));

        app.update();

        {
            let entity = app.world().entity(dragged);
            let transform = entity.get::<Transform>().unwrap();
            let baseline = entity.get::<WobbleBaseline>().unwrap();
            assert_eq!(transform.translation, baseline.translation);
            assert_ne!(transform.rotation, baseline.rotation);
        }

        // Remove proximity signal and run again; should restore and clear baseline component.
        app.world_mut().entity_mut(ghost).despawn();

        app.update();
        {
            let entity = app.world().entity(dragged);
            let transform = entity.get::<Transform>().unwrap();
            assert_eq!(transform.translation, Vec3::new(4.0, 7.0, 0.3));
            assert_eq!(transform.rotation, Quat::IDENTITY);
            assert!(
                entity.get::<WobbleBaseline>().is_none(),
                "baseline must be removed after proximity loss"
            );
        }
    }

    #[test]
    fn clear_feedback_restores_transforms_and_despawns_ghosts() {
        let mut app = App::new();
        app.add_systems(Update, clear_drag_feedback);

        let node = app
            .world_mut()
            .spawn((
                CanvasNode {
                    node_id: NodeId::new(),
                    width: 100.0,
                    height: 40.0,
                },
                Transform::from_xyz(20.0, -5.0, 1.0),
                WobbleBaseline {
                    translation: Vec3::new(1.0, 2.0, 1.0),
                    rotation: Quat::IDENTITY,
                },
            ))
            .id();
        let ghost = app
            .world_mut()
            .spawn(GhostPreview {
                dragged_node_entity: node,
            })
            .id();

        app.update();

        let entity = app.world().entity(node);
        let transform = entity.get::<Transform>().unwrap();
        assert_eq!(transform.translation, Vec3::new(1.0, 2.0, 1.0));
        assert!(entity.get::<WobbleBaseline>().is_none());
        assert!(
            app.world().get_entity(ghost).is_err(),
            "ghost preview must be despawned"
        );
    }
}
