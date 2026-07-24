//! Board viewport pointer input: middle/right drag pan, shift+middle orbit,
//! edge scroll. Emits [`CameraRequest`]s — never writes camera state directly
//! (the rig in [`super::camera_rig`] owns the pose).

use bevy::{
    camera::Camera,
    prelude::*,
    ui::{ComputedNode, UiGlobalTransform},
    window::{CursorIcon, PrimaryWindow, SystemCursorIcon},
};

use crate::application::editor::{
    BoardViewportPointerInput, CursorInteraction, CursorInteractionChanged, EditorSession,
    MusaicEditorSet, cursor_icon_for,
};

use bevy::state::condition::in_state;

use super::camera_rig::CameraRequest;

/// Marker on the UI node that displays the board render target.
#[derive(Component)]
pub struct UiBoardViewport;

#[derive(Resource, Default)]
pub(crate) struct BoardCameraPointerState {
    pub(crate) viewport_pan_active: bool,
    pub(crate) middle_orbit_active: bool,
    pub(crate) pan_dragging: bool,
    last_cursor: Option<Vec2>,
}

const EDGE_SCROLL_MARGIN_PX: f32 = 56.0;
/// Edge scroll speed in screen-pixels-per-frame-equivalent (converted to pan
/// requests like drag input).
const EDGE_SCROLL_MAX_SPEED: f32 = 90.0;
const ORBIT_SENSITIVITY: f32 = 0.006;

pub struct BoardCameraNavPlugin;

impl Plugin for BoardCameraNavPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoardCameraPointerState>().add_systems(
            Update,
            (
                board_camera_pointer_input,
                sync_board_viewport_pointer_input,
                apply_board_cursor_icon,
                board_camera_edge_scroll,
            )
                .chain()
                .before(MusaicEditorSet::MutateState)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
    }
}

fn board_camera_pointer_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    viewports: Query<(&ComputedNode, &UiGlobalTransform), With<UiBoardViewport>>,
    mut pointer: ResMut<BoardCameraPointerState>,
    mut requests: MessageWriter<CameraRequest>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((node, global)) = viewports.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        pointer.last_cursor = None;
        return;
    };

    let shift = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
    let middle_down = mouse.pressed(MouseButton::Middle);
    let right_down = mouse.pressed(MouseButton::Right);
    let over_viewport = viewport_contains_cursor(cursor, node, global);

    let middle_orbit = middle_down && shift && over_viewport;
    let pan_active = (middle_down && !shift && over_viewport) || (right_down && over_viewport);

    let cursor_delta = pointer
        .last_cursor
        .map(|prev| cursor - prev)
        .unwrap_or(Vec2::ZERO);

    if pan_active && cursor_delta != Vec2::ZERO {
        requests.write(CameraRequest::Pan(cursor_delta));
    }
    if middle_orbit && cursor_delta.x != 0.0 {
        requests.write(CameraRequest::Orbit(-cursor_delta.x * ORBIT_SENSITIVITY));
    }

    pointer.viewport_pan_active = pan_active;
    pointer.middle_orbit_active = middle_orbit;
    pointer.pan_dragging = pan_active && cursor_delta != Vec2::ZERO;
    pointer.last_cursor = Some(cursor);
}

fn sync_board_viewport_pointer_input(
    pointer: Res<BoardCameraPointerState>,
    mut input: ResMut<BoardViewportPointerInput>,
) {
    input.viewport_pan_active = pointer.viewport_pan_active;
    input.middle_orbit_active = pointer.middle_orbit_active;
}

fn apply_board_cursor_icon(
    cursor: Res<CursorInteraction>,
    pointer: Res<BoardCameraPointerState>,
    windows: Query<(Entity, Option<&CursorIcon>), With<PrimaryWindow>>,
    mut commands: Commands,
    mut last_icon: Local<Option<SystemCursorIcon>>,
    mut cursor_changed: MessageReader<CursorInteractionChanged>,
) {
    let icon = CursorIcon::from(cursor_icon_for(cursor.phase(), pointer.pan_dragging));
    let system_icon = icon
        .as_system()
        .copied()
        .unwrap_or(SystemCursorIcon::Default);

    if cursor_changed.read().next().is_none()
        && *last_icon == Some(system_icon)
        && !cursor.is_changed()
        && !pointer.is_changed()
    {
        return;
    }

    if let Ok((entity, existing)) = windows.single() {
        if existing != Some(&icon) {
            commands.entity(entity).insert(icon);
        }
        *last_icon = Some(system_icon);
    }
}

fn board_camera_edge_scroll(
    session: Res<EditorSession>,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    viewports: Query<(&ComputedNode, &UiGlobalTransform), With<UiBoardViewport>>,
    pointer: Res<BoardCameraPointerState>,
    mut requests: MessageWriter<CameraRequest>,
) {
    if session.is_placing_from_drawer()
        || pointer.viewport_pan_active
        || pointer.middle_orbit_active
    {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((node, global)) = viewports.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    if !viewport_contains_cursor(cursor, node, global) {
        return;
    }
    let Some((local, logical_size)) = cursor_in_viewport(cursor, node, global) else {
        return;
    };
    let scroll = edge_scroll_vector(local, logical_size, EDGE_SCROLL_MARGIN_PX);
    if scroll == Vec2::ZERO {
        return;
    }
    // Emit as a screen-space pan; the rig converts to plane units with
    // grab-hand semantics, so scroll toward an edge maps to the negated
    // screen-x / positive screen-y drag that moves the view toward it.
    let delta = Vec2::new(-scroll.x, scroll.y) * EDGE_SCROLL_MAX_SPEED * time.delta_secs();
    requests.write(CameraRequest::Pan(delta));
}

/// Map window cursor position to this board camera's viewport space (for `RenderTarget::Image`).
pub fn board_render_viewport_cursor(
    window_cursor: Vec2,
    node: &ComputedNode,
    global: &UiGlobalTransform,
    camera: &Camera,
) -> Option<Vec2> {
    let cam_viewport_size = camera.logical_viewport_size()?;
    let size = node.size;
    if size.x <= 1.0 || size.y <= 1.0 {
        return None;
    }
    let node_rect = Rect::from_center_size(global.translation.trunc(), size);
    let top_left = node_rect.min * node.inverse_scale_factor();
    let logical_size = size * node.inverse_scale_factor();
    let local = (window_cursor - top_left) / logical_size;
    if !(0.0..=1.0).contains(&local.x) || !(0.0..=1.0).contains(&local.y) {
        return None;
    }
    Some(local * cam_viewport_size)
}

fn viewport_contains_cursor(cursor: Vec2, node: &ComputedNode, global: &UiGlobalTransform) -> bool {
    let size = node.size;
    if size.x <= 1.0 || size.y <= 1.0 {
        return false;
    }
    let node_rect = Rect::from_center_size(global.translation.trunc(), size);
    let top_left = node_rect.min * node.inverse_scale_factor();
    let logical_size = size * node.inverse_scale_factor();
    let local = cursor - top_left;
    local.x >= 0.0 && local.y >= 0.0 && local.x <= logical_size.x && local.y <= logical_size.y
}

/// Cursor position in the viewport's top-left logical space, plus logical size.
///
/// Same coordinate contract as [`viewport_contains_cursor`] / [`edge_scroll_vector`]:
/// `(0, 0)` is the viewport top-left and `(size.x, size.y)` is the bottom-right.
/// Must not use the node's center translation — that makes the whole left/top
/// half of the board look like an edge and pan continuously.
fn cursor_in_viewport(
    cursor: Vec2,
    node: &ComputedNode,
    global: &UiGlobalTransform,
) -> Option<(Vec2, Vec2)> {
    let size = node.size;
    if size.x <= 1.0 || size.y <= 1.0 {
        return None;
    }
    let node_rect = Rect::from_center_size(global.translation.trunc(), size);
    let top_left = node_rect.min * node.inverse_scale_factor();
    let logical_size = size * node.inverse_scale_factor();
    let local = cursor - top_left;
    if local.x < 0.0 || local.y < 0.0 || local.x > logical_size.x || local.y > logical_size.y {
        return None;
    }
    Some((local, logical_size))
}

/// Edge-scroll intensity from a top-left-relative local cursor in `[0, size]`.
///
/// Negative `x` means scroll toward the left edge (camera pans right so content
/// moves left); positive `y` means scroll toward the top edge.
fn edge_scroll_vector(local: Vec2, size: Vec2, margin: f32) -> Vec2 {
    if margin <= 0.0 {
        return Vec2::ZERO;
    }
    let mut out = Vec2::ZERO;
    if local.x < margin {
        out.x = -((margin - local.x) / margin).clamp(0.0, 1.0);
    } else if local.x > size.x - margin {
        out.x = ((local.x - (size.x - margin)) / margin).clamp(0.0, 1.0);
    }
    if local.y < margin {
        out.y = ((margin - local.y) / margin).clamp(0.0, 1.0);
    } else if local.y > size.y - margin {
        out.y = -((local.y - (size.y - margin)) / margin).clamp(0.0, 1.0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_scroll_idle_in_viewport_interior() {
        let size = Vec2::new(800.0, 600.0);
        let margin = 56.0;
        assert_eq!(
            edge_scroll_vector(Vec2::new(400.0, 300.0), size, margin),
            Vec2::ZERO
        );
        assert_eq!(
            edge_scroll_vector(Vec2::new(margin, margin), size, margin),
            Vec2::ZERO
        );
        assert_eq!(
            edge_scroll_vector(Vec2::new(size.x - margin, size.y - margin), size, margin),
            Vec2::ZERO
        );
    }

    #[test]
    fn edge_scroll_fires_only_inside_margin() {
        let size = Vec2::new(800.0, 600.0);
        let margin = 56.0;

        let left = edge_scroll_vector(Vec2::new(0.0, 300.0), size, margin);
        assert!(left.x < 0.0 && left.y == 0.0);

        let right = edge_scroll_vector(Vec2::new(size.x, 300.0), size, margin);
        assert!(right.x > 0.0 && right.y == 0.0);

        let top = edge_scroll_vector(Vec2::new(400.0, 0.0), size, margin);
        assert!(top.y > 0.0 && top.x == 0.0);

        let bottom = edge_scroll_vector(Vec2::new(400.0, size.y), size, margin);
        assert!(bottom.y < 0.0 && bottom.x == 0.0);
    }
}
