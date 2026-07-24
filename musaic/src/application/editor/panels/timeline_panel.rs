use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Piano-roll band revealed by dragging the bottom tab bar upward.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct TimelinePanelState {
    pub open: bool,
    pub height: f32,
    #[serde(skip)]
    drag_anchor_height: f32,
}

impl Default for TimelinePanelState {
    fn default() -> Self {
        Self {
            open: false,
            height: Self::DEFAULT_HEIGHT,
            drag_anchor_height: 0.0,
        }
    }
}

impl TimelinePanelState {
    pub const EDGE_HIT_HEIGHT: f32 = 10.0;
    pub const MIN_HEIGHT: f32 = 80.0;
    pub const MAX_HEIGHT: f32 = 280.0;
    pub const DEFAULT_HEIGHT: f32 = 140.0;
    pub const SNAP_OPEN: f32 = 48.0;

    pub fn visible_height(&self) -> f32 {
        if self.open {
            self.height.clamp(Self::MIN_HEIGHT, Self::MAX_HEIGHT)
        } else {
            0.0
        }
    }

    pub fn begin_drag(&mut self) {
        self.drag_anchor_height = if self.open { self.height } else { 0.0 };
        self.open = true;
    }

    pub fn apply_drag_delta(&mut self, delta_y: f32) {
        self.height = (self.drag_anchor_height + delta_y).clamp(0.0, Self::MAX_HEIGHT);
        self.open = self.height >= Self::SNAP_OPEN;
    }

    pub fn finish_drag(&mut self) {
        if self.height < Self::SNAP_OPEN {
            self.open = false;
            self.height = Self::DEFAULT_HEIGHT;
        } else {
            self.open = true;
            self.height = self.height.clamp(Self::MIN_HEIGHT, Self::MAX_HEIGHT);
        }
    }
}
