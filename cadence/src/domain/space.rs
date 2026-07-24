//! Exact spatial position over the lattice.
//!
//! Space is the spatial twin of musical time: a [`Point3`] is built from exact
//! rational [`Coord`]s, and a [`SpatialMotion`] is the spatial analog of a
//! [`Signal`] — a small, pure descriptor that resolves to a position as a
//! function of cycle time. Position is carried as resolved data on moments and
//! events; it is never a scheduling axis, so the time-based scheduler is left
//! untouched.
//!
//! As with time, exact rational values are kept for symbolic logic and only
//! collapse to `f64` at the render/audio boundary (via [`Point3::value`] and
//! [`SpatialMotion::position_at`]).

use std::f64::consts::TAU;

use crate::domain::{
    rational::{Coord, Rational},
    signal::Signal,
};

/// One spatial lattice axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Lateral axis (left/right).
    X,
    /// Vertical axis (down/up).
    Y,
    /// Depth axis (back/front).
    Z,
}

/// An exact position on the 3D lattice.
///
/// The origin `(0, 0, 0)` is the lattice center. `x` is lateral, `y` is
/// vertical, and `z` is depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Point3 {
    /// Lateral coordinate.
    pub x: Coord,
    /// Vertical coordinate.
    pub y: Coord,
    /// Depth coordinate.
    pub z: Coord,
}

impl Point3 {
    /// The lattice center.
    pub const ORIGIN: Self = Self {
        x: Coord::ZERO,
        y: Coord::ZERO,
        z: Coord::ZERO,
    };

    /// Creates a point from three coordinates.
    #[must_use]
    pub fn new(x: Coord, y: Coord, z: Coord) -> Self {
        Self { x, y, z }
    }

    /// Returns the approximate floating-point position (render boundary only).
    #[must_use]
    pub fn value(self) -> (f64, f64, f64) {
        (self.x.value(), self.y.value(), self.z.value())
    }

    /// Returns this point scaled component-wise by `factor`.
    #[must_use]
    pub fn scaled(self, factor: Point3) -> Self {
        Self {
            x: self.x * factor.x,
            y: self.y * factor.y,
            z: self.z * factor.z,
        }
    }

    /// Returns this point reflected (negated) across the given axis.
    #[must_use]
    pub fn reflected(self, axis: Axis) -> Self {
        match axis {
            Axis::X => Self { x: -self.x, ..self },
            Axis::Y => Self { y: -self.y, ..self },
            Axis::Z => Self { z: -self.z, ..self },
        }
    }
}

impl std::ops::Add for Point3 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl std::ops::Sub for Point3 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

/// A spatial trajectory resolved as a pure function of cycle time.
///
/// This is the spatial analog of [`Signal`]: every variant is exact and
/// cycle-relative, so the same descriptor always yields the same position at
/// the same point on the timeline. `Linear` sweeps `start -> end` across one
/// cycle (using the fractional cycle phase), mirroring how a periodic signal
/// samples whatever slice of the cycle the host occupies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpatialMotion {
    /// A fixed position.
    Static(Point3),
    /// A straight sweep from `start` to `end` over one cycle.
    Linear {
        /// Position at cycle phase `0`.
        start: Point3,
        /// Position at cycle phase `1`.
        end: Point3,
    },
    /// A circular orbit in the horizontal `x`/`z` plane around `center`.
    Orbit {
        /// Center of the orbit.
        center: Point3,
        /// Orbit radius.
        radius: Coord,
        /// Revolutions per cycle.
        rate: Rational,
        /// Phase offset in cycles.
        phase: Rational,
    },
    /// Independent per-axis signal-driven motion.
    Signal3 {
        /// Lateral signal.
        x: Signal,
        /// Vertical signal.
        y: Signal,
        /// Depth signal.
        z: Signal,
    },
}

impl SpatialMotion {
    /// A static motion fixed at the lattice origin.
    pub const ORIGIN: Self = Self::Static(Point3::ORIGIN);

    /// Resolves the position at the given cycle time.
    ///
    /// This is pure and deterministic. Exact rational parameters collapse to
    /// `f64` here because this is the render boundary.
    #[must_use]
    pub fn position_at(self, cycle_time: f64) -> (f64, f64, f64) {
        match self {
            Self::Static(point) => point.value(),
            Self::Linear { start, end } => {
                let progress = cycle_time - cycle_time.floor();
                let (sx, sy, sz) = start.value();
                let (ex, ey, ez) = end.value();
                (
                    sx + (ex - sx) * progress,
                    sy + (ey - sy) * progress,
                    sz + (ez - sz) * progress,
                )
            }
            Self::Orbit {
                center,
                radius,
                rate,
                phase,
            } => {
                let angle = TAU * (rate.value() * cycle_time + phase.value());
                let (cx, cy, cz) = center.value();
                let r = radius.value();
                (cx + r * angle.cos(), cy, cz + r * angle.sin())
            }
            Self::Signal3 { x, y, z } => {
                (x.eval(cycle_time), y.eval(cycle_time), z.eval(cycle_time))
            }
        }
    }

    /// Returns this motion translated by `offset`.
    #[must_use]
    pub fn translated(self, offset: Point3) -> Self {
        match self {
            Self::Static(point) => Self::Static(point + offset),
            Self::Linear { start, end } => Self::Linear {
                start: start + offset,
                end: end + offset,
            },
            Self::Orbit {
                center,
                radius,
                rate,
                phase,
            } => Self::Orbit {
                center: center + offset,
                radius,
                rate,
                phase,
            },
            Self::Signal3 { x, y, z } => Self::Signal3 {
                x: x.with_bias(x.bias() + offset.x.value()),
                y: y.with_bias(y.bias() + offset.y.value()),
                z: z.with_bias(z.bias() + offset.z.value()),
            },
        }
    }

    /// Returns this motion scaled component-wise by `factor`.
    ///
    /// For an `Orbit`, the lateral (`x`) factor scales the radius so a uniform
    /// scale stays circular.
    #[must_use]
    pub fn scaled(self, factor: Point3) -> Self {
        match self {
            Self::Static(point) => Self::Static(point.scaled(factor)),
            Self::Linear { start, end } => Self::Linear {
                start: start.scaled(factor),
                end: end.scaled(factor),
            },
            Self::Orbit {
                center,
                radius,
                rate,
                phase,
            } => Self::Orbit {
                center: center.scaled(factor),
                radius: radius * factor.x,
                rate,
                phase,
            },
            Self::Signal3 { x, y, z } => Self::Signal3 {
                x: scale_signal(x, factor.x.value()),
                y: scale_signal(y, factor.y.value()),
                z: scale_signal(z, factor.z.value()),
            },
        }
    }

    /// Returns this motion reflected across the given axis.
    #[must_use]
    pub fn reflected(self, axis: Axis) -> Self {
        match self {
            Self::Static(point) => Self::Static(point.reflected(axis)),
            Self::Linear { start, end } => Self::Linear {
                start: start.reflected(axis),
                end: end.reflected(axis),
            },
            Self::Orbit {
                center,
                radius,
                rate,
                phase,
            } => Self::Orbit {
                center: center.reflected(axis),
                radius,
                // Reflecting across one axis flips orbit handedness.
                rate: Rational::ZERO - rate,
                phase,
            },
            Self::Signal3 { x, y, z } => match axis {
                Axis::X => Self::Signal3 {
                    x: reflect_signal(x),
                    y,
                    z,
                },
                Axis::Y => Self::Signal3 {
                    x,
                    y: reflect_signal(y),
                    z,
                },
                Axis::Z => Self::Signal3 {
                    x,
                    y,
                    z: reflect_signal(z),
                },
            },
        }
    }
}

impl Default for SpatialMotion {
    fn default() -> Self {
        Self::ORIGIN
    }
}

fn scale_signal(signal: Signal, factor: f64) -> Signal {
    signal
        .with_depth(signal.depth() * factor)
        .with_bias(signal.bias() * factor)
}

fn reflect_signal(signal: Signal) -> Signal {
    signal.with_depth(-signal.depth()).with_bias(-signal.bias())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_motion_holds_position() {
        let point = Point3::new(Coord::new(1, 2), Coord::ZERO, Coord::new(-1, 1));
        let motion = SpatialMotion::Static(point);

        assert_eq!(motion.position_at(0.0), (0.5, 0.0, -1.0));
        assert_eq!(motion.position_at(7.3), (0.5, 0.0, -1.0));
    }

    #[test]
    fn linear_sweeps_across_one_cycle() {
        let motion = SpatialMotion::Linear {
            start: Point3::ORIGIN,
            end: Point3::new(Coord::ONE, Coord::ZERO, Coord::ZERO),
        };

        assert_eq!(motion.position_at(0.0).0, 0.0);
        assert!((motion.position_at(0.5).0 - 0.5).abs() < 1e-9);
        assert!((motion.position_at(1.25).0 - 0.25).abs() < 1e-9);
    }

    #[test]
    fn translate_then_reflect_compose_exactly() {
        let motion = SpatialMotion::Static(Point3::new(Coord::ONE, Coord::ZERO, Coord::ZERO))
            .translated(Point3::new(Coord::ONE, Coord::ZERO, Coord::ZERO))
            .reflected(Axis::X);

        assert_eq!(motion.position_at(0.0), (-2.0, 0.0, 0.0));
    }

    #[test]
    fn orbit_traces_a_circle() {
        let motion = SpatialMotion::Orbit {
            center: Point3::ORIGIN,
            radius: Coord::ONE,
            rate: Rational::ONE,
            phase: Rational::ZERO,
        };

        let (x0, _, z0) = motion.position_at(0.0);
        assert!((x0 - 1.0).abs() < 1e-9 && z0.abs() < 1e-9);
        let (x_quarter, _, z_quarter) = motion.position_at(0.25);
        assert!(x_quarter.abs() < 1e-9 && (z_quarter - 1.0).abs() < 1e-9);
    }
}
