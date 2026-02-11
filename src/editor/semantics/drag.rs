//! Drag semantic reducer that applies drag intents to node transforms and persisted layout.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use crate::EditorSystemSet;
use crate::core::{NodeId, default_layout};
use crate::editor::AppState;
use crate::editor::canvas::CanvasNode;
use crate::editor::gestures::pointer::LayoutChanged;
use crate::editor::state::EditorReady;

pub(super) fn plugin(app: &mut App) {
    app.add_message::<DragIntent>();
    app.init_resource::<DragSession>();
    app.add_systems(
        Update,
        apply_drag_intents
            .run_if(in_state(EditorReady))
            .in_set(EditorSystemSet::SelectionResolution),
    );
}

#[derive(Message, Debug, Clone)]
pub enum DragIntent {
    Begin {
        anchor_node: NodeId,
        node_ids: Vec<NodeId>,
        start_world: Vec2,
    },
    Update {
        current_world: Vec2,
    },
    Commit,
    Cancel,
}

#[derive(Resource, Debug, Default)]
pub struct DragSession {
    pub active: bool,
    pub anchor_node: Option<NodeId>,
    pub start_world: Vec2,
    pub current_world: Vec2,
    pub start_positions: HashMap<NodeId, Vec2>,
}

impl DragSession {
    fn clear(&mut self) {
        self.active = false;
        self.anchor_node = None;
        self.start_world = Vec2::ZERO;
        self.current_world = Vec2::ZERO;
        self.start_positions.clear();
    }
}

pub fn apply_drag_intents(
    mut intents: MessageReader<DragIntent>,
    mut session: ResMut<DragSession>,
    mut app_state: ResMut<AppState>,
    mut nodes: Query<(&CanvasNode, &mut Transform)>,
    mut layout_changed: MessageWriter<LayoutChanged>,
) {
    for intent in intents.read() {
        match intent {
            DragIntent::Begin {
                anchor_node,
                node_ids,
                start_world,
            } => begin_drag_session(
                &mut session,
                *anchor_node,
                node_ids,
                *start_world,
                &mut nodes,
            ),
            DragIntent::Update { current_world } => {
                apply_drag_update(&mut session, *current_world, &mut app_state, &mut nodes);
            }
            DragIntent::Commit => {
                if session.active {
                    layout_changed.write(LayoutChanged {
                        scope: Some(app_state.current_scope),
                    });
                }
                session.clear();
            }
            DragIntent::Cancel => {
                session.clear();
            }
        }
    }
}

fn begin_drag_session(
    session: &mut DragSession,
    anchor_node: NodeId,
    node_ids: &[NodeId],
    start_world: Vec2,
    nodes: &mut Query<(&CanvasNode, &mut Transform)>,
) {
    let drag_set: HashSet<NodeId> = node_ids.iter().copied().collect();
    session.clear();
    session.anchor_node = Some(anchor_node);
    session.start_world = start_world;
    session.current_world = start_world;

    for (node, transform) in nodes.iter_mut() {
        if drag_set.contains(&node.node_id) {
            session
                .start_positions
                .insert(node.node_id, transform.translation.truncate());
        }
    }

    session.active = !session.start_positions.is_empty();
}

fn apply_drag_update(
    session: &mut DragSession,
    current_world: Vec2,
    app_state: &mut AppState,
    nodes: &mut Query<(&CanvasNode, &mut Transform)>,
) {
    if !session.active {
        return;
    }

    session.current_world = current_world;
    let delta = session.current_world - session.start_world;

    let mut updates = Vec::with_capacity(session.start_positions.len());
    for (node, mut transform) in nodes.iter_mut() {
        let Some(start_position) = session.start_positions.get(&node.node_id) else {
            continue;
        };

        let new_pos = *start_position + delta;
        transform.translation.x = new_pos.x;
        transform.translation.y = new_pos.y;
        updates.push((node.node_id, new_pos));
    }

    if updates.is_empty() {
        return;
    }

    let scope = app_state.current_scope;
    let layout = app_state
        .project
        .layout
        .layouts
        .entry(scope)
        .or_insert_with(|| default_layout(scope));

    for (node_id, position) in updates {
        layout
            .node_positions
            .insert(node_id, (position.x, position.y));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Project;

    // Invariants:
    // - Begin activates only for resolvable dragged nodes.
    // - Update applies uniform delta and writes layout for dragged subset only.
    // - Commit emits one LayoutChanged and clears session.
    // - Cancel clears session without LayoutChanged.

    fn spawn_canvas_node(app: &mut App, node_id: NodeId, translation: Vec3) {
        app.world_mut().spawn((
            CanvasNode {
                node_id,
                width: 120.0,
                height: 48.0,
            },
            Transform::from_translation(translation),
        ));
    }

    fn app_with_drag_system() -> App {
        let mut app = App::new();
        app.add_message::<DragIntent>();
        app.add_message::<LayoutChanged>();
        app.init_resource::<DragSession>();

        let project = Project::new("drag-test".to_string());
        app.insert_resource(AppState::new(project));
        app.add_systems(Update, apply_drag_intents);
        app
    }

    #[test]
    fn begin_update_commit_updates_layout_and_emits_layout_changed() {
        let mut app = app_with_drag_system();
        let id_a = NodeId::new();
        let id_b = NodeId::new();
        spawn_canvas_node(&mut app, id_a, Vec3::new(10.0, 15.0, 0.0));
        spawn_canvas_node(&mut app, id_b, Vec3::new(-5.0, 3.0, 0.0));

        app.world_mut().write_message(DragIntent::Begin {
            anchor_node: id_a,
            node_ids: vec![id_a, id_b],
            start_world: Vec2::new(0.0, 0.0),
        });
        app.update();
        {
            let session = app.world().resource::<DragSession>();
            assert!(session.active);
            assert_eq!(session.anchor_node, Some(id_a));
            assert_eq!(session.start_positions.len(), 2);
        }

        app.world_mut().write_message(DragIntent::Update {
            current_world: Vec2::new(5.0, -2.0),
        });
        app.update();

        let mut seen = 0;
        {
            let world = app.world_mut();
            let mut query = world.query::<(&CanvasNode, &Transform)>();
            for (node, transform) in query.iter(world) {
                if node.node_id == id_a {
                    seen += 1;
                    assert_eq!(transform.translation.x, 15.0);
                    assert_eq!(transform.translation.y, 13.0);
                } else if node.node_id == id_b {
                    seen += 1;
                    assert_eq!(transform.translation.x, 0.0);
                    assert_eq!(transform.translation.y, 1.0);
                }
            }
        }
        assert_eq!(seen, 2);

        {
            let app_state = app.world().resource::<AppState>();
            let layout = app_state
                .project
                .layout
                .layouts
                .get(&app_state.current_scope)
                .expect("scope layout should exist");
            assert_eq!(layout.node_positions.get(&id_a), Some(&(15.0, 13.0)));
            assert_eq!(layout.node_positions.get(&id_b), Some(&(0.0, 1.0)));
        }

        app.world_mut().write_message(DragIntent::Commit);
        app.update();
        {
            let session = app.world().resource::<DragSession>();
            assert!(!session.active);
            assert!(session.start_positions.is_empty());
            assert_eq!(session.anchor_node, None);
        }

        let mut cursor = app
            .world()
            .resource::<Messages<LayoutChanged>>()
            .get_cursor();
        let events: Vec<_> = cursor
            .read(app.world().resource::<Messages<LayoutChanged>>())
            .copied()
            .collect();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn cancel_clears_session_without_layout_changed() {
        let mut app = app_with_drag_system();
        let id_a = NodeId::new();
        spawn_canvas_node(&mut app, id_a, Vec3::new(1.0, 2.0, 0.0));

        app.world_mut().write_message(DragIntent::Begin {
            anchor_node: id_a,
            node_ids: vec![id_a],
            start_world: Vec2::ZERO,
        });
        app.update();
        app.world_mut().write_message(DragIntent::Cancel);
        app.update();

        let session = app.world().resource::<DragSession>();
        assert!(!session.active);
        assert!(session.start_positions.is_empty());

        let mut cursor = app
            .world()
            .resource::<Messages<LayoutChanged>>()
            .get_cursor();
        let events: Vec<_> = cursor
            .read(app.world().resource::<Messages<LayoutChanged>>())
            .copied()
            .collect();
        assert!(events.is_empty());
    }
}
