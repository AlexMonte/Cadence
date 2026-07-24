//! Positions inside decoded audio.

/// A point in time inside a piece of audio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackPosition {
    /// Position measured in seconds.
    Seconds(f64),
    /// Position measured in exact sample frames.
    Samples(u64),
}

impl PlaybackPosition {
    /// Converts this position into a sample-frame index.
    #[must_use]
    pub fn into_samples(self, sample_rate: u32) -> usize {
        match self {
            Self::Seconds(seconds) => {
                if seconds <= 0.0 {
                    0
                } else {
                    (seconds * sample_rate as f64).round() as usize
                }
            }
            Self::Samples(samples) => samples as usize,
        }
    }
}

impl From<f64> for PlaybackPosition {
    fn from(value: f64) -> Self {
        Self::Seconds(value)
    }
}

impl From<f32> for PlaybackPosition {
    fn from(value: f32) -> Self {
        Self::Seconds(f64::from(value))
    }
}

impl From<u64> for PlaybackPosition {
    fn from(value: u64) -> Self {
        Self::Samples(value)
    }
}

impl From<usize> for PlaybackPosition {
    fn from(value: usize) -> Self {
        Self::Samples(value as u64)
    }
}

impl Default for PlaybackPosition {
    fn default() -> Self {
        Self::Seconds(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::PlaybackPosition;

    #[test]
    fn converts_seconds_to_samples() {
        const SAMPLE_RATE: u32 = 44_100;

        assert_eq!(PlaybackPosition::Seconds(0.0).into_samples(SAMPLE_RATE), 0);
        assert_eq!(
            PlaybackPosition::Seconds(0.5).into_samples(SAMPLE_RATE),
            22_050
        );
        assert_eq!(
            PlaybackPosition::Seconds(1.25).into_samples(SAMPLE_RATE),
            55_125
        );
    }

    #[test]
    fn keeps_sample_positions_exact() {
        assert_eq!(PlaybackPosition::Samples(17).into_samples(44_100), 17);
    }
}
