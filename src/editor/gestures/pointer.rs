//! Pointer gesture state machine and intent emission for click, drag, and context actions.

use crate::core::NodeId;
use crate::editor::camera::CanvasCamera;
use crate::editor::canvas::CanvasNode;
use crate::editor::input::{Cursor, Modifiers, PointerDownTarget, PointerPosition, PointerTarget};
use crate::editor::screens::editor::InEditor;
use crate::editor::semantics::SelectionIntent;
use crate::editor::state::EditorInteractive;
use crate::editor::ui::interactions::InteractiveChild;
use bevy::prelude::*;
use bevy::state::state::NextState;

pub(super) fn plugin(app: &mut App) {
    app.add_message::<LayoutChanged>();
    app.add_message::<ContextMenuIntent>();
    app.add_sub_state::<PointerState>();
    app.add_observer(pointer_press_start);
    app.add_observer(pointer_drag_update);
    app.add_observer(pointer_press_end);
}
const DRAG_OFFSET: f32 = 15.0;
const MIN_PAN_ZOOM: f32 = 0.0001;
/// Message emitted when the layout (positions, camera) has been committed/changed
/// Uses Message for potential system ordering needs (e.g., undo/redo, autosave)
#[derive(Message, Debug, Clone, Copy)]
pub struct LayoutChanged {
    pub scope: Option<crate::core::ScopeId>,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct ContextMenuIntent {
    pub target: ContextMenuTarget,
    pub world_pos: Vec2,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextMenuTarget {
    Canvas,
    Node(NodeId),
    Widget(NodeId),
}

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(InEditor = InEditor)]
pub enum PointerState {
    #[default]
    Idle,
    Armed,
    DraggingNode,
    MarqueeSelecting,
    PanningCanvas,
    InteractingWithWidget,
}

pub enum PointerIntent {
    Click,
    Drag,
    Scroll,
    Hover,
}

/// Root observer: capture a press start and record the pending context.
/// Press target is resolved by ancestry so nested widget children map to either
/// `Widget(node)` or `NodeRoot(node)` instead of being misclassified as canvas/UI.
pub fn pointer_press_start(
    ev: On<Pointer<Press>>,
    pointer: Single<(&mut PointerPosition, &mut PointerTarget), With<Cursor>>,
    canvas_camera: Single<(&GlobalTransform, &Camera), With<CanvasCamera>>,
    nodes: Query<&CanvasNode>,
    ui_nodes: Query<(), With<Node>>,
    interactive_children: Query<(), With<InteractiveChild>>,
    parents: Query<&ChildOf>,
    state: Res<State<PointerState>>,
    editor_interactive: Option<Res<State<EditorInteractive>>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<PointerState>>,
) {
    if ev.entity != ev.original_event_target() {
        return;
    }

    if editor_interactive.is_none() {
        return;
    }
    // Only start a new pending gesture when idle.
    if *state != PointerState::Idle {
        return;
    }

    // Pointer event locations are in viewport space and are monitor/DPI safe.
    let screen = ev.pointer_location.position;
    let (&transform, camera) = canvas_camera.into_inner();
    let modifiers = Modifiers::new(
        keyboard_input.pressed(KeyCode::AltLeft) || keyboard_input.pressed(KeyCode::AltRight),
        keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight),
        keyboard_input.pressed(KeyCode::ControlLeft)
            || keyboard_input.pressed(KeyCode::ControlRight),
    );
    let Ok(world) = camera.viewport_to_world_2d(&transform, screen) else {
        return;
    };

    let resolved_node = resolve_node_root(ev.entity, &nodes, &parents)
        .map(|(node_entity, canvas_node)| (node_entity, canvas_node.node_id));
    // Determine what we pressed on.
    let target = if interactive_children.contains(ev.entity) {
        if let Some((node_entity, node_id)) = resolved_node {
            PointerDownTarget::Widget(node_entity, node_id)
        } else if has_ui_ancestor(ev.entity, &ui_nodes, &parents) {
            PointerDownTarget::Ui
        } else {
            PointerDownTarget::Canvas
        }
    } else if let Some((node_entity, node_id)) = resolved_node {
        PointerDownTarget::NodeRoot(node_entity, node_id)
    } else if has_ui_ancestor(ev.entity, &ui_nodes, &parents) {
        PointerDownTarget::Ui
    } else {
        PointerDownTarget::Canvas
    };
    let (mut pointer_pos, mut target_comp) = pointer.into_inner();
    pointer_pos.begin_press(screen, world);
    target_comp.begin_press(target, ev.button, modifiers);
    next.set(PointerState::Armed);
}

/// Root observer: while the button is held, accumulate movement and promote to a mode
/// once the threshold is exceeded.
pub fn pointer_drag_update(
    ev: On<Pointer<Drag>>,
    pointer: Single<(&mut PointerPosition, &mut PointerTarget), With<Cursor>>,
    camera: Single<(&GlobalTransform, &Camera, &mut Transform), With<CanvasCamera>>,
    state: Res<State<PointerState>>,
    editor_interactive: Option<Res<State<EditorInteractive>>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut next: ResMut<NextState<PointerState>>,
) {
    if ev.entity != ev.original_event_target() {
        return;
    }

    if editor_interactive.is_none() {
        return;
    }
    // Update cursor positions.
    let cursor_position = ev.pointer_location.position;
    let (&camera_global_transform, camera, mut camera_transform) = camera.into_inner();

    let (mut pointer_pos, target_comp) = pointer.into_inner();
    if let Ok(world) = camera.viewport_to_world_2d(&camera_global_transform, cursor_position) {
        pointer_pos.update_cursor(cursor_position, world);
    }

    if *state == PointerState::PanningCanvas {
        pan_canvas_from_drag(ev.delta, &mut camera_transform);
        return;
    }

    // Only promote out of Armed.
    if *state != PointerState::Armed {
        return;
    }

    if pointer_pos.has_exceeded_drag_threshold(DRAG_OFFSET) {
        let space_pressed = keyboard_input.pressed(KeyCode::Space);
        if matches!(target_comp.down_button, Some(PointerButton::Middle))
            || (space_pressed && matches!(target_comp.down_button, Some(PointerButton::Primary)))
        {
            next.set(PointerState::PanningCanvas);
            return;
        }

        match target_comp.down_target {
            Some(PointerDownTarget::NodeRoot(..)) => {
                next.set(PointerState::DraggingNode);
            }
            Some(PointerDownTarget::Widget(..)) | Some(PointerDownTarget::Ui) => {
                next.set(PointerState::InteractingWithWidget);
            }
            Some(PointerDownTarget::Canvas) | None => match target_comp.down_button {
                Some(PointerButton::Primary) => next.set(PointerState::MarqueeSelecting),
                Some(PointerButton::Middle) => next.set(PointerState::PanningCanvas),
                _ => {
                    // Secondary drag on canvas intentionally stays Armed; context intent is emitted on release.
                }
            },
        }
    }
}

fn pan_canvas_from_drag(delta: Vec2, camera_transform: &mut Transform) {
    let zoom = camera_transform.scale.x.max(MIN_PAN_ZOOM);
    camera_transform.translation.x -= delta.x / zoom;
    camera_transform.translation.y += delta.y / zoom;
}

/// Root observer: clear pending press context on release.
/// Modules should finalize their work via `OnExit(PointerMode::X)` systems.
pub fn pointer_press_end(
    ev: On<Pointer<Release>>,
    pointer: Single<(&mut PointerPosition, &mut PointerTarget), With<Cursor>>,
    state: Res<State<PointerState>>,
    editor_interactive: Option<Res<State<EditorInteractive>>>,
    mut next: ResMut<NextState<PointerState>>,
    mut intents: MessageWriter<SelectionIntent>,
    mut context_menu_intents: MessageWriter<ContextMenuIntent>,
) {
    if ev.entity != ev.original_event_target() {
        return;
    }

    let (mut pointer_pos, mut target_comp) = pointer.into_inner();
    let is_interactive = editor_interactive.is_some();

    // Promote click intent out of Armed.
    if is_interactive && *state == PointerState::Armed {
        let modifiers = target_comp.modifiers();
        let down_target = target_comp.down_target();
        let down_button = target_comp.down_button;
        let is_secondary = matches!(down_button, Some(PointerButton::Secondary));

        if is_secondary && let Some(context_target) = context_target_from_down_target(down_target) {
            context_menu_intents.write(ContextMenuIntent {
                target: context_target,
                world_pos: pointer_pos.world_pos(),
                modifiers,
            });
        }

        match down_target {
            Some(PointerDownTarget::NodeRoot(_, node_id))
            | Some(PointerDownTarget::Widget(_, node_id)) => {
                intents.write(SelectionIntent::ClickNode { node_id, modifiers });
            }
            Some(PointerDownTarget::Canvas) | None if !is_secondary => {
                intents.write(SelectionIntent::ClickCanvas { modifiers });
            }
            Some(PointerDownTarget::Canvas) | Some(PointerDownTarget::Ui) | None => {}
        }
    }

    // Always return to idle on release (finalization should happen on exit of the active mode).
    next.set(PointerState::Idle);
    pointer_pos.end_press();
    target_comp.end_press();
}

fn context_target_from_down_target(
    down_target: Option<PointerDownTarget>,
) -> Option<ContextMenuTarget> {
    match down_target {
        Some(PointerDownTarget::Canvas) => Some(ContextMenuTarget::Canvas),
        Some(PointerDownTarget::NodeRoot(_, node_id)) => Some(ContextMenuTarget::Node(node_id)),
        Some(PointerDownTarget::Widget(_, node_id)) => Some(ContextMenuTarget::Widget(node_id)),
        Some(PointerDownTarget::Ui) | None => None,
    }
}

fn resolve_node_root<'a>(
    entity: Entity,
    nodes: &'a Query<&CanvasNode>,
    parents: &Query<&ChildOf>,
) -> Option<(Entity, &'a CanvasNode)> {
    let mut current = Some(entity);
    while let Some(candidate) = current {
        if let Ok(node) = nodes.get(candidate) {
            return Some((candidate, node));
        }
        current = parents.get(candidate).ok().map(|parent| parent.parent());
    }
    None
}

fn has_ui_ancestor(
    entity: Entity,
    ui_nodes: &Query<(), With<Node>>,
    parents: &Query<&ChildOf>,
) -> bool {
    let mut current = Some(entity);
    while let Some(candidate) = current {
        if ui_nodes.contains(candidate) {
            return true;
        }
        current = parents.get(candidate).ok().map(|parent| parent.parent());
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::NodeId;

    // Invariants:
    // - direct node target resolves to itself.
    // - child target resolves to nearest ancestor CanvasNode.
    // - entities without node ancestors resolve to None.

    #[derive(Resource)]
    struct TargetEntity(Entity);

    #[derive(Resource, Default)]
    struct Resolved(Option<(Entity, NodeId)>);

    fn run_resolve_system(
        target: Res<TargetEntity>,
        nodes: Query<&CanvasNode>,
        parents: Query<&ChildOf>,
        mut resolved: ResMut<Resolved>,
    ) {
        resolved.0 = resolve_node_root(target.0, &nodes, &parents)
            .map(|(entity, node)| (entity, node.node_id));
    }

    #[test]
    fn resolve_node_root_invariants() {
        let mut app = App::new();
        app.insert_resource(Resolved::default());
        app.add_systems(Update, run_resolve_system);

        let node_id = NodeId::new();
        let node_entity = app.world_mut().spawn(CanvasNode::new(node_id)).id();
        let child_entity = app.world_mut().spawn_empty().id();
        app.world_mut()
            .entity_mut(node_entity)
            .add_child(child_entity);
        let orphan = app.world_mut().spawn_empty().id();

        app.insert_resource(TargetEntity(node_entity));
        app.update();
        {
            let resolved = app.world().resource::<Resolved>();
            assert_eq!(resolved.0, Some((node_entity, node_id)));
        }

        app.insert_resource(TargetEntity(child_entity));
        app.update();
        {
            let resolved = app.world().resource::<Resolved>();
            assert_eq!(resolved.0, Some((node_entity, node_id)));
        }

        app.insert_resource(TargetEntity(orphan));
        app.update();
        {
            let resolved = app.world().resource::<Resolved>();
            assert_eq!(resolved.0, None);
        }
    }

    #[test]
    fn pan_canvas_from_drag_scales_by_zoom() {
        let mut transform = Transform {
            translation: Vec3::new(10.0, 20.0, 0.0),
            scale: Vec3::splat(2.0),
            ..Default::default()
        };

        pan_canvas_from_drag(Vec2::new(4.0, 6.0), &mut transform);

        assert_eq!(transform.translation.x, 8.0);
        assert_eq!(transform.translation.y, 23.0);
    }

    #[test]
    fn context_target_maps_pointer_down_targets() {
        let node_id = NodeId::new();
        assert_eq!(
            context_target_from_down_target(Some(PointerDownTarget::Canvas)),
            Some(ContextMenuTarget::Canvas)
        );
        assert_eq!(
            context_target_from_down_target(Some(PointerDownTarget::NodeRoot(
                Entity::PLACEHOLDER,
                node_id
            ))),
            Some(ContextMenuTarget::Node(node_id))
        );
        assert_eq!(
            context_target_from_down_target(Some(PointerDownTarget::Widget(
                Entity::PLACEHOLDER,
                node_id
            ))),
            Some(ContextMenuTarget::Widget(node_id))
        );
        assert_eq!(
            context_target_from_down_target(Some(PointerDownTarget::Ui)),
            None
        );
        assert_eq!(context_target_from_down_target(None), None);
    }
}
