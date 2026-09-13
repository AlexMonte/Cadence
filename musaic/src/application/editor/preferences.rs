//! User interaction preferences are separate from musical project data.
use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

pub mod keymap;
pub mod library;

#[derive(Resource, Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditorPreferences {
    pub vim_navigation: bool,
    pub reduced_motion: bool,
    pub library: library::LibraryPreferences,
    pub keymap: keymap::Keymap,
}

impl EditorPreferences {
    pub fn validate(&self) -> Result<(), String> {
        self.library.validate()?;
        self.keymap.validate()
    }
}
