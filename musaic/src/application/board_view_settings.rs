use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomDisplayMode {
    Stack,
    CompoundTile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AtomDisplayScope {
    RootBoard,
    ContainerPreviewOnRoot,
    ContainerInterior,
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardViewSettings {
    pub root_board: AtomDisplayMode,
    pub container_preview_on_root: AtomDisplayMode,
    pub container_interior: AtomDisplayMode,
}

impl Default for BoardViewSettings {
    fn default() -> Self {
        Self {
            root_board: AtomDisplayMode::CompoundTile,
            container_preview_on_root: AtomDisplayMode::CompoundTile,
            container_interior: AtomDisplayMode::Stack,
        }
    }
}

impl BoardViewSettings {
    pub fn mode_for_scope(self, scope: AtomDisplayScope) -> AtomDisplayMode {
        match scope {
            AtomDisplayScope::RootBoard => self.root_board,
            AtomDisplayScope::ContainerPreviewOnRoot => self.container_preview_on_root,
            AtomDisplayScope::ContainerInterior => self.container_interior,
        }
    }

    pub fn cycle_mode(&mut self, scope: AtomDisplayScope) {
        let toggle = |mode: AtomDisplayMode| match mode {
            AtomDisplayMode::Stack => AtomDisplayMode::CompoundTile,
            AtomDisplayMode::CompoundTile => AtomDisplayMode::Stack,
        };
        match scope {
            AtomDisplayScope::RootBoard => self.root_board = toggle(self.root_board),
            AtomDisplayScope::ContainerPreviewOnRoot => {
                self.container_preview_on_root = toggle(self.container_preview_on_root)
            }
            AtomDisplayScope::ContainerInterior => {
                self.container_interior = toggle(self.container_interior)
            }
        }
    }

    pub fn label_for_scope(self, scope: AtomDisplayScope) -> &'static str {
        match self.mode_for_scope(scope) {
            AtomDisplayMode::Stack => "Stack",
            AtomDisplayMode::CompoundTile => "Compound",
        }
    }
}
