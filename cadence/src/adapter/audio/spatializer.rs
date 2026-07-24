//! Spatial rendering: maps a 3D lattice position to a channel output frame.
//!
//! This is the seam between exact spatial position (resolved per output frame
//! from a [`crate::domain::space::SpatialMotion`]) and the concrete channel
//! format the device expects. v1 ships [`StereoVectorPanner`]; richer layouts
//! (HRTF, ambisonics, more than two channels) add new [`Spatializer`]
//! implementations without touching the voice render path.

use crate::adapter::audio::frame::Frame;

/// Output channel layout a [`Spatializer`] renders into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChannelLayout {
    /// Two-channel stereo.
    Stereo,
}

/// Converts a 3D lattice position into a concrete channel output frame.
///
/// Position uses the lattice convention: `x` is lateral (right positive), `y`
/// is vertical (up positive), `z` is depth (forward positive). The origin is
/// the listener.
pub trait Spatializer: Send + Sync {
    /// Channel layout this spatializer renders into.
    fn layout(&self) -> ChannelLayout;

    /// Places `frame` at `position` `(x, y, z)` and returns the output frame.
    fn place(&self, frame: Frame, position: (f64, f64, f64)) -> Frame;
}

/// Stereo spatializer: constant-power lateral pan plus inverse-distance
/// attenuation.
///
/// v1 honours the lateral axis (pan) and overall distance (gain). Elevation
/// (`y`) and depth low-pass cues are intentionally left to richer spatializers
/// such as a future HRTF or ambisonic implementation; this keeps the stereo
/// path exact and stateless while preserving the trait seam.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StereoVectorPanner {
    /// How strongly distance attenuates gain. `0.0` disables attenuation.
    pub distance_rolloff: f64,
}

impl StereoVectorPanner {
    /// Default inverse-distance rolloff factor.
    pub const DEFAULT_DISTANCE_ROLLOFF: f64 = 1.0;

    /// Creates a stereo vector panner with the default rolloff.
    #[must_use]
    pub fn new() -> Self {
        Self {
            distance_rolloff: Self::DEFAULT_DISTANCE_ROLLOFF,
        }
    }
}

impl Default for StereoVectorPanner {
    fn default() -> Self {
        Self::new()
    }
}

impl Spatializer for StereoVectorPanner {
    fn layout(&self) -> ChannelLayout {
        ChannelLayout::Stereo
    }

    fn place(&self, frame: Frame, position: (f64, f64, f64)) -> Frame {
        let (x, y, z) = position;
        let distance = (x * x + y * y + z * z).sqrt();
        let attenuation = (1.0 / (1.0 + self.distance_rolloff * distance.max(0.0))) as f32;
        let pan = x.clamp(-1.0, 1.0) as f32;
        frame.panned(pan) * attenuation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_is_centered_and_unattenuated() {
        let panner = StereoVectorPanner::new();
        let frame = Frame::new(0.5, 0.5);

        assert_eq!(panner.place(frame, (0.0, 0.0, 0.0)), frame);
    }

    #[test]
    fn lateral_position_pans() {
        let panner = StereoVectorPanner {
            distance_rolloff: 0.0,
        };
        let placed = panner.place(Frame::from_mono(1.0), (1.0, 0.0, 0.0));

        assert!(placed.right > placed.left);
    }

    #[test]
    fn distance_attenuates_gain() {
        let panner = StereoVectorPanner::new();
        let near = panner.place(Frame::from_mono(1.0), (0.0, 0.0, 0.0));
        let far = panner.place(Frame::from_mono(1.0), (0.0, 0.0, 4.0));

        assert!(far.left.abs() < near.left.abs());
    }
}
