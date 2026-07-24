use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Right-edge minimap panel (hidden until opened via board edge drag or `M`).
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct MinimapPanelState {
    pub open: bool,
    pub width: f32,
    #[serde(skip)]
    drag_anchor_width: f32,
}

impl Default for MinimapPanelState {
    fn default() -> Self {
        Self {
            open: false,
            width: Self::DEFAULT_WIDTH,
            drag_anchor_width: 0.0,
        }
    }
}

impl MinimapPanelState {
    pub const EDGE_HIT_WIDTH: f32 = 10.0;
    pub const MIN_WIDTH: f32 = 120.0;
    pub const MAX_WIDTH: f32 = 300.0;
    pub const DEFAULT_WIDTH: f32 = 180.0;
    pub const SNAP_OPEN: f32 = 56.0;

    pub fn visible_width(&self) -> f32 {
        if self.open {
            self.width.clamp(Self::MIN_WIDTH, Self::MAX_WIDTH)
        } else {
            0.0
        }
    }

    pub fn begin_drag(&mut self) {
        self.drag_anchor_width = if self.open { self.width } else { 0.0 };
        self.open = true;
    }

    pub fn apply_drag_delta(&mut self, delta_x: f32) {
        self.width = (self.drag_anchor_width + delta_x).clamp(0.0, Self::MAX_WIDTH);
        self.open = self.width >= Self::SNAP_OPEN;
    }

    pub fn finish_drag(&mut self) {
        if self.width < Self::SNAP_OPEN {
            self.open = false;
            self.width = Self::DEFAULT_WIDTH;
        } else {
            self.open = true;
            self.width = self.width.clamp(Self::MIN_WIDTH, Self::MAX_WIDTH);
        }
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
        if self.open && self.width < Self::MIN_WIDTH {
            self.width = Self::DEFAULT_WIDTH;
        }
    }
}
