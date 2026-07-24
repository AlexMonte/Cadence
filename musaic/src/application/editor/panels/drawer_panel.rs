//! Drawer panel user override — `open` is explicit; default derives from focus in [`super::logic`].

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::logic::effective_drawer_open;
use crate::application::editor::workspace::EditorAttention;

/// Controls visibility of the tile palette in the context inspector (not a separate panel).
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct DrawerPanelState {
    /// When `Some`, user explicitly toggled drawer; when `None`, derive from focus.
    pub open: Option<bool>,
}

impl DrawerPanelState {
    pub fn effective_open(&self, attention: &EditorAttention) -> bool {
        effective_drawer_open(self.open, attention)
    }

    pub fn toggle(&mut self, attention: &EditorAttention) {
        let current = self.effective_open(attention);
        self.open = Some(!current);
    }

    pub fn clear_override(&mut self) {
        self.open = None;
    }
}
