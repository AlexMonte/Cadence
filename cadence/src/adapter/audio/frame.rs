//! Stereo audio frames and interpolation helpers.

use std::{
    f32::consts::SQRT_2,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign},
};

/// A stereo audio frame.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Frame {
    /// Left channel sample.
    pub left: f32,
    /// Right channel sample.
    pub right: f32,
}

impl Frame {
    /// Silent stereo frame.
    pub const ZERO: Frame = Frame {
        left: 0.0,
        right: 0.0,
    };

    /// Creates a stereo frame from explicit channel values.
    #[must_use]
    pub fn new(left: f32, right: f32) -> Self {
        Self { left, right }
    }

    /// Creates a stereo frame from one mono sample.
    #[must_use]
    pub fn from_mono(value: f32) -> Self {
        Self::new(value, value)
    }

    /// Applies constant-power panning.
    #[must_use]
    pub fn panned(self, pan: f32) -> Self {
        if pan.abs() <= f32::EPSILON {
            return self;
        }

        let pan = pan.clamp(-1.0, 1.0);
        let left_right_mix = (pan + 1.0) * 0.5;

        Self::new(
            self.left * (1.0 - left_right_mix).sqrt(),
            self.right * left_right_mix.sqrt(),
        ) * SQRT_2
    }

    /// Averages both channels into mono.
    #[must_use]
    pub fn as_mono(self) -> Self {
        Self::from_mono((self.left + self.right) * 0.5)
    }
}

impl Add for Frame {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.left + rhs.left, self.right + rhs.right)
    }
}

impl AddAssign for Frame {
    fn add_assign(&mut self, rhs: Self) {
        self.left += rhs.left;
        self.right += rhs.right;
    }
}

impl Sub for Frame {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.left - rhs.left, self.right - rhs.right)
    }
}

impl SubAssign for Frame {
    fn sub_assign(&mut self, rhs: Self) {
        self.left -= rhs.left;
        self.right -= rhs.right;
    }
}

impl Mul<f32> for Frame {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.left * rhs, self.right * rhs)
    }
}

impl MulAssign<f32> for Frame {
    fn mul_assign(&mut self, rhs: f32) {
        self.left *= rhs;
        self.right *= rhs;
    }
}

impl Div<f32> for Frame {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.left / rhs, self.right / rhs)
    }
}

impl DivAssign<f32> for Frame {
    fn div_assign(&mut self, rhs: f32) {
        self.left /= rhs;
        self.right /= rhs;
    }
}

impl Neg for Frame {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.left, -self.right)
    }
}

/// Interpolates between neighboring frames using cubic interpolation.
#[must_use]
pub fn interpolate_frame(
    previous: Frame,
    current: Frame,
    next_1: Frame,
    next_2: Frame,
    fraction: f32,
) -> Frame {
    let c0 = current;
    let c1 = (next_1 - previous) * 0.5;
    let c2 = previous - current * 2.5 + next_1 * 2.0 - next_2 * 0.5;
    let c3 = (next_2 - previous) * 0.5 + (current - next_1) * 1.5;

    ((c3 * fraction + c2) * fraction + c1) * fraction + c0
}

#[cfg(test)]
mod tests {
    use super::Frame;

    #[test]
    fn center_pan_keeps_frame_unchanged() {
        let frame = Frame::new(0.25, 0.25);

        assert_eq!(frame.panned(0.0), frame);
    }

    #[test]
    fn hard_left_pan_mutes_right_channel() {
        let frame = Frame::new(0.5, 0.5).panned(-1.0);

        assert!(frame.left > 0.0);
        assert_eq!(frame.right, 0.0);
    }
}
