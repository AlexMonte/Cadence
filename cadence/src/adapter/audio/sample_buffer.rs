//! In-memory decoded audio storage.

use std::{sync::Arc, time::Duration};

use crate::adapter::audio::frame::Frame;

/// Audio data stored fully in memory for reliable one-shot playback.
#[derive(Debug, Clone)]
pub struct SampleBuffer {
    sample_rate: u32,
    frames: Arc<[Frame]>,
}

impl SampleBuffer {
    /// Creates a sample buffer.
    ///
    /// # Panics
    ///
    /// Panics if `sample_rate == 0`.
    #[must_use]
    pub fn new(sample_rate: u32, frames: impl Into<Arc<[Frame]>>) -> Self {
        assert!(sample_rate > 0, "sample_rate must be positive");

        Self {
            sample_rate,
            frames: frames.into(),
        }
    }

    /// Returns the sample rate of the decoded audio.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Returns the number of decoded frames.
    #[must_use]
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Returns `true` when the buffer contains no frames.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Returns the full decoded frame slice.
    #[must_use]
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// Returns one frame by index.
    #[must_use]
    pub fn frame(&self, index: usize) -> Option<Frame> {
        self.frames.get(index).copied()
    }

    /// Returns the decoded duration at the stored sample rate.
    #[must_use]
    pub fn duration(&self) -> Duration {
        if self.is_empty() {
            Duration::ZERO
        } else {
            Duration::from_secs_f64(self.len() as f64 / self.sample_rate as f64)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_duration_matches_frame_count_and_rate() {
        let sample = SampleBuffer::new(
            4,
            vec![
                Frame::from_mono(0.0),
                Frame::from_mono(1.0),
                Frame::from_mono(0.5),
                Frame::from_mono(0.0),
            ],
        );

        assert_eq!(sample.duration(), Duration::from_secs(1));
    }

    #[test]
    fn sample_exposes_frame_access() {
        let sample = SampleBuffer::new(2, vec![Frame::from_mono(0.25), Frame::from_mono(0.5)]);

        assert_eq!(sample.frame(0), Some(Frame::from_mono(0.25)));
        assert_eq!(sample.frame(1), Some(Frame::from_mono(0.5)));
        assert_eq!(sample.frame(2), None);
    }
}
