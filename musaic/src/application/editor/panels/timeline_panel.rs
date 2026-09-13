use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Piano-roll band revealed by dragging the bottom tab bar upward.
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct TimelinePanelState {
    pub open: bool,
    pub height: f32,
    /// A browsed cycle is presentation state, independent of the audio clock.
    /// None follows playback (or the revision queued at its next boundary).
    #[serde(skip)]
    pub preview_cycle: Option<u32>,
    #[serde(skip)]
    drag_anchor_height: f32,
    #[serde(skip)]
    dragging: bool,
}

impl Default for TimelinePanelState {
    fn default() -> Self {
        Self {
            open: false,
            height: Self::DEFAULT_HEIGHT,
            preview_cycle: None,
            drag_anchor_height: 0.0,
            dragging: false,
        }
    }
}

impl TimelinePanelState {
    pub const MAX_PREVIEW_CYCLE: u32 = 1_000_000;
    pub const EDGE_HIT_HEIGHT: f32 = 16.0;
    pub const MIN_HEIGHT: f32 = 80.0;
    pub const MAX_HEIGHT: f32 = 280.0;
    pub const DEFAULT_HEIGHT: f32 = 140.0;
    pub const SNAP_OPEN: f32 = 48.0;

    pub fn visible_height(&self) -> f32 {
        if self.dragging {
            self.height.clamp(0.0, Self::MAX_HEIGHT)
        } else if self.open {
            self.height.clamp(Self::MIN_HEIGHT, Self::MAX_HEIGHT)
        } else {
            0.0
        }
    }

    pub fn begin_drag(&mut self) {
        self.drag_anchor_height = if self.open { self.height } else { 0.0 };
        self.height = self.drag_anchor_height;
        self.dragging = true;
    }

    /// Total pointer travel from the drag start, in logical pixels.
    pub fn apply_drag_distance(&mut self, delta_y: f32) {
        self.height = (self.drag_anchor_height + delta_y).clamp(0.0, Self::MAX_HEIGHT);
        self.open = self.height >= Self::SNAP_OPEN;
    }

    pub fn finish_drag(&mut self) {
        self.dragging = false;
        if self.height < Self::SNAP_OPEN {
            self.open = false;
            self.height = Self::DEFAULT_HEIGHT;
        } else {
            self.open = true;
            self.height = self.height.clamp(Self::MIN_HEIGHT, Self::MAX_HEIGHT);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_continuous_drag_opens_resizes_and_closes() {
        let mut panel = TimelinePanelState::default();
        panel.begin_drag();
        assert_eq!(panel.visible_height(), 0.0);
        for distance in [4.0, 20.0, 40.0, 100.0, 180.0] {
            panel.apply_drag_distance(distance);
            assert_eq!(panel.visible_height(), distance);
        }
        panel.finish_drag();
        assert!(panel.open);
        assert_eq!(panel.visible_height(), 180.0);
        panel.begin_drag();
        panel.apply_drag_distance(-160.0);
        assert_eq!(panel.visible_height(), 20.0);
        panel.finish_drag();
        assert!(!panel.open);
        assert_eq!(panel.visible_height(), 0.0);
        panel.begin_drag();
        panel.apply_drag_distance(160.0);
        panel.finish_drag();
        assert_eq!(panel.visible_height(), 160.0);
    }
}
