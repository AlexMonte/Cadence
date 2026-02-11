//! Camera components and setup for editor, UI, preview, and cursor render layers.

use crate::AppSystemsSet;
use bevy::{
    camera::visibility::RenderLayers,
    camera_controller::pan_camera::{PanCamera, PanCameraPlugin},
    prelude::*,
    ui::IsDefaultUiCamera,
};

const DEFAULT_CAMERA_SENSITIVITY: Vec2 = Vec2::new(0.003, 0.002);
const CAMERA_ZOOM_MIN: f32 = 0.1;
const CAMERA_ZOOM_MAX: f32 = 5.0;

#[derive(Component, Reflect, Debug, Deref, DerefMut)]
#[reflect(Component, Debug, Default)]
pub struct CameraSensitivity(Vec2);

impl Default for CameraSensitivity {
    fn default() -> Self {
        Self(
            // These factors are just arbitrary mouse sensitivity values.
            // It's often nicer to have a faster horizontal sensitivity than vertical.
            // We use a component for them so that we can make them user-configurable at runtime
            // for accessibility reasons.
            // It also allows you to inspect them in an editor if you `Reflect` the component.
            DEFAULT_CAMERA_SENSITIVITY,
        )
    }
}

pub const CANVAS_RENDER_LAYER: isize = 0;
pub const PREVIEW_MODEL_RENDER_LAYER: isize = 1;
pub const UI_CAMERA_LAYER: isize = 2;
pub const CURSOR_RENDER_LAYER: isize = 3;

pub fn default_editor_pan_camera() -> PanCamera {
    PanCamera {
        // Avoid S-key conflicts with save and other editor actions.
        key_up: Some(KeyCode::ArrowUp),
        key_down: Some(KeyCode::ArrowDown),
        key_left: Some(KeyCode::ArrowLeft),
        key_right: Some(KeyCode::ArrowRight),
        ..default()
    }
}
/// Bevy component for tracking canvas pan and zoom
#[derive(Component, Reflect)]
#[reflect(Component)]
#[require(
    Camera2d,
    Camera{order: CANVAS_RENDER_LAYER, ..default()},
    CameraSensitivity,
    PanCamera,
    RenderLayers::layer(CANVAS_RENDER_LAYER.try_into().unwrap())
)]
pub struct CanvasCamera {
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
}

impl Default for CanvasCamera {
    fn default() -> Self {
        CanvasCamera {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }
}

impl CanvasCamera {
    /// Update camera position
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.pan_x += dx / self.zoom;
        self.pan_y += dy / self.zoom;
    }

    /// Update zoom level
    pub fn zoom(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(CAMERA_ZOOM_MIN, CAMERA_ZOOM_MAX);
    }

    /// Get world position from screen coordinates
    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        let world_x = (screen_x / self.zoom) - self.pan_x;
        let world_y = (screen_y / self.zoom) - self.pan_y;
        (world_x, world_y)
    }

    /// Get screen position from world coordinates
    pub fn world_to_screen(&self, world_x: f32, world_y: f32) -> (f32, f32) {
        let screen_x = (world_x + self.pan_x) * self.zoom;
        let screen_y = (world_y + self.pan_y) * self.zoom;
        (screen_x, screen_y)
    }
}

#[derive(Component, Reflect)]
#[reflect(Component)]
#[require(
    Camera2d,
    Camera{order: PREVIEW_MODEL_RENDER_LAYER, ..default()},
    RenderLayers::layer(PREVIEW_MODEL_RENDER_LAYER.try_into().unwrap())
)]
pub struct ModelViewCamera;

#[derive(Component, Reflect)]
#[reflect(Component)]
#[require(
    Camera{order: UI_CAMERA_LAYER, clear_color: ClearColorConfig::None, ..Default::default()},
    Camera2d,
    RenderLayers::layer(UI_CAMERA_LAYER.try_into().unwrap()),
    IsDefaultUiCamera
)]
pub struct UiCamera;

#[derive(Component, Reflect)]
#[reflect(Component)]
#[require(
    Camera{order: CURSOR_RENDER_LAYER, clear_color: ClearColorConfig::None, ..Default::default()},
    Camera2d,
    RenderLayers::layer(CURSOR_RENDER_LAYER.try_into().unwrap())
)]
pub struct CursorCamera;

pub fn plugin(app: &mut App) {
    app.add_plugins(PanCameraPlugin)
        .register_type::<CanvasCamera>()
        .register_type::<ModelViewCamera>()
        .register_type::<UiCamera>()
        .register_type::<CursorCamera>();

    app.add_systems(Startup, spawn_ui_camera.in_set(AppSystemsSet::Initiate));
}

fn spawn_ui_camera(mut commands: Commands) {
    log::info!("Spawning UI camera");
    commands.spawn(UiCamera);
}

#[cfg(test)]
mod test {
    use crate::editor::camera::{CanvasCamera, default_editor_pan_camera};
    use bevy::prelude::KeyCode;
    #[test]
    fn test_camera_zoom() {
        let mut camera = CanvasCamera::default();
        camera.zoom(2.0);
        assert_eq!(camera.zoom, 2.0);

        camera.zoom(0.5);
        assert_eq!(camera.zoom, 1.0);
    }

    #[test]
    fn test_camera_zoom_bounds() {
        let mut camera = CanvasCamera::default();
        camera.zoom(100.0); // Should clamp to 5.0
        assert_eq!(camera.zoom, 5.0);

        camera.zoom = 1.0;
        camera.zoom(0.05); // Should clamp to 0.1
        assert_eq!(camera.zoom, 0.1);
    }

    #[test]
    fn test_coordinate_conversion() {
        let camera = CanvasCamera {
            pan_x: 10.0,
            pan_y: 20.0,
            zoom: 2.0,
        };

        let (world_x, world_y) = camera.screen_to_world(100.0, 100.0);
        let (screen_x, screen_y) = camera.world_to_screen(world_x, world_y);

        assert!((screen_x - 100.0).abs() < 0.01);
        assert!((screen_y - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_editor_pan_keybindings_use_arrows() {
        let controls = default_editor_pan_camera();
        assert_eq!(controls.key_up, Some(KeyCode::ArrowUp));
        assert_eq!(controls.key_down, Some(KeyCode::ArrowDown));
        assert_eq!(controls.key_left, Some(KeyCode::ArrowLeft));
        assert_eq!(controls.key_right, Some(KeyCode::ArrowRight));
    }
}
