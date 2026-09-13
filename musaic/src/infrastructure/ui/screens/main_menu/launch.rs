use bevy::prelude::*;

use crate::application::session::MusaicProject;

/// Applied on [`crate::infrastructure::app::AppState::Editor`] entry after load_up finishes.
#[derive(Debug, Clone)]
pub enum EditorLaunchIntent {
    FirstLoop,
    Example,
    NewProject,
    OpenProject(std::path::PathBuf),
    /// In-memory project (e.g. wasm file picker).
    LoadedProject(Box<MusaicProject>),
}

#[derive(Resource, Default)]
pub struct MenuUiRoot(pub Option<Entity>);
