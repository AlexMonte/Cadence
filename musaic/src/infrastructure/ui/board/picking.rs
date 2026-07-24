use bevy::prelude::*;

use crate::application::editor::interaction::{BoardPickEvent, BoardPickHit, BoardPickTargetKind};
use crate::application::editor::{
    BoardCursorHover, BoardPlacementPointer, CursorInteraction, EditorSession,
    PickHit, blocks_board_picks_with_cursor,
};
use crate::application::pipeline::scene_sync::VisibleBoardState;

use super::components::*;
use super::helpers::SLOT_HEIGHT;

pub(super) fn on_tile_clicked(
    mut event: On<'_, '_, Pointer<Click>>,
    session: Res<'_, EditorSession>,
    cursor: Res<'_, CursorInteraction>,
    targets: Query<'_, '_, &BoardPickTarget>,
    mut pick_events: MessageWriter<'_, BoardPickEvent>,
) {
    if blocks_board_picks_with_cursor(&session, cursor.phase()) {
        event.propagate(false);
        return;
    }
    let entity = event.original_event_target();
    let Ok(target) = targets.get(entity) else {
        return;
    };
    let Some(position) = event.hit.position else {
        return;
    };

    pick_events.write(BoardPickEvent::Hit(BoardPickHit {
        surface_id: target.surface_id,
        kind: target.kind.clone(),
        world_position: position,
        distance: 0.0,
    }));
    event.propagate(false);
}

/// Board base clicks route through the board projection (`VisibleBoardState`):
/// empty slots and stack inserts become pick hits; anything else is a miss
/// that clears editor focus. Drag hover/commit are owned by the placement
/// pointer + editor session, never by picking drag events.
pub(super) fn on_board_base_clicked(
    mut event: On<'_, '_, Pointer<Click>>,
    session: Res<'_, EditorSession>,
    cursor: Res<'_, CursorInteraction>,
    visible: Res<'_, VisibleBoardState>,
    surfaces: Query<'_, '_, &BoardDragSurface>,
    mut pick_events: MessageWriter<'_, BoardPickEvent>,
) {
    if blocks_board_picks_with_cursor(&session, cursor.phase()) {
        event.propagate(false);
        return;
    }
    let entity = event.original_event_target();
    let Ok(base) = surfaces.get(entity) else {
        return;
    };
    let Some(position) = event.hit.position else {
        return;
    };

    let pick = crate::domain::board::geometry::slot_at_world_position(position, visible.layout)
        .and_then(|slot| visible.pick_at(slot))
        .and_then(|hit| match hit {
            PickHit::EmptySlot { slot, .. } => Some(BoardPickTargetKind::Slot { slot }),
            PickHit::StackInsert { index, .. } => Some(BoardPickTargetKind::StackInsert { index }),
            _ => None,
        });

    match pick {
        Some(kind) => {
            pick_events.write(BoardPickEvent::Hit(BoardPickHit {
                surface_id: base.surface,
                kind,
                world_position: position,
                distance: 0.0,
            }));
        }
        None => {
            pick_events.write(BoardPickEvent::Miss);
        }
    }
    event.propagate(false);
}

pub(super) fn sync_board_cursor_hover(
    pointer: Res<BoardPlacementPointer>,
    visible: Res<VisibleBoardState>,
    session: Res<EditorSession>,
    mut hover: ResMut<BoardCursorHover>,
) {
    hover.pickable = false;
    if session.is_placing_from_drawer() {
        return;
    }
    let Some(world) = pointer.cursor_world else {
        return;
    };
    let Some(slot) = crate::domain::board::geometry::slot_at_world_position(world, visible.layout)
    else {
        return;
    };
    if visible.pick_at(slot).is_some() {
        hover.pickable = true;
    }
}

/// Samples pointer position against the board viewport (no application session mutation).
pub(super) fn sample_board_placement_pointer(
    windows: Query<&Window>,
    board_camera: Query<(&Camera, &GlobalTransform), With<Board3dCamera>>,
    viewports: Query<
        (&ComputedNode, &UiGlobalTransform),
        With<crate::infrastructure::ui::board_camera_nav::UiBoardViewport>,
    >,
    mut pointer: ResMut<BoardPlacementPointer>,
) {
    pointer.window_cursor = windows.iter().next().and_then(|w| w.cursor_position());
    pointer.viewport_cursor = None;
    pointer.cursor_world = None;

    let Some(window_cursor) = pointer.window_cursor else {
        return;
    };
    let Ok((camera, camera_transform)) = board_camera.single() else {
        return;
    };
    let Ok((node, global)) = viewports.single() else {
        return;
    };
    let Some(viewport_cursor) =
        crate::infrastructure::ui::board_camera_nav::board_render_viewport_cursor(
            window_cursor,
            node,
            global,
            camera,
        )
    else {
        return;
    };
    pointer.viewport_cursor = Some(viewport_cursor);

    let Ok(ray) = camera.viewport_to_world(camera_transform, viewport_cursor) else {
        return;
    };
    let direction = ray.direction.normalize();
    if direction.y.abs() < 1e-5 {
        return;
    }
    let t = (SLOT_HEIGHT - ray.origin.y) / direction.y;
    if t < 0.0 {
        return;
    }
    pointer.cursor_world = Some(ray.origin + direction * t);
}

