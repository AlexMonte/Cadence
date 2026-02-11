use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};

use crate::core::Project;
use crate::editor::AppState;

pub mod core;
#[cfg(feature = "dev")]
mod dev_tools;
pub mod editor;
pub mod runtime;
pub mod shared;
pub mod store;

#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Clone)]
pub enum AppSystemsSet {
    Initiate,
    TickTimers,
    RecordInput,
    Update,
    UpdateCamera,
}

/// Whether or not the app is paused.
#[derive(States, Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
struct Pause(pub bool);

/// A system set for systems that shouldn't run while the app is paused.
#[derive(SystemSet, Copy, Clone, Eq, PartialEq, Hash, Debug)]
struct PausableSystems;

/// System set ordering for the editor.
/// Defines the "shape" of the editor's update pipeline.
///
/// Order:
/// 1. PointerInput: Click/pointer observers capture input, emit SelectionIntent
/// 2. DragComputation: Update PointerState.mode, marquee_rect_screen based on drag
/// 3. SelectionResolution: Apply SelectionIntent, compute marquee hits, update SelectionState
/// 4. RenderSelection: Draw outlines, highlight based on SelectionState
#[derive(SystemSet, Debug, Hash, Eq, PartialEq, Clone)]
pub enum EditorSystemSet {
    PointerInput,
    DragComputation,
    SelectionResolution,
    RenderSelection,
}

/// Configure system set ordering for the editor.
/// Call this in your plugin setup.
pub fn configure_system_sets(app: &mut App) {
    app.configure_sets(
        Update,
        (
            EditorSystemSet::PointerInput,
            EditorSystemSet::DragComputation,
            EditorSystemSet::SelectionResolution,
            EditorSystemSet::RenderSelection,
        )
            .chain(),
    );
}

fn setup_app(mut commands: Commands) {
    // Start in a blank project so the Project Hub can drive project selection.
    let project = Project::new("Untitled".to_string());

    // Initialize AppState with the blank project (no camera spawned here)
    let app_state = AppState::new(project);
    commands.insert_resource(app_state);
}
pub struct GrooveAtlasApp;

impl Plugin for GrooveAtlasApp {
    fn build(&self, app: &mut App) {
        log::info!("Initializing GrooveAtlas");
        app.configure_sets(
            Update,
            (
                AppSystemsSet::Initiate,
                AppSystemsSet::TickTimers,
                AppSystemsSet::RecordInput,
                AppSystemsSet::Update,
                AppSystemsSet::UpdateCamera,
            )
                .chain(),
        );
        configure_system_sets(app);
        app.add_plugins(
            DefaultPlugins
                .build()
                .set(AssetPlugin {
                    meta_check: bevy::asset::AssetMetaCheck::Always,
                    watch_for_changes_override: Some(true),
                    ..Default::default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "GrooveAtlas".to_string(),
                        canvas: Some("#canvas".to_string()),
                        resolution: (1024, 768).into(),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: true,
                        window_theme: Some(bevy::window::WindowTheme::Dark),

                        ..Default::default()
                    }),
                    primary_cursor_options: Some(CursorOptions {
                        grab_mode: CursorGrabMode::Confined,
                        ..default()
                    }),
                    ..Default::default()
                })
                .set(ImagePlugin::default_nearest()),
        );

        app.insert_resource(CaptureCursorOptions::default());
        app.add_plugins((
            #[cfg(feature = "dev")]
            dev_tools::plugin,
            shared::plugin,
            editor::plugin,
            runtime::plugin,
        ));
        app.add_systems(Update, capture_cursor);
        app.add_systems(Startup, setup_app);

        // Set up the `Pause` state.
        app.init_state::<Pause>();
        app.configure_sets(Update, PausableSystems.run_if(in_state(Pause(false))));
    }
}

#[derive(Resource, Reflect)]
#[reflect(Resource, Default)]
pub struct CaptureCursorOptions {
    pub grab_mode: bool,
    pub visible: bool,
}

impl Default for CaptureCursorOptions {
    fn default() -> Self {
        Self {
            grab_mode: false,
            visible: true,
        }
    }
}

fn capture_cursor(
    mut window: Single<&mut CursorOptions, With<Window>>,
    options: ResMut<CaptureCursorOptions>,
) {
    if options.grab_mode {
        window.grab_mode = CursorGrabMode::Locked;
    } else {
        window.grab_mode = CursorGrabMode::None;
    }
    window.visible = options.visible;
}
