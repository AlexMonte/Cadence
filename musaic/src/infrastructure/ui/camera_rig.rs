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
    application::editor::{
        ActiveSurfaceChangeReason, ActiveSurfaceChanged, EditorAttention, EditorSession,
        FocusTarget,
    },
    application::pipeline::scene_sync::VisibleBoardState,
    application::session::MusaicProject,
    domain::board::SurfaceLayoutKind,
    domain::document::{DocumentQueries, PlacementAddress, StackIndex},
    infrastructure::app::{AppState, MusaicSet},
};

use super::board::Board3dCamera;
use super::board_geometry::{stack_column_center, tessera_slot_center};

pub const ROOT_CAMERA_DISTANCE: f32 = 12.0;
const CONTAINER_CAMERA_DISTANCE: f32 = 9.0;
const TIMELINE_CAMERA_DISTANCE: f32 = 10.5;
pub const ROOT_VIEWPORT_HEIGHT: f32 = 6.6;
const CONTAINER_VIEWPORT_HEIGHT: f32 = 4.8;
const TIMELINE_VIEWPORT_HEIGHT: f32 = 5.6;

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
    attention: Res<EditorAttention>,
    session: Res<EditorSession>,
    project: Res<MusaicProject>,
    mut requests: MessageWriter<CameraRequest>,
    mut last_focus: Local<Option<FocusTarget>>,
    mut was_placing: Local<bool>,
) {
    let mut surface_changed = false;
    for change in surface_changes.read() {
        surface_changed = true;
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

    // Focus jumps (board click, minimap click, inspector navigation).
    if last_focus.as_ref() != Some(&attention.focus) {
        *last_focus = Some(attention.focus.clone());
        if !surface_changed {
            if let Some(address) = address_for_focus(&attention.focus, &project) {
                requests.write(CameraRequest::FocusAddress(address));
            }
        }
    }
}

fn address_for_focus(focus: &FocusTarget, project: &MusaicProject) -> Option<PlacementAddress> {
    match focus {
        FocusTarget::EmptySlot { slot, .. } => Some(PlacementAddress::BoardSlot(*slot)),
        FocusTarget::StackInsert { index, .. } => Some(PlacementAddress::StackIndex(*index)),
        FocusTarget::Tile { node }
        | FocusTarget::Atom { node }
        | FocusTarget::Port { node, .. } => {
            let queries = DocumentQueries::new(&project.document);
            Some(queries.location_of(node)?.address)
        }
        FocusTarget::None | FocusTarget::TimelineEvent { .. } => None,
    }
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
            rig.focus = focus;
            rig.distance = distance;
            rig.viewport_height = viewport_height;
            rig.pan_offset = Vec2::ZERO;
            rig.orbit_yaw = 0.0;
            rig.mode = RigMode::Follow;
        }
        ResolvedIntent::Focus(position) => {
            rig.focus = Vec3::new(position.x, 0.0, position.z);
            rig.pan_offset = Vec2::ZERO;
            rig.mode = RigMode::Follow;
        }
        ResolvedIntent::Navigate { pan, orbit } => {
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
    visible: Res<VisibleBoardState>,
    mut rig: ResMut<BoardCameraRig>,
) {
    let intent = resolve_requests(
        requests.read(),
        visible.layout,
        visible_surface_center(&visible),
        rig.orbit_yaw,
    );
    if intent == ResolvedIntent::Idle && rig.mode == RigMode::Follow {
        return; // avoid dirtying the resource every frame
    }
    apply_intent(&mut rig, intent);
}

/// The only writer of the board camera `Transform`/`Projection`.
fn smooth_camera_transform(
    time: Res<Time>,
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
    transform.translation = transform
        .translation
        .lerp(desired.translation, damping_factor(position_rate, dt));
    transform.rotation = transform
        .rotation
        .slerp(desired.rotation, damping_factor(rotation_rate, dt));

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

/// 45° isometric view above the board plane, orbited by `yaw`.
pub fn isometric_camera_transform(focus: Vec3, distance: f32, orbit_yaw: f32) -> Transform {
    let pitch = std::f32::consts::FRAC_PI_4;
    let horizontal = distance * pitch.cos();
    let height = distance * pitch.sin();
    let offset = Vec3::new(
        horizontal * orbit_yaw.sin(),
        height,
        horizontal * orbit_yaw.cos(),
    );
    Transform::from_translation(focus + offset).looking_at(focus, Vec3::Y)
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
        PlacementAddress::StackIndex(index) => Vec3::new(stack_column_center(index), 0.0, 0.0),
    }
}

/// Content center of the visible surface (framing target on navigation).
fn visible_surface_center(visible: &VisibleBoardState) -> Vec3 {
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

fn stack_content_center(visible: &VisibleBoardState) -> Vec3 {
    let mut indices: Vec<usize> = Vec::new();
    for node in &visible.nodes {
        if let PlacementAddress::StackIndex(index) = node.address {
            indices.push(index.0);
        }
    }
    for index in &visible.stack_inserts {
        indices.push(index.0);
    }
    for index in &visible.stack_locked_slots {
        indices.push(index.0);
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
    fn screen_pan_matches_isometric_axes_at_zero_yaw() {
        let pan = screen_delta_to_plane(0.0, Vec2::new(10.0, 0.0), 1.0);
        assert_eq!(pan, Vec2::new(-10.0, 0.0));
        let pan = screen_delta_to_plane(0.0, Vec2::new(0.0, 10.0), 1.0);
        assert_eq!(pan, Vec2::new(0.0, -10.0));
    }
}
