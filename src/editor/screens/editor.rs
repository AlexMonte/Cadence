//! Editor gameplay screen setup (cameras, world root, and lighting).

use bevy::{camera::visibility::RenderLayers, color::palettes::tailwind, prelude::*};

use super::Screen;
use crate::{
    CaptureCursorOptions,
    editor::{
        camera::{
            CANVAS_RENDER_LAYER, CanvasCamera, PREVIEW_MODEL_RENDER_LAYER,
            default_editor_pan_camera,
        },
        //ui::ThemeColorPalette,
    },
};

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct InEditor;

impl ComputedStates for InEditor {
    type SourceStates = Option<Screen>;

    fn compute(sources: Option<Screen>) -> Option<Self> {
        matches!(sources, Some(Screen::Editor)).then_some(InEditor)
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_computed_state::<InEditor>()
        // Setup systems for entering the game screen
        .add_systems(
            OnEnter(InEditor),
            (
                spawn_canvas_camera,
                spawn_editor_world,
                setup_lighting,
                hide_os_cursor_in_editor,
            )
                .chain(),
        )
        .add_systems(OnExit(InEditor), show_os_cursor_outside_editor);
}

fn spawn_canvas_camera(mut commands: Commands) {
    info!("Spawning canvas camera");
    commands.spawn((
        CanvasCamera::default(),
        default_editor_pan_camera(),
        Transform::default(),
        DespawnOnExit(InEditor),
    ));
}

fn spawn_editor_world(mut commands: Commands) {
    info!("Spawning test world");
    commands.spawn((
        Transform::default(),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::VISIBLE,
        ViewVisibility::default(),
        Name::new("Test World Root"),
        DespawnOnExit(InEditor),
        children![
            (
                DirectionalLight::default(),
                Transform::default().looking_to(Vec3::new(-1.0, -3.0, 0.5), Vec3::Y),
                RenderLayers::from_layers(&[
                    CANVAS_RENDER_LAYER as usize,
                    PREVIEW_MODEL_RENDER_LAYER as usize
                ]),
            ),
            (
                PointLight {
                    color: Color::from(tailwind::ROSE_300),
                    shadows_enabled: true,
                    ..default()
                },
                Transform::from_xyz(-2.0, 4.0, -0.75),
                // The light source illuminates both the world model and the view model.
                RenderLayers::from_layers(&[
                    CANVAS_RENDER_LAYER as usize,
                    PREVIEW_MODEL_RENDER_LAYER as usize
                ]),
            ),
        ],
    ));

    // Enable grid snapping (optional)
    // commands.insert_resource(world::grid::GridSnapEnabled);
}

fn hide_os_cursor_in_editor(mut options: ResMut<CaptureCursorOptions>) {
    options.grab_mode = false;
    options.visible = false;
}

fn show_os_cursor_outside_editor(mut options: ResMut<CaptureCursorOptions>) {
    options.grab_mode = false;
    options.visible = true;
}

fn setup_lighting(mut commands: Commands) {
    info!("Setting up lighting");

    // Add directional light (sun)
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.95, 0.8),
            illuminance: 5000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 50.0, 0.0).looking_at(Vec3::new(-10.0, 0.0, -10.0), Vec3::Y),
        Name::new("Sun"),
    ));

    // Add a secondary fill light
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.6, 0.7, 1.0),
            illuminance: 2000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(10.0, 30.0, 20.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("Fill Light"),
    ));
}
