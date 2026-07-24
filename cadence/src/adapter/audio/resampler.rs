//! Small interpolation buffer for sample playback resampling.

use crate::adapter::audio::frame::{Frame, interpolate_frame};

#[derive(Debug, Clone, Copy, PartialEq)]
struct RecentFrame {
    frame: Frame,
    frame_index: usize,
}

#[derive(Debug, Clone)]
/// Rolling four-frame interpolation buffer.
pub struct Resampler {
    frames: [RecentFrame; 4],
    time_until_empty: usize,
}

impl Resampler {
    /// Creates a resampler whose initial frame indices start at
    /// `starting_frame_index`.
    #[must_use]
    pub fn new(starting_frame_index: usize) -> Self {
        Self {
            frames: [RecentFrame {
                frame: Frame::ZERO,
                frame_index: starting_frame_index,
            }; 4],
            time_until_empty: 0,
        }
    }

    /// Pushes the next source frame into the interpolation window.
    pub fn push_frame(&mut self, frame: Option<Frame>, sample_index: usize) {
        if frame.is_some() {
            self.time_until_empty = 4;
        } else {
            self.time_until_empty = self.time_until_empty.saturating_sub(1);
        }

        let frame = frame.unwrap_or_default();
        self.frames.copy_within(1.., 0);
        self.frames[self.frames.len() - 1] = RecentFrame {
            frame,
            frame_index: sample_index,
        };
    }

    /// Interpolates the current output frame at fractional position
    /// `fractional_position`.
    #[must_use]
    pub fn get(&self, fractional_position: f32) -> Frame {
        interpolate_frame(
            self.frames[0].frame,
            self.frames[1].frame,
            self.frames[2].frame,
            self.frames[3].frame,
            fractional_position,
        )
    }

    /// Returns the current source frame index.
    #[must_use]
    pub fn current_frame_index(&self) -> usize {
        self.frames[1].frame_index
    }

    /// Returns `true` when the buffer has shifted out all real frames.
    #[must_use]
    pub fn empty(&self) -> bool {
        self.time_until_empty == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampler_returns_current_frame_at_zero_fraction() {
        let mut resampler = Resampler::new(0);
        resampler.push_frame(Some(Frame::from_mono(0.0)), 0);
        resampler.push_frame(Some(Frame::from_mono(1.0)), 1);
        resampler.push_frame(Some(Frame::from_mono(2.0)), 2);
        resampler.push_frame(Some(Frame::from_mono(3.0)), 3);

        assert_eq!(resampler.get(0.0), Frame::from_mono(1.0));
        assert_eq!(resampler.current_frame_index(), 1);
        assert!(!resampler.empty());
    }

    #[test]
    fn resampler_becomes_empty_after_enough_missing_frames() {
        let mut resampler = Resampler::new(0);
        resampler.push_frame(Some(Frame::from_mono(1.0)), 0);

        for _ in 0..4 {
            resampler.push_frame(None, 0);
        }

        assert!(resampler.empty());
    }
}
