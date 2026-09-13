//! Board camera rig — the single owner of camera pose.
//!
//! Contract (see `docs/ARCHITECTURE.md`):
//! - [`BoardCameraRig`] is written by exactly one system: [`arbitrate_camera_rig`].
//! - The `Board3dCamera` `Transform`/`Projection` is written by exactly one
//!   system: [`smooth_camera_transform`], using frame-rate-independent
//!   exponential damping.
//! - Everything else (OnEnter Editor framing, pointer pan, edge scroll,
//!   surface navigation, focus jumps, placement settles) submits a
//!   [`CameraRequest`]; conflicts are resolved once per frame with the
//!   written priority `FrameSurface > FocusAddress > Pan/Orbit`.

use bevy::{prelude::*, state::condition::in_state};

use crate::{
    application::editor::{ActiveSurfaceChangeReason, ActiveSurfaceChanged, EditorSession},
    application::pipeline::scene_sync::VisibleBoardState,
    domain::board::SurfaceLayoutKind,
    domain::document::{PlacementAddress, StackIndex},
    infrastructure::app::{AppState, MusaicSet},
};

use super::board::Board3dCamera;
use super::board_geometry::{stack_column_center, tessera_slot_center};

pub const ROOT_CAMERA_DISTANCE: f32 = 12.0;
const CONTAINER_CAMERA_DISTANCE: f32 = 9.0;
const TIMELINE_CAMERA_DISTANCE: f32 = 10.5;
pub const ROOT_VIEWPORT_HEIGHT: f32 = 9.0;
const CONTAINER_VIEWPORT_HEIGHT: f32 = 9.0;
const TIMELINE_VIEWPORT_HEIGHT: f32 = 10.0;

/// Screen pixels → board-plane world units for drag pan.
const PAN_SENSITIVITY: f32 = 0.022;

/// Exponential damping rates (per second). Higher = snappier.
const FOLLOW_POSITION_RATE: f32 = 6.5;
const FOLLOW_ROTATION_RATE: f32 = 8.0;
const USER_POSITION_RATE: f32 = 26.0;
const USER_ROTATION_RATE: f32 = 36.0;

/// Rig phases in [`CameraRigSet::Emit`] → `Arbitrate` → `Smooth` order,
/// scheduled after `MusaicSet::SceneSync`.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CameraRigSet {
    Emit,
    Arbitrate,
    Smooth,
}

/// A deliberate or continuous camera intent. Producers never touch the rig.
#[derive(Message, Debug, Clone)]
pub enum CameraRequest {
    /// Frame the active surface (navigation, open document).
    FrameSurface(ActiveSurfaceChangeReason),
    /// Center on a board address (focus jump, minimap click, placement settle).
    FocusAddress(PlacementAddress),
    /// User drag pan, in screen pixels.
    Pan(Vec2),
    /// User orbit, in radians.
    Orbit(f32),
}

/// Explicit viewport actions shared by View and command search. These never edit music.
#[derive(Message, Debug, Clone, Copy)]
pub enum BoardViewAction {
    ZoomIn,
    ZoomOut,
    FitSelection,
    FitBoard,
}

pub const BOARD_VIEW_ACTIONS: [(&str, BoardViewAction); 4] = [
    ("Zoom in", BoardViewAction::ZoomIn),
    ("Zoom out", BoardViewAction::ZoomOut),
    ("Fit selected tiles", BoardViewAction::FitSelection),
    ("Fit whole board", BoardViewAction::FitBoard),
];

#[derive(Debug, Clone, Copy, Default)]
pub enum CameraFraming {
    #[default]
    Surface,
    Free,
    Bounds {
        min: Vec2,
        max: Vec2,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RigMode {
    #[default]
    Follow,
    UserControl,
}

/// The single source of truth for the board camera pose.
#[derive(Resource, Debug, Clone)]
pub struct BoardCameraRig {
    pub focus: Vec3,
    pub pan_offset: Vec2,
    pub orbit_yaw: f32,
    pub distance: f32,
    pub viewport_height: f32,
    pub mode: RigMode,
    pub framing: CameraFraming,
}

impl Default for BoardCameraRig {
    fn default() -> Self {
        Self {
            focus: Vec3::ZERO,
            pan_offset: Vec2::ZERO,
            orbit_yaw: 0.0,
            distance: ROOT_CAMERA_DISTANCE,
            viewport_height: ROOT_VIEWPORT_HEIGHT,
            mode: RigMode::Follow,
            framing: CameraFraming::Surface,
        }
    }
}

impl BoardCameraRig {
    /// The point on the board plane the camera looks at (focus plus user pan).
    pub fn look_target(&self) -> Vec3 {
        self.focus + Vec3::new(self.pan_offset.x, 0.0, self.pan_offset.y)
    }

    pub fn desired_transform(&self) -> Transform {
        isometric_camera_transform(self.look_target(), self.distance, self.orbit_yaw)
    }
}

pub struct CameraRigPlugin;

impl Plugin for CameraRigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BoardCameraRig>()
            .add_message::<CameraRequest>()
            .add_message::<BoardViewAction>()
            .configure_sets(
                Update,
                (
                    CameraRigSet::Emit,
                    CameraRigSet::Arbitrate,
                    CameraRigSet::Smooth,
                )
                    .chain()
                    .after(MusaicSet::SceneSync),
            )
            .add_systems(OnEnter(AppState::Editor), request_enter_editor_frame)
            .add_systems(
                Update,
                (
                    emit_navigation_requests.in_set(CameraRigSet::Emit),
                    arbitrate_camera_rig.in_set(CameraRigSet::Arbitrate),
                    smooth_camera_transform.in_set(CameraRigSet::Smooth),
                )
                    .run_if(in_state(AppState::Editor)),
            );
    }
}

/// Enter-editor framing goes through the request queue so
/// [`arbitrate_camera_rig`] remains the only [`BoardCameraRig`] writer.
fn request_enter_editor_frame(mut requests: MessageWriter<CameraRequest>) {
    requests.write(CameraRequest::FrameSurface(
        ActiveSurfaceChangeReason::OpenDocument,
    ));
}

/// Translates editor navigation state changes into camera requests:
/// surface changes, focus jumps, and end-of-placement settles.
fn emit_navigation_requests(
    mut surface_changes: MessageReader<ActiveSurfaceChanged>,
    session: Res<EditorSession>,
    mut requests: MessageWriter<CameraRequest>,
    mut was_placing: Local<bool>,
) {
    for change in surface_changes.read() {
        requests.write(CameraRequest::FrameSurface(change.reason.clone()));
    }

    // End of a drawer drag settles onto the committed address.
    let placing = session.is_placing_from_drawer();
    if *was_placing && !placing {
        if let Some(address) = session.last_placement_address {
            requests.write(CameraRequest::FocusAddress(address));
        }
    }
    *was_placing = placing;

    // Selection does not move the camera. Surface navigation and explicit
    // FocusAddress messages are the only discrete reframing requests.
}

/// Per-frame outcome of draining the request queue.
#[derive(Debug, Clone, PartialEq)]
enum ResolvedIntent {
    Frame {
        focus: Vec3,
        distance: f32,
        viewport_height: f32,
    },
    Focus(Vec3),
    Navigate {
        pan: Vec2,
        orbit: f32,
    },
    Idle,
}

/// Priority: FrameSurface > FocusAddress > Pan/Orbit. A pan arriving in the
/// same frame as a deliberate jump is dropped, never merged.
fn resolve_requests<'a>(
    requests: impl Iterator<Item = &'a CameraRequest>,
    layout: SurfaceLayoutKind,
    surface_center: Vec3,
    orbit_yaw: f32,
) -> ResolvedIntent {
    let mut frame: Option<&ActiveSurfaceChangeReason> = None;
    let mut focus: Option<PlacementAddress> = None;
    let mut pan = Vec2::ZERO;
    let mut orbit = 0.0;

    for request in requests {
        match request {
            CameraRequest::FrameSurface(reason) => frame = Some(reason),
            CameraRequest::FocusAddress(address) => focus = Some(*address),
            CameraRequest::Pan(delta) => pan += *delta,
            CameraRequest::Orbit(delta) => orbit += *delta,
        }
    }

    if let Some(reason) = frame {
        return ResolvedIntent::Frame {
            focus: surface_center,
            distance: distance_for_surface_change(layout, reason),
            viewport_height: viewport_height_for_surface_change(layout, reason),
        };
    }
    if let Some(address) = focus {
        return ResolvedIntent::Focus(address_world_position(address));
    }
    if pan != Vec2::ZERO || orbit != 0.0 {
        return ResolvedIntent::Navigate {
            pan: screen_delta_to_plane(orbit_yaw, pan, PAN_SENSITIVITY),
            orbit,
        };
    }
    ResolvedIntent::Idle
}

fn apply_intent(rig: &mut BoardCameraRig, intent: ResolvedIntent) {
    match intent {
        ResolvedIntent::Frame {
            focus,
            distance,
            viewport_height,
        } => {
            rig.framing = CameraFraming::Surface;
            rig.focus = focus;
            rig.distance = distance;
            rig.viewport_height = viewport_height;
            rig.pan_offset = Vec2::ZERO;
            rig.orbit_yaw = 0.0;
            rig.mode = RigMode::Follow;
        }
        ResolvedIntent::Focus(position) => {
            rig.framing = CameraFraming::Free;
            rig.focus = Vec3::new(position.x, 0.0, position.z);
            rig.pan_offset = Vec2::ZERO;
            rig.mode = RigMode::Follow;
        }
        ResolvedIntent::Navigate { pan, orbit } => {
            rig.framing = CameraFraming::Free;
            rig.pan_offset += pan;
            rig.orbit_yaw += orbit;
            rig.mode = RigMode::UserControl;
        }
        ResolvedIntent::Idle => {
            rig.mode = RigMode::Follow;
        }
    }
}

/// The only writer of [`BoardCameraRig`].
fn arbitrate_camera_rig(
    mut requests: MessageReader<CameraRequest>,
    mut view_actions: MessageReader<BoardViewAction>,
    visible: Res<VisibleBoardState>,
    mut rig: ResMut<BoardCameraRig>,
    viewports: Query<&ComputedNode, With<super::board_camera_nav::UiBoardViewport>>,
    mut last_aspect: Local<f32>,
) {
    let actions: Vec<_> = view_actions.read().copied().collect();
    let mapped_requests: Vec<_> = requests
        .read()
        .cloned()
        .map(|request| match request {
            CameraRequest::FocusAddress(address) => {
                CameraRequest::FocusAddress(visible.display_address(address))
            }
            other => other,
        })
        .collect();
    let mut intent = resolve_requests(
        mapped_requests.iter(),
        visible.layout,
        visible_surface_center(&visible),
        rig.orbit_yaw,
    );
    let aspect = viewports
        .single()
        .ok()
        .and_then(|node| {
            (node.size.x > 1.0 && node.size.y > 1.0).then_some(node.size.x / node.size.y)
        })
        .unwrap_or(1.5);
    // Pan speed follows zoom, while the rig remains the only pose owner.
    if let ResolvedIntent::Navigate { pan, .. } = &mut intent {
        *pan *= rig.viewport_height / ROOT_VIEWPORT_HEIGHT;
    }
    let surface_frame = matches!(intent, ResolvedIntent::Frame { .. });
    if let ResolvedIntent::Frame {
        viewport_height, ..
    } = &mut intent
    {
        *viewport_height = fitted_viewport_height(&visible, aspect, *viewport_height);
    } else if (*last_aspect - aspect).abs() > 0.001 {
        match rig.framing {
            CameraFraming::Surface if rig.pan_offset == Vec2::ZERO => {
                rig.viewport_height =
                    fitted_viewport_height(&visible, aspect, ROOT_VIEWPORT_HEIGHT);
            }
            CameraFraming::Bounds { min, max } => {
                rig.viewport_height = bounds_viewport_height(min, max, aspect);
            }
            _ => {}
        }
    }
    *last_aspect = aspect;
    if intent != ResolvedIntent::Idle || rig.mode != RigMode::Follow {
        apply_intent(&mut rig, intent);
    }
    // Navigation/opening owns this frame. A stale viewport action must not
    // override the new surface; consumed actions are never replayed later.
    if !surface_frame {
        for action in actions {
            apply_view_action(&mut rig, action, &visible, aspect);
        }
    }
}

fn bounds_viewport_height(min: Vec2, max: Vec2, aspect: f32) -> f32 {
    let size = max - min + Vec2::splat(1.0);
    3.0_f32.max(size.y).max(size.x / aspect.max(0.1))
}

fn apply_view_action(
    rig: &mut BoardCameraRig,
    action: BoardViewAction,
    visible: &VisibleBoardState,
    aspect: f32,
) {
    match action {
        BoardViewAction::ZoomIn | BoardViewAction::ZoomOut => {
            let factor = if matches!(action, BoardViewAction::ZoomIn) {
                0.8
            } else {
                1.25
            };
            // A fitted large project may already exceed the manual zoom range.
            // Zoom out must never unexpectedly zoom in from that framing.
            rig.viewport_height =
                (rig.viewport_height * factor).clamp(3.0, rig.viewport_height.max(24.0));
            rig.framing = CameraFraming::Free;
            rig.mode = RigMode::UserControl;
        }
        BoardViewAction::FitSelection | BoardViewAction::FitBoard => {
            let mut targets = visible.clone();
            if matches!(action, BoardViewAction::FitSelection) {
                let has_selection = targets.nodes.iter().any(|node| node.selected);
                targets.nodes.retain(|node| {
                    if has_selection {
                        node.selected
                    } else {
                        node.focused
                    }
                });
            }
            let Some((min, max)) = visible_surface_bounds(&targets) else {
                return;
            };
            let center = (min + max) * 0.5;
            rig.focus = Vec3::new(center.x, 0.0, center.y);
            rig.pan_offset = Vec2::ZERO;
            rig.orbit_yaw = 0.0;
            rig.viewport_height = bounds_viewport_height(min, max, aspect);
            rig.framing = CameraFraming::Bounds { min, max };
            rig.mode = RigMode::Follow;
        }
    }
}

fn fitted_viewport_height(visible: &VisibleBoardState, aspect: f32, minimum: f32) -> f32 {
    if visible.layout == SurfaceLayoutKind::Stack {
        // Stack framing is centered on the complete editing canvas, even when
        // only its first few cells contain notes. Fit that same canvas so the
        // first column cannot disappear behind a wider inspector.
        let width = crate::domain::board::geometry::STACK_COLUMNS as f32 + 1.0;
        let height = stack_canvas_rows(visible) as f32 + 1.0;
        return minimum.max(height).max(width / aspect.max(0.1));
    }
    let Some((min, max)) = visible_surface_bounds(visible) else {
        return minimum;
    };
    let size = max - min + Vec2::splat(1.5);
    minimum.max(size.y).max(size.x / aspect.max(0.1))
}

fn visible_surface_bounds(visible: &VisibleBoardState) -> Option<(Vec2, Vec2)> {
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for node in &visible.nodes {
        let footprint = node
            .tessera_footprint
            .unwrap_or(tessera::prelude::TileFootprint::unit());
        let world = super::board_geometry::placement_center_for_address(
            visible.display_address(node.address),
            footprint,
            node.kind,
            0.92,
            0.0,
        );
        let center = Vec2::new(world.x, world.z);
        let half = Vec2::new(footprint.width as f32, footprint.height as f32) * 0.5;
        min = min.min(center - half);
        max = max.max(center + half);
    }
    min.is_finite().then_some((min, max))
}

/// The only writer of the board camera `Transform`/`Projection`.
fn smooth_camera_transform(
    time: Res<Time>,
    preferences: Option<Res<crate::application::editor::preferences::EditorPreferences>>,
    rig: Res<BoardCameraRig>,
    mut camera: Query<(&mut Transform, &mut Projection), With<Board3dCamera>>,
) {
    let Ok((mut transform, mut projection)) = camera.single_mut() else {
        return;
    };

    let desired = rig.desired_transform();
    let (position_rate, rotation_rate) = match rig.mode {
        RigMode::Follow => (FOLLOW_POSITION_RATE, FOLLOW_ROTATION_RATE),
        RigMode::UserControl => (USER_POSITION_RATE, USER_ROTATION_RATE),
    };
    let dt = time.delta_secs();
    let factor = |rate| {
        if preferences
            .as_ref()
            .is_some_and(|preferences| preferences.reduced_motion)
        {
            1.0
        } else {
            damping_factor(rate, dt)
        }
    };
    transform.translation = transform
        .translation
        .lerp(desired.translation, factor(position_rate));
    transform.rotation = transform
        .rotation
        .slerp(desired.rotation, factor(rotation_rate));

    if let Projection::Orthographic(ref mut orthographic) = *projection {
        orthographic.scaling_mode = bevy::camera::ScalingMode::FixedVertical {
            viewport_height: rig.viewport_height,
        };
        orthographic.scale = 1.0;
    }
}

/// Frame-rate-independent damping: applying `dt` twice equals applying `2*dt`
/// once.
fn damping_factor(rate: f32, dt: f32) -> f32 {
    1.0 - (-rate * dt).exp()
}

/// Orthographic top-down lattice. Yaw rotates the paper around its normal;
/// the default view keeps rows horizontal and glyphs upright.
pub fn isometric_camera_transform(focus: Vec3, distance: f32, orbit_yaw: f32) -> Transform {
    let up = Vec3::new(-orbit_yaw.sin(), 0.0, -orbit_yaw.cos());
    Transform::from_translation(focus + Vec3::Y * distance).looking_at(focus, up)
}

/// Convert a screen-space drag to a board-plane pan (grab-hand semantics),
/// using only the rig's own yaw — never the smoothed camera transform.
pub fn screen_delta_to_plane(orbit_yaw: f32, screen_delta: Vec2, sensitivity: f32) -> Vec2 {
    let right = Vec2::new(orbit_yaw.cos(), -orbit_yaw.sin());
    let forward = Vec2::new(-orbit_yaw.sin(), -orbit_yaw.cos());
    (right * -screen_delta.x + forward * screen_delta.y) * sensitivity
}

fn address_world_position(address: PlacementAddress) -> Vec3 {
    match address {
        PlacementAddress::BoardSlot(slot) => {
            tessera_slot_center(slot, tessera::prelude::TileFootprint::unit(), 0.0)
        }
        PlacementAddress::StackIndex(index) => Vec3::new(
            stack_column_center(index),
            0.0,
            crate::domain::board::geometry::stack_row_center(index.0),
        ),
    }
}

/// Content center of the visible surface (framing target on navigation).
fn visible_surface_center(visible: &VisibleBoardState) -> Vec3 {
    if visible.layout == SurfaceLayoutKind::Stack {
        let rows = stack_canvas_rows(visible);
        return Vec3::new(0.0, 0.0, (rows as f32 - 1.0) * 0.5);
    }
    if let Some((min, max)) = visible_surface_bounds(visible) {
        let center = (min + max) * 0.5;
        return Vec3::new(center.x, 0.0, center.y);
    }
    match visible.layout {
        SurfaceLayoutKind::Stack => stack_content_center(visible),
        SurfaceLayoutKind::Board => {
            if visible.nodes.is_empty() {
                return Vec3::ZERO;
            }
            let mut min = Vec3::splat(f32::INFINITY);
            let mut max = Vec3::splat(f32::NEG_INFINITY);
            for node in &visible.nodes {
                let position = address_world_position(node.address);
                min = min.min(position);
                max = max.max(position);
            }
            Vec3::new((min.x + max.x) * 0.5, 0.0, (min.z + max.z) * 0.5)
        }
    }
}

/// Frame an eight-row workspace, expanding vertically for longer expressions.
pub(crate) fn stack_canvas_rows(visible: &VisibleBoardState) -> usize {
    let last = visible
        .nodes
        .iter()
        .filter_map(|node| match visible.display_address(node.address) {
            PlacementAddress::StackIndex(index) => Some(index.0),
            _ => None,
        })
        .chain(
            visible
                .stack_inserts
                .iter()
                .map(|index| visible.stack_display.display_index(*index).0),
        )
        .max()
        .unwrap_or(0);
    (last / crate::domain::board::geometry::STACK_COLUMNS + 1).max(8)
}

fn stack_content_center(visible: &VisibleBoardState) -> Vec3 {
    let mut indices: Vec<usize> = Vec::new();
    for node in &visible.nodes {
        if let PlacementAddress::StackIndex(index) = visible.display_address(node.address) {
            indices.push(index.0);
        }
    }
    for index in &visible.stack_inserts {
        indices.push(visible.stack_display.display_index(*index).0);
    }
    for index in &visible.stack_locked_slots {
        indices.push(visible.stack_display.display_index(*index).0);
    }

    if indices.is_empty() {
        return Vec3::new(-0.5, 0.0, 0.0);
    }
    let min = *indices.iter().min().unwrap();
    let max = *indices.iter().max().unwrap();
    Vec3::new(stack_column_center(StackIndex((min + max) / 2)), 0.0, 0.0)
}

fn distance_for_surface_change(
    layout: SurfaceLayoutKind,
    reason: &ActiveSurfaceChangeReason,
) -> f32 {
    match (layout, reason) {
        (SurfaceLayoutKind::Stack, ActiveSurfaceChangeReason::EnterContainer { .. }) => {
            CONTAINER_CAMERA_DISTANCE
        }
        (SurfaceLayoutKind::Board, ActiveSurfaceChangeReason::EnterContainer { .. }) => {
            ROOT_CAMERA_DISTANCE
        }
        (_, ActiveSurfaceChangeReason::BreadcrumbJump) => ROOT_CAMERA_DISTANCE,
        (_, ActiveSurfaceChangeReason::TimelineJump) => TIMELINE_CAMERA_DISTANCE,
        (_, ActiveSurfaceChangeReason::DeleteFallbackToRoot)
        | (_, ActiveSurfaceChangeReason::OpenDocument) => match layout {
            SurfaceLayoutKind::Board => ROOT_CAMERA_DISTANCE,
            SurfaceLayoutKind::Stack => CONTAINER_CAMERA_DISTANCE,
        },
    }
}

fn viewport_height_for_surface_change(
    layout: SurfaceLayoutKind,
    reason: &ActiveSurfaceChangeReason,
) -> f32 {
    match (layout, reason) {
        (SurfaceLayoutKind::Stack, ActiveSurfaceChangeReason::EnterContainer { .. }) => {
            CONTAINER_VIEWPORT_HEIGHT
        }
        (SurfaceLayoutKind::Board, ActiveSurfaceChangeReason::EnterContainer { .. }) => {
            ROOT_VIEWPORT_HEIGHT
        }
        (_, ActiveSurfaceChangeReason::BreadcrumbJump) => ROOT_VIEWPORT_HEIGHT,
        (_, ActiveSurfaceChangeReason::TimelineJump) => TIMELINE_VIEWPORT_HEIGHT,
        (_, ActiveSurfaceChangeReason::DeleteFallbackToRoot)
        | (_, ActiveSurfaceChangeReason::OpenDocument) => match layout {
            SurfaceLayoutKind::Board => ROOT_VIEWPORT_HEIGHT,
            SurfaceLayoutKind::Stack => CONTAINER_VIEWPORT_HEIGHT,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::board::BoardSlot;

    fn requests(list: &[CameraRequest]) -> Vec<CameraRequest> {
        list.to_vec()
    }

    fn view_app() -> App {
        use crate::application::pipeline::scene_sync::{
            TileSurfaceContent, VisibleBoardNode, VisibleNodeKind,
        };
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<CameraRequest>()
            .add_message::<BoardViewAction>()
            .init_resource::<BoardCameraRig>()
            .insert_resource(VisibleBoardState {
                nodes: [(-12, 4, 3, true), (24, -8, 2, false)]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (x, y, width, selected))| VisibleBoardNode {
                        node: tessera::prelude::NodeId::new(format!("tile_{i}")),
                        address: PlacementAddress::BoardSlot(BoardSlot::new(x, y)),
                        tessera_footprint: Some(tessera::prelude::TileFootprint::new(width, 2)),
                        kind: VisibleNodeKind::Container,
                        selected,
                        focused: !selected,
                        icon: None,
                        atom: None,
                        ports: None,
                        surface_content: TileSurfaceContent::Empty,
                    })
                    .collect(),
                ..default()
            })
            .add_systems(Update, arbitrate_camera_rig);
        app.update();
        app
    }

    #[test]
    fn fit_selection_uses_full_footprint_and_keeps_music_addresses() {
        let mut app = view_app();
        let before = app.world().resource::<VisibleBoardState>().nodes.clone();
        app.world_mut().write_message(BoardViewAction::FitSelection);
        app.update();
        let rig = app.world().resource::<BoardCameraRig>();
        let center = tessera_slot_center(
            BoardSlot::new(-12, 4),
            tessera::prelude::TileFootprint::new(3, 2),
            0.0,
        );
        assert_eq!(rig.look_target(), center);
        assert_eq!(rig.viewport_height, 3.0);
        assert!(matches!(rig.framing, CameraFraming::Bounds { .. }));
        assert_eq!(app.world().resource::<VisibleBoardState>().nodes, before);
        app.world_mut().write_message(BoardViewAction::FitBoard);
        app.update();
        assert!(app.world().resource::<BoardCameraRig>().viewport_height > 20.0);
    }

    #[test]
    fn zoom_preserves_target_respects_limits_and_does_not_reverse_above_limit() {
        let mut app = view_app();
        app.world_mut().write_message(BoardViewAction::FitSelection);
        app.update();
        let target = app.world().resource::<BoardCameraRig>().look_target();
        for _ in 0..40 {
            app.world_mut().write_message(BoardViewAction::ZoomOut);
        }
        app.update();
        assert_eq!(
            app.world().resource::<BoardCameraRig>().viewport_height,
            24.0
        );
        assert_eq!(
            app.world().resource::<BoardCameraRig>().look_target(),
            target
        );
        for _ in 0..40 {
            app.world_mut().write_message(BoardViewAction::ZoomIn);
        }
        app.update();
        assert_eq!(
            app.world().resource::<BoardCameraRig>().viewport_height,
            3.0
        );
        app.world_mut().write_message(BoardViewAction::FitBoard);
        app.update();
        let large = app.world().resource::<BoardCameraRig>().viewport_height;
        app.world_mut().write_message(BoardViewAction::ZoomOut);
        app.update();
        assert!(app.world().resource::<BoardCameraRig>().viewport_height >= large);
    }

    #[test]
    fn resize_refits_bounds_but_preserves_a_deliberate_zoom() {
        let mut app = view_app();
        let viewport = app
            .world_mut()
            .spawn((
                super::super::board_camera_nav::UiBoardViewport,
                ComputedNode {
                    size: Vec2::new(300.0, 200.0),
                    ..default()
                },
            ))
            .id();
        app.world_mut().write_message(BoardViewAction::FitSelection);
        app.update();
        app.world_mut()
            .get_mut::<ComputedNode>(viewport)
            .unwrap()
            .size = Vec2::new(100.0, 200.0);
        app.update();
        assert_eq!(
            app.world().resource::<BoardCameraRig>().viewport_height,
            8.0
        );
        app.world_mut().write_message(BoardViewAction::ZoomIn);
        app.update();
        let zoom = app.world().resource::<BoardCameraRig>().viewport_height;
        app.world_mut()
            .get_mut::<ComputedNode>(viewport)
            .unwrap()
            .size = Vec2::new(300.0, 200.0);
        app.update();
        assert_eq!(
            app.world().resource::<BoardCameraRig>().viewport_height,
            zoom
        );
    }

    #[test]
    fn stack_fit_uses_the_same_wide_face_as_the_renderer() {
        let mut app = view_app();
        {
            let mut visible = app.world_mut().resource_mut::<VisibleBoardState>();
            visible.layout = SurfaceLayoutKind::Stack;
            visible.nodes.truncate(1);
            visible.nodes[0].address = PlacementAddress::StackIndex(StackIndex(2));
        }
        app.world_mut().write_message(BoardViewAction::FitSelection);
        app.update();
        let rig = app.world().resource::<BoardCameraRig>();
        assert!((rig.look_target().x - (stack_column_center(StackIndex(2)) + 1.0)).abs() < 0.001);
        let CameraFraming::Bounds { min, max } = rig.framing else {
            panic!("expected fitted bounds")
        };
        assert_eq!(max - min, Vec2::new(3.0, 2.0));
    }

    #[test]
    fn navigation_consumes_stale_zoom_and_empty_fit_leaves_view_alone() {
        let mut app = view_app();
        app.world_mut().write_message(BoardViewAction::ZoomIn);
        app.world_mut().write_message(CameraRequest::FrameSurface(
            ActiveSurfaceChangeReason::OpenDocument,
        ));
        app.update();
        assert!(matches!(
            app.world().resource::<BoardCameraRig>().framing,
            CameraFraming::Surface
        ));
        let height = app.world().resource::<BoardCameraRig>().viewport_height;
        app.update();
        assert_eq!(
            app.world().resource::<BoardCameraRig>().viewport_height,
            height
        );
        app.world_mut()
            .resource_mut::<VisibleBoardState>()
            .nodes
            .clear();
        app.world_mut().write_message(BoardViewAction::FitSelection);
        app.update();
        assert_eq!(
            app.world().resource::<BoardCameraRig>().viewport_height,
            height
        );
    }

    #[test]
    fn frame_surface_outranks_focus_and_pan() {
        let queued = requests(&[
            CameraRequest::Pan(Vec2::new(40.0, 0.0)),
            CameraRequest::FocusAddress(PlacementAddress::BoardSlot(BoardSlot::new(3, 3))),
            CameraRequest::FrameSurface(ActiveSurfaceChangeReason::BreadcrumbJump),
        ]);
        let intent = resolve_requests(
            queued.iter(),
            SurfaceLayoutKind::Board,
            Vec3::new(1.0, 0.0, 2.0),
            0.0,
        );
        assert_eq!(
            intent,
            ResolvedIntent::Frame {
                focus: Vec3::new(1.0, 0.0, 2.0),
                distance: ROOT_CAMERA_DISTANCE,
                viewport_height: ROOT_VIEWPORT_HEIGHT,
            }
        );
    }

    #[test]
    fn focus_address_drops_same_frame_pan() {
        let slot = BoardSlot::new(4, 7);
        let queued = requests(&[
            CameraRequest::Pan(Vec2::new(25.0, -10.0)),
            CameraRequest::FocusAddress(PlacementAddress::BoardSlot(slot)),
        ]);
        let intent = resolve_requests(queued.iter(), SurfaceLayoutKind::Board, Vec3::ZERO, 0.0);
        let expected = address_world_position(PlacementAddress::BoardSlot(slot));
        assert_eq!(intent, ResolvedIntent::Focus(expected));

        let mut rig = BoardCameraRig {
            pan_offset: Vec2::new(9.0, 9.0),
            ..default()
        };
        apply_intent(&mut rig, intent);
        assert_eq!(rig.pan_offset, Vec2::ZERO, "focus jumps reset user pan");
        assert_eq!(rig.mode, RigMode::Follow);
    }

    #[test]
    fn pans_accumulate_when_no_deliberate_jump() {
        let queued = requests(&[
            CameraRequest::Pan(Vec2::new(10.0, 0.0)),
            CameraRequest::Pan(Vec2::new(5.0, 0.0)),
        ]);
        let intent = resolve_requests(queued.iter(), SurfaceLayoutKind::Board, Vec3::ZERO, 0.0);
        let ResolvedIntent::Navigate { pan, orbit } = intent else {
            panic!("expected navigate intent");
        };
        assert_eq!(orbit, 0.0);
        // Grab-hand: drag right moves the focus left along +X view axis.
        assert!(pan.x < 0.0);

        let mut rig = BoardCameraRig::default();
        apply_intent(&mut rig, ResolvedIntent::Navigate { pan, orbit });
        assert_eq!(rig.mode, RigMode::UserControl);
        assert_eq!(rig.pan_offset, pan);
    }

    #[test]
    fn idle_returns_to_follow_mode() {
        let mut rig = BoardCameraRig {
            mode: RigMode::UserControl,
            ..default()
        };
        apply_intent(&mut rig, ResolvedIntent::Idle);
        assert_eq!(rig.mode, RigMode::Follow);
    }

    #[test]
    fn damping_is_frame_rate_independent() {
        // Two 1/60s steps must equal one 1/30s step exactly (geometric
        // composition of exponential decay).
        let rate = 6.5;
        let one_step = damping_factor(rate, 1.0 / 30.0);
        let half = damping_factor(rate, 1.0 / 60.0);
        let two_steps = half + (1.0 - half) * half;
        assert!((one_step - two_steps).abs() < 1e-6);
    }

    #[test]
    fn a_sparse_pattern_keeps_its_first_and_last_columns_in_view() {
        let visible = VisibleBoardState {
            layout: SurfaceLayoutKind::Stack,
            ..default()
        };
        for aspect in [0.7, 1.0, 1.5, 2.0] {
            let height = fitted_viewport_height(&visible, aspect, CONTAINER_VIEWPORT_HEIGHT);
            let center = visible_surface_center(&visible);
            let half_width = height * aspect * 0.5;
            assert!(center.x - half_width < -6.0);
            assert!(center.x + half_width > 6.0);
            assert!(center.z - height * 0.5 < -0.5);
            assert!(center.z + height * 0.5 > 7.5);
        }
    }

    #[test]
    fn screen_pan_matches_isometric_axes_at_zero_yaw() {
        let pan = screen_delta_to_plane(0.0, Vec2::new(10.0, 0.0), 1.0);
        assert_eq!(pan, Vec2::new(-10.0, 0.0));
        let pan = screen_delta_to_plane(0.0, Vec2::new(0.0, 10.0), 1.0);
        assert_eq!(pan, Vec2::new(0.0, -10.0));
    }
}
