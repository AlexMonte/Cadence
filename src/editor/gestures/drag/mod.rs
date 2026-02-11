//! Node drag gesture orchestration and drag intent emission.

use crate::EditorSystemSet;
use crate::core::NodeId;
use crate::editor::canvas::NodeStackIntent;
use crate::editor::gestures::PointerState;
use crate::editor::input::{Cursor, PointerDownTarget, PointerPosition, PointerTarget};
use crate::editor::semantics::drag::DragIntent;
use crate::editor::semantics::{SelectionIntent, SelectionState};
use crate::editor::state::EditorInteractive;

use bevy::prelude::*;

pub mod dropzone;

pub use dropzone::DropZone;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<DragContext>();
    app.add_systems(
        OnEnter(PointerState::DraggingNode),
        begin_node_drag.run_if(in_state(EditorInteractive)),
    );
    app.add_systems(
        Update,
        emit_drag_update
            .run_if(in_state(PointerState::DraggingNode))
            .run_if(in_state(EditorInteractive))
            .in_set(EditorSystemSet::DragComputation),
    );
    app.add_systems(OnExit(PointerState::DraggingNode), end_node_drag);
    app.add_plugins(dropzone::plugin);
}

#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct DragContext {
    pub dragged_node: Option<Entity>,
    pub hover_zone: Option<NodeId>,
}

fn begin_node_drag(
    pointer: Single<(&PointerPosition, &PointerTarget), With<Cursor>>,
    selection: Res<SelectionState>,
    mut drag_context: ResMut<DragContext>,
    mut drag_intents: MessageWriter<DragIntent>,
    mut selection_intents: MessageWriter<SelectionIntent>,
    mut stack_intents: MessageWriter<NodeStackIntent>,
) {
    let (pointer_position, pointer_target) = pointer.into_inner();

    let Some(start_world) = pointer_position.down_world_pos() else {
        return;
    };

    let Some((anchor_entity, anchor_node)) = (match pointer_target.down_target() {
        Some(PointerDownTarget::NodeRoot(entity, node_id))
        | Some(PointerDownTarget::Widget(entity, node_id)) => Some((entity, node_id)),
        _ => None,
    }) else {
        return;
    };

    let modifiers = pointer_target.modifiers();
    let anchor_selected = selection.is_selected(anchor_node);
    let node_ids = if anchor_selected && !selection.selected.is_empty() {
        selection.selected.clone()
    } else {
        vec![anchor_node]
    };

    if !anchor_selected && !modifiers.shift && !modifiers.ctrl {
        selection_intents.write(SelectionIntent::ClickNode {
            node_id: anchor_node,
            modifiers,
        });
    }

    drag_context.dragged_node = Some(anchor_entity);
    drag_context.hover_zone = None;

    stack_intents.write(NodeStackIntent::BringToFront {
        node_ids: node_ids.clone(),
    });

    drag_intents.write(DragIntent::Begin {
        anchor_node,
        node_ids,
        start_world,
    });
}

fn emit_drag_update(
    pointer: Single<&PointerPosition, With<Cursor>>,
    mut drag_intents: MessageWriter<DragIntent>,
) {
    drag_intents.write(DragIntent::Update {
        current_world: pointer.world_pos(),
    });
}

fn end_node_drag(
    mut drag_context: ResMut<DragContext>,
    mut drag_intents: MessageWriter<DragIntent>,
) {
    drag_context.dragged_node = None;
    drag_context.hover_zone = None;
    drag_intents.write(DragIntent::Commit);
}
