//! Single persistent 2D camera for all window UI (menu, loading, editor chrome).

use bevy::prelude::*;

use crate::infrastructure::ui::theme::MusaicUiTheme;

#[derive(Component)]
pub struct PrimaryWindowCamera;

pub struct AppWindowPlugin;

impl Plugin for AppWindowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_primary_window_camera);
    }
}

fn spawn_primary_window_camera(mut commands: Commands, theme: Res<MusaicUiTheme>) {
    commands.spawn((
        PrimaryWindowCamera,
        Camera2d,
        Camera {
            order: 0,
            clear_color: ClearColorConfig::Custom(theme.chrome.window_bg),
            ..default()
        },
    ));
}
