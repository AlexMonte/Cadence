use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Right-edge minimap panel (hidden until opened via board edge drag or `M`).
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct MinimapPanelState {
    pub open: bool,
    pub width: f32,
    #[serde(skip)]
    drag_anchor_width: f32,
    #[serde(skip)]
    dragging: bool,
}

impl Default for MinimapPanelState {
    fn default() -> Self {
        Self {
            open: false,
            width: Self::DEFAULT_WIDTH,
            drag_anchor_width: 0.0,
            dragging: false,
        }
    }
}

impl MinimapPanelState {
    pub const EDGE_HIT_WIDTH: f32 = 16.0;
    pub const MIN_WIDTH: f32 = 120.0;
    pub const MAX_WIDTH: f32 = 300.0;
    pub const DEFAULT_WIDTH: f32 = 180.0;
    pub const SNAP_OPEN: f32 = 56.0;

    pub fn visible_width(&self) -> f32 {
        if self.dragging {
            self.width.clamp(0.0, Self::MAX_WIDTH)
        } else if self.open {
            self.width.clamp(Self::MIN_WIDTH, Self::MAX_WIDTH)
        } else {
            0.0
        }
    }

    pub fn begin_drag(&mut self) {
        self.drag_anchor_width = if self.open { self.width } else { 0.0 };
        self.width = self.drag_anchor_width;
        self.dragging = true;
    }

    /// Total pointer travel from the drag start, in logical pixels.
    pub fn apply_drag_distance(&mut self, delta_x: f32) {
        self.width = (self.drag_anchor_width + delta_x).clamp(0.0, Self::MAX_WIDTH);
        self.open = self.width >= Self::SNAP_OPEN;
    }

    pub fn finish_drag(&mut self) {
        self.dragging = false;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_continuous_drag_opens_resizes_and_closes() {
        let mut panel = MinimapPanelState::default();
        panel.begin_drag();
        assert_eq!(panel.visible_width(), 0.0);
        for distance in [4.0, 20.0, 40.0, 100.0, 180.0] {
            panel.apply_drag_distance(distance);
            assert_eq!(panel.visible_width(), distance);
        }
        panel.finish_drag();
        assert!(panel.open);
        assert_eq!(panel.visible_width(), 180.0);
        panel.begin_drag();
        panel.apply_drag_distance(-160.0);
        assert_eq!(panel.visible_width(), 20.0);
        panel.finish_drag();
        assert!(!panel.open);
        assert_eq!(panel.visible_width(), 0.0);
        panel.begin_drag();
        panel.apply_drag_distance(160.0);
        panel.finish_drag();
        assert_eq!(panel.visible_width(), 160.0);
    }
}
