//! Editor module composition and editor-scoped application state.

pub mod audio;
pub mod camera;
pub mod canvas;
pub mod ecs;
pub mod gestures;
pub mod history;
pub mod input;
pub mod menus;
pub mod screens;
pub mod semantics;
pub mod state;
pub mod ui;
pub mod ux;

use crate::core::{Project, ScopeId};
use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    // App shell + UI
    app.add_plugins((audio::plugin, ui::plugin));
    #[cfg(feature = "ui_audio")]
    app.add_plugins(ui::interactions::audio::plugin);
    app.add_plugins((menus::plugin, screens::plugin));
    #[cfg(feature = "ui_preview")]
    app.add_plugins(ui::preview::plugin);

    // Input → gestures → semantics
    app.add_plugins((
        camera::plugin,
        input::plugin,
        state::plugin,
        gestures::plugin,
        semantics::plugin,
        ux::plugin,
        history::plugin,
    ));

    // Projection/render
    app.add_plugins(canvas::plugin);
}

/// Bevy resource for editor-side app state and project access
#[derive(Debug, Resource)]
pub struct AppState {
    pub project: Project,
    pub current_scope: ScopeId,
}

impl AppState {
    pub fn new(project: Project) -> Self {
        let current_scope = project.root_scope();
        AppState {
            project,
            current_scope,
        }
    }
}
