use bevy::prelude::*;
use bevy::window::{PresentMode, Window, WindowPlugin};

use crate::adapter::platform::PlatformPlugin;
use crate::application::editor::EditorPlugin;
use crate::infrastructure::board::BoardRenderPlugin;
use crate::infrastructure::shell::ShellUiPlugin;
use crate::infrastructure::theme::ThemePlugin;

pub fn run() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(primary_window()),
            ..default()
        }))
        .add_plugins((
            PlatformPlugin,
            ThemePlugin,
            EditorPlugin,
            BoardRenderPlugin,
            ShellUiPlugin,
        ))
        .run();
}

fn primary_window() -> Window {
    Window {
        title: "Cadence".to_string(),
        resolution: (1440, 900).into(),
        present_mode: PresentMode::AutoVsync,
        fit_canvas_to_parent: true,
        prevent_default_event_handling: false,
        ..default()
    }
}
